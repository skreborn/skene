//! The SSR web platform: a string tree.
//!
//! Emits the same web semantics as the browser leaf (`StyledBox` -> `<div>` with
//! `display:flex`) to an HTML string instead of a live DOM. It implements the shared
//! vocabulary the built-in views build against; the raw-handle escape hatch (`element`,
//! `document`) is absent by construction, so a view that reaches for a real `Element` will
//! not compile here and must wrap in [`client_only`](crate::client_only). Layout, painting
//! and interactivity are the client's after hydration; `handle_click` is inert on the server.
//! Render with [`render_to_string`].

pub(crate) mod tree;

mod stylesheet;
pub(crate) mod task;

use std::{cell::RefCell, rc::Rc};

use rustc_hash::FxHashMap;
use skene_core::{IntoView, NodeId, ScrollEvent, View};

use self::stylesheet::StyleStore;
pub use self::task::{drive, set_render_deadline, sleep};
// The CSS declaration set is web-family shared; this leaf takes it in `set_style`.
use crate::Declarations;

/// The virtual root; only its children are serialized, never the node itself.
const ROOT: NodeId = NodeId::new(0);

/// One element in the string tree.
struct Node {
  tag: String,
  attributes: Vec<(String, String)>,
  class: Option<Rc<str>>,
  inline: Declarations,
  text: Option<String>,
  children: Vec<NodeId>,
  parent: Option<NodeId>,
}

impl Node {
  fn new(tag: &str) -> Self {
    Self {
      tag: tag.to_owned(),
      attributes: Vec::new(),
      class: None,
      inline: Declarations::new(),
      text: None,
      children: Vec::new(),
      parent: None,
    }
  }
}

/// The SSR platform's state: the node arena and the string stylesheet. Private—reached
/// only through this module's functions.
#[must_use]
struct State {
  nodes: FxHashMap<NodeId, Node>,
  styles: StyleStore,
  next_id: u64,
  document_title: Option<String>,
  head_entries: Vec<(String, Vec<(String, String)>)>,
  preempt_scheme: bool,
}

impl State {
  fn new() -> Self {
    let mut nodes = FxHashMap::default();

    nodes.insert(ROOT, Node::new(""));

    Self {
      nodes,
      styles: StyleStore::new(),
      next_id: 1,
      document_title: None,
      head_entries: Vec::new(),
      preempt_scheme: false,
    }
  }

  /// Renders the collected head content: the scheme script first, when the page needs
  /// it, then the title and each head element in registration order.
  fn render_head(&self) -> String {
    let mut out = String::new();

    if self.preempt_scheme {
      out.push_str("<script>");
      out.push_str(PREEMPT_SCHEME);
      out.push_str("</script>");
    }

    if let Some(title) = &self.document_title {
      out.push_str("<title>");
      escape_text(title, &mut out);
      out.push_str("</title>");
    }

    for (tag, attributes) in &self.head_entries {
      out.push('<');
      out.push_str(tag);

      for (name, value) in attributes {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        escape_attr(value, &mut out);
        out.push('"');
      }

      out.push('>');

      if tag == "script" {
        out.push_str("</script>");
      }
    }

    out
  }

  fn alloc_id(&mut self) -> NodeId {
    let id = NodeId::new(self.next_id);

    self.next_id += 1;

    id
  }

  fn create_element(&mut self, tag: &str) -> NodeId {
    let id = self.alloc_id();

    self.nodes.insert(id, Node::new(tag));

    id
  }

  fn create_group(&mut self) -> NodeId {
    self.create_element("div")
  }

  fn set_attribute(&mut self, node: NodeId, name: &str, value: &str) {
    if let Some(node) = self.nodes.get_mut(&node) {
      node.attributes.push((name.to_owned(), value.to_owned()));
    }
  }

  fn set_text(&mut self, node: NodeId, text: &str) {
    if let Some(node) = self.nodes.get_mut(&node) {
      node.text = Some(text.to_owned());
    }
  }

  /// Interns `template` as a shared, content-addressed class and assigns it to `node`,
  /// refcounting rules so those of a torn-down subtree are collected before serialization.
  fn assign_class(&mut self, node: NodeId, template: &str) {
    let previous = self.nodes.get(&node).and_then(|node| node.class.clone());
    let class = self.styles.class_for(template);

    if previous.as_deref() == Some(&*class) {
      return;
    }

    self.styles.retain(&class);

    if let Some(old) = &previous {
      self.styles.release(old);
    }

    if let Some(node) = self.nodes.get_mut(&node) {
      node.class = Some(class);
    }
  }

  /// Merges `delta` into a node's inline style, winning over its interned class—the path a
  /// reactive property takes, so its settled value is serialized inline.
  fn override_style(&mut self, node: NodeId, delta: &Declarations) {
    if let Some(node) = self.nodes.get_mut(&node) {
      for (property, value) in delta.iter() {
        node.inline = std::mem::take(&mut node.inline).set(property, value.to_owned());
      }
    }
  }

  /// The node's children, in document order; empty for one the arena no longer holds.
  fn children(&self, node: NodeId) -> Vec<NodeId> {
    self
      .nodes
      .get(&node)
      .map(|node| node.children.clone())
      .unwrap_or_default()
  }

  fn attach_child(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    if let Some(old) = self.nodes.get(&child).and_then(|node| node.parent)
      && let Some(old) = self.nodes.get_mut(&old)
    {
      old.children.retain(|node| *node != child);
    }

    if let Some(child_node) = self.nodes.get_mut(&child) {
      child_node.parent = Some(parent);
    }

    if let Some(parent) = self.nodes.get_mut(&parent) {
      match before.and_then(|before| parent.children.iter().position(|node| *node == before)) {
        Some(index) => parent.children.insert(index, child),
        None => parent.children.push(child),
      }
    }
  }

  fn detach_child(&mut self, parent: NodeId, child: NodeId) {
    if let Some(parent) = self.nodes.get_mut(&parent) {
      parent.children.retain(|node| *node != child);
    }

    if let Some(child) = self.nodes.get_mut(&child) {
      child.parent = None;
    }
  }

  fn destroy(&mut self, node: NodeId) {
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
      if let Some(node) = self.nodes.remove(&current) {
        stack.extend(node.children);

        if let Some(class) = node.class {
          self.styles.release(&class);
        }
      }
    }
  }

  /// Renders the `<style>` block: the reset plus the interned rules.
  fn render_styles(&self) -> String {
    format!(
      "<style>{}{}</style>",
      crate::RESET_CSS,
      self.styles.render()
    )
  }

  /// Renders the settled view tree—the page body.
  fn render_body(&self) -> String {
    let mut out = String::new();

    if let Some(root) = self.nodes.get(&ROOT) {
      for child in &root.children {
        self.serialize(*child, &mut out);
      }
    }

    out
  }

  fn serialize(&self, id: NodeId, out: &mut String) {
    let Some(node) = self.nodes.get(&id)
    else {
      return;
    };

    node.emit_open(id, out);

    match &node.text {
      Some(text) => escape_text(text, out),
      None => {
        for child in &node.children {
          self.serialize(*child, out);
        }
      }
    }

    node.emit_close(out);
  }

  /// Serializes the tree in document order, pausing at each unsettled boundary.
  ///
  /// Resumes from `cursor`, appending to `out` until it reaches a child that is a boundary whose
  /// work has not settled—then returns that boundary's node, leaving the cursor poised to emit
  /// it next. Ancestors stay open across the pause, exactly as a streamed document expects. A
  /// `None` return means the whole tree is serialized.
  fn stream(&self, cursor: &mut Vec<Frame>, out: &mut String) -> Option<NodeId> {
    while let Some(&Frame { id, next_child }) = cursor.last() {
      let Some(node) = self.nodes.get(&id)
      else {
        cursor.pop();

        continue;
      };

      if next_child == node.children.len() {
        if id != ROOT {
          node.emit_close(out);
        }

        cursor.pop();

        continue;
      }

      let child = node.children[next_child];

      if boundary_pending(child) {
        return Some(child);
      }

      cursor.last_mut().expect("the frame just read").next_child += 1;

      let child_node = self
        .nodes
        .get(&child)
        .expect("a child listed in its parent");

      child_node.emit_open(child, out);

      match &child_node.text {
        Some(text) => {
          escape_text(text, out);
          child_node.emit_close(out);
        }
        None if child_node.children.is_empty() => child_node.emit_close(out),
        None => cursor.push(Frame {
          id: child,
          next_child: 0,
        }),
      }
    }

    None
  }
}

/// A boundary whose content settles asynchronously, and a probe for whether it has.
type BoundarySettled = Box<dyn Fn() -> bool>;

/// A position in the in-order stream walk: a node whose opening tag is emitted and how many of
/// its children have been serialized.
#[derive(Clone, Copy)]
struct Frame {
  id: NodeId,
  next_child: usize,
}

impl Node {
  /// Emits a node's opening tag: `<tag data-h="id" …attributes… class style>`.
  fn emit_open(&self, id: NodeId, out: &mut String) {
    out.push('<');
    out.push_str(&self.tag);
    // The hydration marker: the client matches each node it builds to the server element with
    // its creation-order id, so a divergence is caught rather than silently mis-adopted.
    out.push_str(" data-h=\"");
    out.push_str(&id.value().to_string());
    out.push('"');

    for (name, value) in &self.attributes {
      out.push(' ');
      out.push_str(name);
      out.push_str("=\"");
      escape_attr(value, out);
      out.push('"');
    }

    if let Some(class) = &self.class {
      out.push_str(" class=\"");
      out.push_str(class);
      out.push('"');
    }

    if !self.inline.is_empty() {
      out.push_str(" style=\"");
      escape_attr(&self.inline.render(), out);
      out.push('"');
    }

    out.push('>');
  }

  /// Emits a node's closing tag: `</tag>`—nothing for a void element, which has no
  /// closing tag in HTML.
  fn emit_close(&self, out: &mut String) {
    if self.is_void() {
      return;
    }

    out.push_str("</");
    out.push_str(&self.tag);
    out.push('>');
  }

  /// Whether the node's tag is an HTML void element.
  fn is_void(&self) -> bool {
    matches!(self.tag.as_str(), "input" | "br" | "hr" | "img")
  }
}

thread_local! {
  static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
  /// The statically-renderable routes the app recorded during the last render.
  static STATIC_ROUTES: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
  /// The path being rendered, as supplied by the runner; `/` when none was.
  static REQUEST_PATH: RefCell<String> = RefCell::new("/".to_owned());
  /// Each critical resource's serializer, in creation order; drained once the render settles.
  static SOURCES: RefCell<Vec<crate::HydrationSource>> = const { RefCell::new(Vec::new()) };
  /// Each tag-keyed resource's serializer; drained with [`SOURCES`], sorted by tag.
  static TAGGED_SOURCES: RefCell<Vec<(String, crate::HydrationSource)>> =
    const { RefCell::new(Vec::new()) };
  /// Each deferred boundary's node, mapped to a probe for whether its work has settled; consulted
  /// by the streaming walk to decide where to pause.
  static BOUNDARIES: RefCell<FxHashMap<NodeId, BoundarySettled>> = RefCell::new(FxHashMap::default());
}

/// Registers a deferred boundary: its group node and a probe for whether its content has settled.
/// The streaming render pauses before an unsettled boundary, blocks on it, then emits its content
/// in place.
pub fn register_boundary(node: NodeId, settled: BoundarySettled) {
  BOUNDARIES.with(|boundaries| boundaries.borrow_mut().insert(node, settled));
}

/// Whether `node` is a boundary whose content has not settled—the streaming walk's pause test.
fn boundary_pending(node: NodeId) -> bool {
  BOUNDARIES.with(|boundaries| {
    boundaries
      .borrow()
      .get(&node)
      .is_some_and(|settled| !settled())
  })
}

/// Sets the path the coming render is for, read back by [`current_path`].
pub fn set_request_path(path: &str) {
  REQUEST_PATH.with(|current| path.clone_into(&mut current.borrow_mut()));
}

/// The path being rendered, as supplied through [`set_request_path`].
#[must_use]
pub fn current_path() -> String {
  REQUEST_PATH.with(|current| current.borrow().clone())
}

/// The server never adopts: it produces the state the client adopts.
#[inline(always)]
#[must_use]
pub fn adopt_next() -> Option<String> {
  None
}

/// Registers a critical resource's serializer, to be drained once the render settles.
pub fn register_source(source: crate::HydrationSource) {
  SOURCES.with(|sources| sources.borrow_mut().push(source));
}

/// The server never adopts: it produces the state the client adopts.
#[inline(always)]
#[must_use]
pub fn adopt_tagged(_tag: &str) -> Option<String> {
  None
}

/// Registers a tag-keyed resource's serializer, to be drained once the render settles.
///
/// # Panics
///
/// Panics when `tag` is already registered in this render: tags pair one server value
/// with one client resource, so a duplicate has no well-defined owner.
pub fn register_tagged_source(tag: &str, source: crate::HydrationSource) {
  TAGGED_SOURCES.with(|sources| {
    let mut sources = sources.borrow_mut();

    assert!(
      sources.iter().all(|(existing, _)| existing != tag),
      "duplicate resource tag: {tag}",
    );

    sources.push((tag.to_owned(), source));
  });
}

/// This leaf renders on the server, so a client-only load skips its round here.
#[inline(always)]
#[must_use]
pub fn is_server() -> bool {
  true
}

/// Marks the page as one whose reader may have chosen a scheme, so its head carries the
/// script that applies that choice before the first paint.
///
/// The choice itself is the reader's and lives in their browser, which a server render
/// cannot see: the markup carries the platform-following form, which needs no script at
/// all, and [`PREEMPT_SCHEME`] corrects it from storage before anything is painted. Only
/// a page that can be told a scheme pays for the script.
pub fn set_color_scheme(_scheme: Option<skene_core::ColorScheme>) {
  with_state(|state| state.preempt_scheme = true);
}

/// The color scheme the reader chose on a previous visit; never known to a server.
#[must_use]
pub const fn stored_scheme() -> Option<skene_core::ColorScheme> {
  None
}

/// The script that applies a stored scheme choice before the first paint.
///
/// Inline and blocking in the head, which is the whole point: a deferred script runs
/// after the page has already been painted in the platform's scheme, which is the flash
/// it exists to prevent. It writes exactly what the driver's own rules are gated on, and
/// reads only its own key, so a page that never chose a scheme is untouched.
pub(crate) const PREEMPT_SCHEME: &str = "try{var r=document.documentElement,\
s=localStorage.getItem('skene-scheme');\
if(s==='dark'||s==='light'){r.setAttribute('data-skene-scheme',s);r.style.colorScheme=s}}catch(e){}";

/// Whether the client owns the page; never, on the server, so a view serializes its
/// scriptless form.
#[must_use]
pub fn settled() -> bool {
  false
}

/// Runs `apply` against the installed platform state.
///
/// # Panics
///
/// Panics if no render is in progress on this thread.
#[inline(always)]
fn with_state<R>(apply: impl FnOnce(&mut State) -> R) -> R {
  STATE.with(|state| {
    apply(
      state
        .borrow_mut()
        .as_mut()
        .expect("the SSR platform to be rendering on this thread"),
    )
  })
}

/// Creates a DOM element with the given `tag` as a detached, tree-participating node.
#[must_use]
pub fn create_element(tag: &str) -> NodeId {
  with_state(|state| state.create_element(tag))
}

/// Sets `node`'s `name` attribute to `value`.
pub fn set_attribute(node: NodeId, name: &str, value: &str) {
  with_state(|state| state.set_attribute(node, name, value));
}

/// Interns `declarations` as a shared, content-addressed class and assigns it to `node`.
pub fn set_style(node: NodeId, declarations: &Declarations) {
  with_state(|state| {
    state.assign_class(node, &crate::mapping::declarations_template(declarations));
  });
}

/// Interns a full rule template as a shared, content-addressed class and assigns it to
/// `node`.
pub(crate) fn assign_class(node: NodeId, template: &str) {
  with_state(|state| state.assign_class(node, template));
}

/// Applies each of `declarations` as an inline override on `node`, winning over its
/// interned class.
pub fn override_style(node: NodeId, declarations: &Declarations) {
  with_state(|state| state.override_style(node, declarations));
}

/// Sets a node's text content.
pub fn set_text(node: NodeId, text: &str) {
  with_state(|state| state.set_text(node, text));
}

/// Registers a click handler—inert on the server, where HTML is static; interactivity
/// returns when the client hydrates.
#[inline(always)]
pub fn handle_click(_node: NodeId, _handler: Rc<skene_core::ClickHandler>) {}

/// Registers a pointer-enter handler—inert on the server.
#[inline(always)]
pub fn handle_pointer_enter(_node: NodeId, _handler: Rc<dyn Fn()>) {}

/// Registers a pointer-leave handler—inert on the server.
#[inline(always)]
pub fn handle_pointer_leave(_node: NodeId, _handler: Rc<dyn Fn()>) {}

/// Registers a focus handler—inert on the server.
#[inline(always)]
pub fn handle_focus(_node: NodeId, _handler: Rc<dyn Fn()>) {}

/// Registers a blur handler—inert on the server.
#[inline(always)]
pub fn handle_blur(_node: NodeId, _handler: Rc<dyn Fn()>) {}

/// Registers a key handler—inert on the server.
#[inline(always)]
pub fn handle_key_down(_node: NodeId, _handler: Rc<skene_core::KeyHandler>) {}

/// Inert on the server; interactivity is the client's after hydration.
pub fn handle_change(_node: NodeId, _handler: Rc<dyn Fn(&str)>) {}

/// Inert on the server; interactivity is the client's after hydration.
pub fn handle_toggle(_node: NodeId, _handler: Rc<dyn Fn(bool)>) {}

/// Inert on the server; interactivity is the client's after hydration.
pub fn handle_pointer_down(_node: NodeId, _handler: Rc<skene_core::PointerHandler>) {}

/// Inert on the server; interactivity is the client's after hydration.
pub fn handle_pointer_up(_node: NodeId, _handler: Rc<skene_core::PointerHandler>) {}

/// Inert on the server; interactivity is the client's after hydration.
pub fn handle_pointer_move(_node: NodeId, _handler: Rc<skene_core::PointerHandler>) {}

/// Inert on the server; interactivity is the client's after hydration.
pub fn handle_scroll(_node: NodeId, _handler: Rc<dyn Fn(&ScrollEvent)>) {}

/// Inert on the server; sizes are measured by the client after hydration.
pub fn observe_size(_node: NodeId, _handler: Rc<dyn Fn(f32, f32)>) {}

/// Inert on the server; a lazy view's content renders in place, claimed only by the
/// hydrating client.
#[must_use]
pub fn claim_lazy(_node: NodeId) -> bool {
  false
}

/// Inert on the server; a lazy view's content renders in place, discarded only by the
/// hydrating client.
pub fn discard_lazy(_node: NodeId) {}

/// Inert on the server; focus is the client's after hydration.
pub fn focus_node(_node: NodeId) {}

/// Inert on the server; focus is the client's after hydration.
pub fn blur_node(_node: NodeId) {}

/// Inert on the server; an overlay serializes closed.
pub fn register_overlay(_node: NodeId, _modal: bool, _placement: skene_core::Placement) {}

/// Inert on the server; an overlay serializes closed.
pub fn set_overlay_open(_node: NodeId, _open: bool, _anchor: Option<NodeId>) {}

/// Inert on the server; interactivity is the client's after hydration.
pub fn handle_overlay_dismiss(_node: NodeId, _handler: Rc<dyn Fn()>) {}

/// Sets the value of an input, text area, or select node: the `value` attribute, the
/// text content, or the matching option's `selected` mark, respectively.
pub fn set_value(node: NodeId, value: &str) {
  with_state(|state| {
    let Some(record) = state.nodes.get(&node)
    else {
      return;
    };

    match record.tag.as_str() {
      "textarea" => {
        if let Some(record) = state.nodes.get_mut(&node) {
          record.text = Some(value.to_owned());
        }
      }

      "select" => {
        let options = record.children.clone();

        for option in options {
          let Some(record) = state.nodes.get_mut(&option)
          else {
            continue;
          };

          let matches = record
            .attributes
            .iter()
            .any(|(name, current)| name == "value" && current == value);

          record.attributes.retain(|(name, _)| name != "selected");

          if matches {
            record
              .attributes
              .push(("selected".to_owned(), String::new()));
          }
        }
      }

      // A range input serializes its value and nothing computed from it: a filled run
      // painted here would freeze at this value while the control kept working without
      // script, so the slider would lie about its own state. See the reset.
      _ => {
        if let Some(record) = state.nodes.get_mut(&node) {
          match record
            .attributes
            .iter_mut()
            .find(|(name, _)| name == "value")
          {
            Some((_, existing)) => value.clone_into(existing),
            None => record
              .attributes
              .push(("value".to_owned(), value.to_owned())),
          }
        }
      }
    }
  });
}

/// Records the app's statically-renderable routes, readable after the render through
/// [`recorded_static_routes`].
pub fn record_static_routes(routes: Vec<String>) {
  STATIC_ROUTES.with(|recorded| *recorded.borrow_mut() = Some(routes));
}

/// The routes the last render recorded, or `None` when it recorded none.
#[must_use]
pub fn recorded_static_routes() -> Option<Vec<String>> {
  STATIC_ROUTES.with(|recorded| recorded.borrow().clone())
}

/// Sets the document title, serialized into the page head.
pub fn set_document_title(title: &str) {
  with_state(|state| state.document_title = Some(title.to_owned()));
}

/// Adds an element to the page head, serialized in registration order.
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) {
  with_state(|state| {
    state.head_entries.push((
      tag.to_owned(),
      attributes
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect(),
    ));
  });
}

/// Sets the checked state of a checkbox or radio node, serialized as the `checked`
/// attribute.
pub fn set_checked(node: NodeId, checked: bool) {
  with_state(|state| {
    if let Some(record) = state.nodes.get_mut(&node) {
      record.attributes.retain(|(name, _)| name != "checked");

      if checked {
        record
          .attributes
          .push(("checked".to_owned(), String::new()));
      }
    }
  });
}

/// Renders `app`, returning `(styles, head, body, state)`: the `<style>` block, the
/// registered head content, the settled view tree, and the encoded resource state the
/// client adopts on hydration.
///
/// The load-bearing steps: build the tree, drive its resources to a fixpoint so the *settled*
/// tree is what serializes, drain the resource state while the signals are still alive, then
/// tear the render down so the next call starts clean.
pub fn render_to_string<V, F>(app: F) -> (String, String, String, String)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  STATE.with(|state| *state.borrow_mut() = Some(State::new()));
  STATIC_ROUTES.with(|recorded| *recorded.borrow_mut() = None);
  SOURCES.with(|sources| sources.borrow_mut().clear());
  TAGGED_SOURCES.with(|sources| sources.borrow_mut().clear());
  // A `Deferred` still registers its boundary here, though this path serializes the fully-settled
  // tree and never consults it; clear so registrations do not accumulate across renders.
  BOUNDARIES.with(|boundaries| boundaries.borrow_mut().clear());

  let scope = skene_core::root_owner().child();

  scope.enter(|| {
    let node = app().into_view().build();

    with_state(|state| state.attach_child(ROOT, node, None));
  });

  task::stabilize();

  // Drain the sources while the resource signals are still alive, before the scope disposes.
  let state = collect_hydration_state();
  let styles = with_state(|state| state.render_styles());
  let head = with_state(|state| state.render_head());
  let body = with_state(|state| state.render_body());

  scope.dispose();
  task::clear();

  SOURCES.with(|sources| sources.borrow_mut().clear());
  TAGGED_SOURCES.with(|sources| sources.borrow_mut().clear());
  BOUNDARIES.with(|boundaries| boundaries.borrow_mut().clear());
  STATE.with(|state| *state.borrow_mut() = None);

  (styles, head, body, state)
}

/// One piece of a streamed render, in emission order.
pub enum StreamPiece<'a> {
  /// The settled shell's styles and head, before any body chunk.
  Shell {
    /// The `<style>` block.
    styles: &'a str,
    /// The registered head content.
    head: &'a str,
  },
  /// One body chunk: the markup up to the next deferred boundary, or after the last.
  Chunk(String),
  /// The encoded hydration state, once the walk completes.
  Tail(String),
}

/// Renders `app` as an in-order stream, each piece handed to `flush` as it is
/// produced: the shell once it settles, a body chunk at each deferred boundary, and
/// the hydration state last.
///
/// The shell settles and its styles render, then the body walks in document order. At each
/// deferred boundary the walk flushes the chunk built so far, blocks until that boundary's
/// content settles, and resumes emitting it in place—so a client with no JavaScript receives a
/// correct document progressively, each chunk appended to the last.
///
/// The styles are the shell's; a boundary whose loaded content introduces a brand-new class is
/// the delta-style streaming's concern, not yet handled here.
pub fn render_to_stream_with<V, F>(app: F, mut flush: impl FnMut(StreamPiece<'_>))
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  STATE.with(|state| *state.borrow_mut() = Some(State::new()));
  STATIC_ROUTES.with(|recorded| *recorded.borrow_mut() = None);
  SOURCES.with(|sources| sources.borrow_mut().clear());
  TAGGED_SOURCES.with(|sources| sources.borrow_mut().clear());
  BOUNDARIES.with(|boundaries| boundaries.borrow_mut().clear());

  let scope = skene_core::root_owner().child();
  let root = skene_core::Boundary::new();

  scope.enter(|| {
    // The whole render is a boundary: work outside any deferred boundary counts as the shell's,
    // so the walk can settle it before flushing the first chunk.
    skene_reactive::provide_context(root);

    let node = app().into_view().build();

    with_state(|state| state.attach_child(ROOT, node, None));
  });

  task::stabilize_until(move || root.is_settled());

  let styles = with_state(|state| state.render_styles());
  let head = with_state(|state| state.render_head());

  flush(StreamPiece::Shell {
    styles: &styles,
    head: &head,
  });

  let mut cursor = vec![Frame {
    id: ROOT,
    next_child: 0,
  }];

  loop {
    let mut chunk = String::new();
    let pending = with_state(|state| state.stream(&mut cursor, &mut chunk));

    flush(StreamPiece::Chunk(chunk));

    let Some(node) = pending
    else {
      break;
    };

    task::stabilize_until(|| !boundary_pending(node));
    // Drop it whether or not it settled: settled emits its content, unsettled emits what it has,
    // and either way the walk must not pause on it again.
    BOUNDARIES.with(|boundaries| boundaries.borrow_mut().remove(&node));
  }

  flush(StreamPiece::Tail(collect_hydration_state()));

  scope.dispose();
  task::clear();

  SOURCES.with(|sources| sources.borrow_mut().clear());
  TAGGED_SOURCES.with(|sources| sources.borrow_mut().clear());
  BOUNDARIES.with(|boundaries| boundaries.borrow_mut().clear());
  STATE.with(|state| *state.borrow_mut() = None);
}

/// [`render_to_stream_with`], collected: `(styles, head, body_chunks, state)`.
pub fn render_to_stream<V, F>(app: F) -> (String, String, Vec<String>, String)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  let mut styles = String::new();
  let mut head = String::new();
  let mut chunks = Vec::new();
  let mut state = String::new();

  render_to_stream_with(app, |piece| match piece {
    StreamPiece::Shell {
      styles: shell_styles,
      head: shell_head,
    } => {
      shell_styles.clone_into(&mut styles);
      shell_head.clone_into(&mut head);
    }
    StreamPiece::Chunk(chunk) => chunks.push(chunk),
    StreamPiece::Tail(tail) => state = tail,
  });

  (styles, head, chunks, state)
}

/// Forgets every registered resource serializer, so a render starts clean.
pub(crate) fn reset_hydration_sources() {
  SOURCES.with(|sources| sources.borrow_mut().clear());
  TAGGED_SOURCES.with(|sources| sources.borrow_mut().clear());
}

/// Drains the registered sources into the encoded hydration state.
pub(crate) fn collect_hydration_state() -> String {
  let values: Vec<Option<String>> =
    SOURCES.with(|sources| sources.borrow().iter().map(|source| source()).collect());
  let mut tagged: Vec<(String, String)> = TAGGED_SOURCES.with(|sources| {
    sources
      .borrow()
      .iter()
      .filter_map(|(tag, source)| source().map(|value| (tag.clone(), value)))
      .collect()
  });

  tagged.sort();

  crate::hydration::encode_state(&values, &tagged)
}

pub(crate) fn escape_text(text: &str, out: &mut String) {
  for character in text.chars() {
    match character {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      other => out.push(other),
    }
  }
}

pub(crate) fn escape_attr(value: &str, out: &mut String) {
  for character in value.chars() {
    match character {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      other => out.push(other),
    }
  }
}
