//! The server leaf's realization of the platform: a string tree serialized to HTML with
//! the interned stylesheet, the head and the hydration state, whole or as an in-order
//! stream around deferred boundaries.
//!
//! Every element carries a `data-h` marker counting elements in creation order, which
//! the browser leaf adopts by; an anchor is a comment pair (`<!--s:N-->`, `<!--e:N-->`)
//! counting segments the same way; a scope is a `display: contents` element. Handlers are inert,
//! focus and sizes are the client's, and an overlay serializes closed. A streamed chunk opens with
//! a `<style>` delta holding the registrations and rules interned since the previous flush.

mod task;

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  thread::LocalKey,
  time::Duration,
};

use rustc_hash::{FxHashMap, FxHashSet};
use skene_common::gui::{GuiExt, Painter, Painting};
use skene_core::{
  AccessProp,
  Boundary,
  BoxProps,
  BoxValue,
  ColorScheme,
  ContainerSizes,
  ControlProps,
  ControlState,
  Definitions,
  EventKind,
  FocusOrder,
  IntoView,
  MediaSignals,
  NodeId,
  NodeKind,
  OverlayProps,
  Part,
  Platform,
  RunProps,
  RunValue,
  Scene,
  Size,
  Slot,
  StaticLeaf,
  TextSelection,
  Timer,
  Toggle,
  TokenName,
  View,
};
use skene_reactive::{Owner, ReadSignal, Set, Signal, WriteSignal, provide_context};

pub use self::task::{drive, install_executor, set_render_deadline, sleep};
use super::{
  Web,
  compile::{
    Compiled,
    SlotBinding,
    all_unconditional,
    box_fingerprint,
    compile_box,
    compile_control,
    compile_definitions,
    compile_overlay,
    compile_root_definitions,
    compile_run,
    inline_definitions,
    overlay_fingerprint,
    render_box_leaf,
    render_run_leaf,
    run_fingerprint,
    slot_registrations,
  },
  condition::{CLASS_PLACEHOLDER, CONTROL_ATTRIBUTE, NATIVE_CLASS},
  markup::{
    MARKER_ATTRIBUTE,
    NATIVE_ACCESS_ATTRIBUTES,
    NativeElement,
    SCOPE_ATTRIBUTE,
    STYLE_ATTRIBUTE,
    STYLE_ID,
    access_attribute,
    anchor_text,
    aria_role,
    box_tag,
    drawn_element,
    element_id,
    focus_order_attribute,
    is_form_element,
    link_attributes,
    native_attribute,
    native_attributes,
    native_element,
    referenced_nodes,
    runs_script,
    scrolls,
    takes_disabled,
  },
  motion::VIEW_TRANSITION_RULE,
  token::WebTokenValue,
};
use crate::{
  Declarations,
  RESET_CSS,
  RenderHints,
  WebMarkup,
  hydration::STATE_SCRIPT_ID,
  interner::ClassInterner,
};

/// The root of the string tree; its children are the page body.
const ROOT: u32 = 0;

/// A probe for whether a deferred boundary's content has settled.
type BoundarySettled = Box<dyn Fn() -> bool>;

/// A position in the in-order stream walk: a node whose opening tag is emitted and how
/// many of its children have been serialized.
#[derive(Clone, Copy)]
struct Frame {
  index: u32,
  next_child: usize,
}

/// What a node is in the string tree.
enum Content {
  Element(&'static str),
  Comment(String),
}

/// One node of the string tree.
struct Node {
  content: Content,
  /// The element's hydration marker: its place in element creation order.
  marker: u32,
  attributes: Vec<(String, String)>,
  style_class: Option<Rc<str>>,
  token_class: Option<Rc<str>>,
  inline: Vec<(String, String)>,
  text: Option<String>,
  children: Vec<u32>,
  parent: Option<u32>,
  /// The native element carrying a control's state, focus and value, when the control
  /// has one apart from its root.
  carrier: Option<u32>,
  control: bool,
  /// Whether the box scrolls its content, so it takes a place in the focus order.
  scrolls: bool,
  /// Whether the node stands bare: a control that is its own `<input>`, `<textarea>`
  /// or `<select>`, whose caption is a `<label for>` beside it.
  bare: bool,
  /// The `<label for>` a bare control's caption is serialized in, before the control,
  /// outside its children.
  caption: Option<u32>,
  /// A `<select>`'s options, each a value and the name shown, and the value chosen.
  options: Vec<(String, String)>,
  chosen: Option<String>,
  /// The scene node the element realizes.
  realizes: Option<NodeId>,
  slots: Rc<[SlotBinding]>,
}

impl Node {
  fn element(tag: &'static str) -> Self {
    Self {
      content: Content::Element(tag),
      marker: 0,
      attributes: Vec::new(),
      style_class: None,
      token_class: None,
      inline: Vec::new(),
      text: None,
      children: Vec::new(),
      parent: None,
      carrier: None,
      control: false,
      scrolls: false,
      bare: false,
      options: Vec::new(),
      chosen: None,
      caption: None,
      realizes: None,
      slots: Rc::from([]),
    }
  }

  fn comment(text: String) -> Self {
    Self {
      content: Content::Comment(text),
      ..Self::element("")
    }
  }

  fn tag(&self) -> &'static str {
    match self.content {
      Content::Element(tag) => tag,
      Content::Comment(_) => "",
    }
  }

  fn set_attribute(&mut self, name: &str, value: &str) {
    match self
      .attributes
      .iter_mut()
      .find(|(existing, _)| existing == name)
    {
      Some((_, existing)) => value.clone_into(existing),
      None => self.attributes.push((name.to_owned(), value.to_owned())),
    }
  }

  fn remove_attribute(&mut self, name: &str) {
    self.attributes.retain(|(existing, _)| existing != name);
  }

  fn set_flag(&mut self, name: &str, on: bool) {
    if on {
      self.set_attribute(name, "");
    }
    else {
      self.remove_attribute(name);
    }
  }

  /// Writes the attributes a control's native element carries.
  fn write_native_attributes(&mut self, native: NativeElement, props: &ControlProps<Web>) {
    for (name, value) in native_attributes(native, props) {
      self.set_attribute(name, &value);
    }
  }

  fn set_inline(&mut self, property: String, value: String) {
    match self
      .inline
      .iter_mut()
      .find(|(existing, _)| *existing == property)
    {
      Some((_, existing)) => *existing = value,
      None => self.inline.push((property, value)),
    }
  }

  fn is_void(&self) -> bool {
    matches!(self.tag(), "input" | "br" | "hr" | "img")
  }

  fn emit_comment(text: &str, out: &mut String) {
    out.push_str("<!--");
    out.push_str(text);
    out.push_str("-->");
  }

  fn emit_open(&self, out: &mut String) {
    out.push('<');
    out.push_str(self.tag());
    out.push(' ');
    out.push_str(MARKER_ATTRIBUTE);
    out.push_str("=\"");
    out.push_str(&self.marker.to_string());
    out.push('"');

    for (name, value) in &self.attributes {
      out.push(' ');
      out.push_str(name);

      if !value.is_empty() {
        out.push_str("=\"");
        escape_attr(value, out);
        out.push('"');
      }
    }

    let classes: Vec<&str> = self
      .style_class
      .iter()
      .chain(&self.token_class)
      .map(|class| &**class)
      .collect();

    if !classes.is_empty() {
      out.push_str(" class=\"");
      out.push_str(&classes.join(" "));
      out.push('"');
    }

    if !self.inline.is_empty() {
      out.push_str(" style=\"");

      let mut style = String::new();

      for (property, value) in &self.inline {
        style.push_str(property);
        style.push(':');
        style.push_str(value);
        style.push(';');
      }

      escape_attr(&style, out);
      out.push('"');
    }

    out.push('>');
  }

  /// Writes a `<select>`'s options, the chosen one marked.
  fn emit_options(&self, out: &mut String) {
    for (value, name) in &self.options {
      out.push_str("<option value=\"");
      escape_attr(value, out);
      out.push('"');

      if self.chosen.as_deref() == Some(value.as_str()) {
        out.push_str(" selected");
      }

      out.push('>');
      escape_text(name, out);
      out.push_str("</option>");
    }
  }

  fn emit_close(&self, out: &mut String) {
    if self.is_void() {
      return;
    }

    out.push_str("</");
    out.push_str(self.tag());
    out.push('>');
  }
}

/// The string tree, its stylesheet and the head, for one render.
struct Tree {
  nodes: Vec<Option<Node>>,
  styles: ClassInterner,
  cache: FxHashMap<u64, Rc<Compiled>>,
  registered: FxHashSet<TokenName>,
  registrations: String,
  registrations_emitted: usize,
  emitted: FxHashSet<Rc<str>>,
  root_class: Option<Rc<str>>,
  markers: u32,
  anchors: u32,
  title: Option<String>,
  language: Option<String>,
  head: Vec<(String, Vec<(String, String)>)>,
}

impl Tree {
  fn new() -> Self {
    Self {
      nodes: vec![Some(Node::element(""))],
      styles: ClassInterner::new(),
      cache: FxHashMap::default(),
      registered: FxHashSet::default(),
      registrations: String::new(),
      registrations_emitted: 0,
      emitted: FxHashSet::default(),
      root_class: None,
      markers: 0,
      anchors: 0,
      title: None,
      language: None,
      head: Vec::new(),
    }
  }

  fn create(&mut self, node: Node) -> u32 {
    let index = u32::try_from(self.nodes.len()).expect("a tree holds at most 2^32 nodes");

    self.nodes.push(Some(node));

    index
  }

  fn element(&mut self, tag: &'static str) -> u32 {
    self.markers += 1;

    let mut node = Node::element(tag);

    node.marker = self.markers;

    self.create(node)
  }

  fn node(&mut self, index: u32) -> Option<&mut Node> {
    self.nodes.get_mut(index as usize).and_then(Option::as_mut)
  }

  /// The element carrying `index`'s state: its native input, else itself.
  fn carrier(&mut self, index: u32) -> u32 {
    self
      .nodes
      .get(index as usize)
      .and_then(Option::as_ref)
      .and_then(|node| node.carrier)
      .unwrap_or(index)
  }

  /// The compiled template for `key`, rendered by `compile` on a miss.
  fn compiled(&mut self, key: u64, compile: impl FnOnce() -> Compiled) -> Rc<Compiled> {
    Rc::clone(self.cache.entry(key).or_insert_with(|| Rc::new(compile())))
  }

  /// Interns `template` and swaps it in for `previous`, returning the class.
  fn swap_class(&mut self, template: &str, previous: Option<&str>) -> Rc<str> {
    let (class, _) = self.styles.class_for(template);

    if previous != Some(&*class) {
      self.styles.retain(&class);

      if let Some(previous) = previous {
        self.styles.release(previous);
      }
    }

    class
  }

  fn assign_style(&mut self, index: u32, compiled: &Rc<Compiled>) {
    let previous = self.node(index).and_then(|node| node.style_class.clone());
    let class = self.swap_class(&compiled.template, previous.as_deref());

    if let Some(node) = self.node(index) {
      node.style_class = Some(class);
      node.slots = Rc::from(compiled.slots.as_slice());
    }
  }

  /// Attaches `child` into the caption of `parent` when `parent` stands bare and
  /// `child` its `Label` part, making the `<label for>` on the first; `false` for any
  /// other pair, which attaches as usual.
  fn attach_caption(&mut self, parent: u32, child: u32) -> bool {
    let Some(node) = self
      .node(parent)
      .filter(|node| node.bare)
      .and_then(|node| node.realizes)
    else {
      return false;
    };
    let is_label = self
      .node(child)
      .and_then(|child| child.realizes)
      .is_some_and(|child| Scene::<Web>::with(|scene| scene.part(child) == Some(Part::Label)));

    if !is_label {
      return false;
    }

    let caption = if let Some(caption) = self.node(parent).and_then(|node| node.caption) {
      caption
    }
    else {
      let caption = self.element("label");
      let id = element_id(node);

      if let Some(label) = self.node(caption) {
        label.set_attribute("for", &id);
      }

      if let Some(entry) = self.node(parent) {
        entry.set_attribute("id", &id);
        entry.caption = Some(caption);
      }

      caption
    };

    self.attach(caption, child, None);

    true
  }

  /// Detaches `child` from the caption of `parent` when it sits there, dropping the
  /// caption once empty; `false` when it does not.
  fn detach_caption(&mut self, parent: u32, child: u32) -> bool {
    let Some(caption) = self.node(parent).and_then(|node| node.caption)
    else {
      return false;
    };

    if self.node(child).and_then(|node| node.parent) != Some(caption) {
      return false;
    }

    self.detach(caption, child);

    if self
      .node(caption)
      .is_some_and(|label| label.children.is_empty())
    {
      self.destroy(caption);

      if let Some(entry) = self.node(parent) {
        entry.caption = None;
      }
    }

    true
  }

  fn attach(&mut self, parent: u32, child: u32, before: Option<u32>) {
    if let Some(old) = self.node(child).and_then(|node| node.parent)
      && let Some(old) = self.node(old)
    {
      old.children.retain(|node| *node != child);
    }

    if let Some(node) = self.node(child) {
      node.parent = Some(parent);
    }

    if let Some(parent) = self.node(parent) {
      match before.and_then(|before| parent.children.iter().position(|node| *node == before)) {
        Some(index) => parent.children.insert(index, child),
        None => parent.children.push(child),
      }
    }
  }

  fn detach(&mut self, parent: u32, child: u32) {
    if let Some(parent) = self.node(parent) {
      parent.children.retain(|node| *node != child);
    }

    if let Some(child) = self.node(child) {
      child.parent = None;
    }
  }

  fn destroy(&mut self, index: u32) {
    let mut stack = vec![index];

    while let Some(current) = stack.pop() {
      let Some(node) = self.nodes.get_mut(current as usize).and_then(Option::take)
      else {
        continue;
      };

      stack.extend(node.children);
      stack.extend(node.caption);

      for class in node.style_class.iter().chain(&node.token_class) {
        self.styles.release(class);
      }
    }
  }

  fn register(&mut self, definitions: &Definitions<WebTokenValue>) {
    for (name, value) in definitions.iter() {
      if self.registered.insert(name) {
        self.registrations.push_str(&value.registration(name));
      }
    }
  }

  /// The `<style>` block a document opens with: the reset, the slot registrations, and
  /// every token registration and live rule not emitted yet, in interning order.
  fn render_styles(&mut self) -> String {
    let mut out = format!("<style id=\"{STYLE_ID}\">");

    out.push_str(RESET_CSS);
    out.push_str(&slot_registrations());

    if VIEW_TRANSITIONS.with(Cell::get) {
      out.push_str(VIEW_TRANSITION_RULE);
    }

    self.render_new_styles(&mut out);
    out.push_str("</style>");

    out
  }

  /// The `<style>` block a later chunk opens with: the token registrations and live
  /// rules not emitted yet, in interning order; empty when there are none.
  fn render_style_delta(&mut self) -> String {
    let mut delta = String::new();

    self.render_new_styles(&mut delta);

    if delta.is_empty() {
      return delta;
    }

    format!("<style {STYLE_ATTRIBUTE}>{delta}</style>")
  }

  /// Appends the token registrations and live rules not emitted yet, marking them
  /// emitted; no `</` is written, so the text cannot end the `<style>` it sits in.
  fn render_new_styles(&mut self, out: &mut String) {
    push_style_text(out, &self.registrations[self.registrations_emitted..]);
    self.registrations_emitted = self.registrations.len();

    for (class, template) in self.styles.live_rules() {
      if self.emitted.contains(class) {
        continue;
      }

      push_style_text(out, &template.replace(CLASS_PLACEHOLDER, class));
      self.emitted.insert(Rc::from(class));
    }
  }

  fn render_head(&self) -> String {
    let mut out = String::new();

    if PREEMPT_SCHEME.with(Cell::get) {
      out.push_str("<script>");
      out.push_str(PREEMPT_SCHEME_SCRIPT);
      out.push_str("</script>");
    }

    if let Some(title) = &self.title {
      out.push_str("<title>");
      escape_text(title, &mut out);
      out.push_str("</title>");
    }

    for (tag, attributes) in &self.head {
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

  fn render_body(&self) -> String {
    let mut out = String::new();

    if let Some(Some(root)) = self.nodes.first() {
      for child in &root.children {
        self.serialize(*child, &mut out);
      }
    }

    out
  }

  fn serialize(&self, index: u32, out: &mut String) {
    let Some(Some(node)) = self.nodes.get(index as usize)
    else {
      return;
    };

    if let Content::Comment(text) = &node.content {
      Node::emit_comment(text, out);

      return;
    }

    if let Some(caption) = node.caption {
      self.serialize(caption, out);
    }

    node.emit_open(out);
    node.emit_options(out);

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

  /// Serializes the tree in document order from `cursor`, pausing before a child
  /// `pending` reports as an unsettled boundary.
  ///
  /// Appends to `out` until it reaches such a child, returning that node with the
  /// cursor poised to emit it next; ancestors stay open across the pause, exactly as a
  /// streamed document expects. `None` means the whole tree is serialized.
  fn stream(
    &self,
    cursor: &mut Vec<Frame>,
    out: &mut String,
    pending: impl Fn(u32) -> bool,
  ) -> Option<u32> {
    while let Some(&Frame { index, next_child }) = cursor.last() {
      let Some(Some(node)) = self.nodes.get(index as usize)
      else {
        cursor.pop();

        continue;
      };

      if next_child == node.children.len() {
        if index != ROOT {
          node.emit_close(out);
        }

        cursor.pop();

        continue;
      }

      let child = node.children[next_child];

      if pending(child) {
        return Some(child);
      }

      cursor.last_mut().expect("the frame just read").next_child += 1;

      let Some(Some(child_node)) = self.nodes.get(child as usize)
      else {
        continue;
      };

      if let Content::Comment(text) = &child_node.content {
        Node::emit_comment(text, out);

        continue;
      }

      if let Some(caption) = child_node.caption {
        self.serialize(caption, out);
      }

      child_node.emit_open(out);
      child_node.emit_options(out);

      match &child_node.text {
        Some(text) => {
          escape_text(text, out);
          child_node.emit_close(out);
        }
        None if child_node.children.is_empty() => child_node.emit_close(out),
        None => cursor.push(Frame {
          index: child,
          next_child: 0,
        }),
      }
    }

    None
  }
}

/// The environment's signals with their write halves.
struct MediaState {
  signals: MediaSignals<f32>,
  set_viewport: WriteSignal<Size<f32>>,
  set_scheme: WriteSignal<ColorScheme>,
  set_reduced_motion: WriteSignal<bool>,
  /// The server's render is never hydrated and shows no pixels.
  absent: ReadSignal<bool>,
}

thread_local! {
  static SCENE: RefCell<Scene<Web>> = RefCell::new(Scene::new());
  static TREE: RefCell<Tree> = RefCell::new(Tree::new());
  static HINTS: Cell<RenderHints> = const { Cell::new(RenderHints::DEFAULT) };
  static MEDIA: RefCell<Option<MediaState>> = const { RefCell::new(None) };
  static PREEMPT_SCHEME: Cell<bool> = const { Cell::new(false) };
  /// Whether the app opted into view transitions, so the stylesheet carries the
  /// cross-document rule.
  static VIEW_TRANSITIONS: Cell<bool> = const { Cell::new(false) };
  /// The path being rendered, as supplied by the runner; `/` when none was.
  static REQUEST_PATH: RefCell<String> = RefCell::new("/".to_owned());
  /// The statically-renderable routes the app recorded during the last render.
  static STATIC_ROUTES: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
  /// Each deferred boundary's start anchor, mapped to a probe for whether its content
  /// has settled; consulted by the streaming walk to decide where to pause.
  static BOUNDARIES: RefCell<FxHashMap<u32, BoundarySettled>> = RefCell::new(FxHashMap::default());
}

/// The script that applies a stored scheme choice before the first paint.
///
/// Inline and blocking in the head: it writes exactly what the platform's own rules are
/// gated on and reads only its own key, so a page that never chose a scheme is
/// untouched.
const PREEMPT_SCHEME_SCRIPT: &str = "try{var r=document.documentElement,\
s=localStorage.getItem('skene-scheme');\
if(s==='dark'||s==='light'){r.setAttribute('data-skene-scheme',s);r.style.colorScheme=s}}catch(e){}";

/// Sets the path the coming render is for, read back by [`current_path`].
pub fn set_request_path(path: &str) {
  REQUEST_PATH.with(|current| path.clone_into(&mut current.borrow_mut()));
}

/// The path being rendered, as supplied through [`set_request_path`].
#[must_use]
pub fn current_path() -> String {
  REQUEST_PATH.with(|current| current.borrow().clone())
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

/// Forgets the routes the last render recorded, so the next starts clean.
fn forget_static_routes() {
  STATIC_ROUTES.with(|recorded| *recorded.borrow_mut() = None);
}

/// Appends CSS text to a `<style>` element's content, escaping every `</` so the text
/// cannot end the element.
fn push_style_text(out: &mut String, text: &str) {
  let mut rest = text;

  while let Some(at) = rest.find("</") {
    out.push_str(&rest[..at]);
    out.push_str("<\\/");
    rest = &rest[at + 2..];
  }

  out.push_str(rest);
}

/// Appends `text` with the characters HTML text content reserves escaped.
fn escape_text(text: &str, out: &mut String) {
  for character in text.chars() {
    match character {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      other => out.push(other),
    }
  }
}

/// Appends `value` with the characters an HTML attribute value reserves escaped.
fn escape_attr(value: &str, out: &mut String) {
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

/// Runs `operation` on the tree.
fn with_tree<R>(operation: impl FnOnce(&mut Tree) -> R) -> R {
  TREE.with(|tree| operation(&mut tree.borrow_mut()))
}

/// Runs `operation` on the node `index`, if it is live.
fn with_node<R>(index: u32, operation: impl FnOnce(&mut Node) -> R) -> Option<R> {
  with_tree(|tree| tree.node(index).map(operation))
}

/// Sets the environment the following renders answer with.
pub fn set_render_hints(hints: RenderHints) {
  HINTS.with(|current| current.set(hints));
  MEDIA.with(|media| {
    if let Some(media) = &*media.borrow() {
      media.set_viewport.set(hints.viewport);
      media.set_scheme.set(hints.scheme);
      media.set_reduced_motion.set(hints.reduced_motion);
    }
  });
}

/// The environment's signals, created under the root owner on first use.
fn media_signals() -> MediaSignals<f32> {
  with_media(|media| media.signals)
}

/// Runs `operation` on the environment state, created under the root owner on first
/// use.
fn with_media<R>(operation: impl FnOnce(&MediaState) -> R) -> R {
  MEDIA.with(|media| {
    let mut media = media.borrow_mut();
    let media = media.get_or_insert_with(|| {
      let hints = HINTS.with(Cell::get);

      skene_core::root_owner().enter(|| {
        let (viewport, set_viewport) = Signal::opaque(hints.viewport).split();
        let (scheme, set_scheme) = Signal::opaque(hints.scheme).split();
        let (reduced_motion, set_reduced_motion) = Signal::opaque(hints.reduced_motion).split();
        let (absent, _) = Signal::opaque(false).split();

        MediaState {
          signals: MediaSignals {
            viewport,
            scheme,
            reduced_motion,
            container_sizes: ContainerSizes::new(),
          },
          set_viewport,
          set_scheme,
          set_reduced_motion,
          absent,
        }
      })
    });

    operation(media)
  })
}

/// Writes `focus_order` as `tabindex` on `node`; `None` takes a control out of focus
/// and leaves a box without one.
fn write_focus_order(node: &mut Node, focus_order: Option<FocusOrder>) {
  match focus_order_attribute(focus_order, node.control, node.scrolls) {
    Some(tabindex) => node.set_attribute("tabindex", &tabindex),
    None => node.remove_attribute("tabindex"),
  }
}

/// Replaces the ARIA attributes of `node` with those `access` maps to; a form element
/// takes the native attribute where one exists.
fn write_access(node: &mut Node, access: &[AccessProp]) {
  let form = matches!(node.content, Content::Element(tag) if is_form_element(tag));

  node.attributes.retain(|(name, _)| {
    let native = form && NATIVE_ACCESS_ATTRIBUTES.contains(&name.as_str());

    !native && !name.starts_with("aria-") && name != "title"
  });

  for prop in access {
    if form && let Some(name) = native_attribute(prop) {
      node.set_attribute(name, "");

      continue;
    }

    let (name, value) = access_attribute(prop);

    node.set_attribute(name, &value);
  }
}

/// Gives every node `access` refers to the id the reference names.
fn write_referenced_ids(tree: &mut Tree, access: &[AccessProp]) {
  for node in referenced_nodes(access) {
    let Some(sink) = Scene::<Web>::with(|scene| scene.sink(node))
    else {
      continue;
    };
    let target = tree.carrier(sink);

    if let Some(target) = tree.node(target) {
      target.set_attribute("id", &element_id(node));
    }
  }
}

/// Realizes the props every box-like node shares onto `index`.
fn finish_box(index: u32, props: &BoxProps<Web>, compiled: &Rc<Compiled>) {
  with_tree(|tree| {
    tree.assign_style(index, compiled);

    let target = tree.carrier(index);
    let scrolls = scrolls::<Web>(&props.style);

    if let Some(node) = tree.node(target) {
      node.scrolls = scrolls;
      write_focus_order(node, props.focus_order);
      write_access(node, &props.access);
    }

    write_referenced_ids(tree, &props.access);
  });
}

impl Painting for Web {
  fn paint(&self, _sink: &u32, _painter: Box<Painter>) {}

  fn available(&self) -> ReadSignal<bool> {
    with_media(|media| media.absent)
  }
}

impl Platform for Web {
  type Scalar = f32;
  type Sink = u32;
  type StyleExt = GuiExt;
  type TokenValue = WebTokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(&self, props: &BoxProps<Self>) -> u32 {
    let (index, compiled) = with_tree(|tree| {
      let index = tree.element(box_tag(props.role, props.destination.is_some()));

      if let Some(node) = tree.node(index) {
        if let Some(role) = props.role.filter(|role| role.is_spelled()) {
          node.set_attribute("role", aria_role(role));
        }

        if let Some(href) = props
          .destination
          .as_deref()
          .filter(|href| !runs_script(href))
        {
          node.set_attribute("href", href);
        }
      }

      let compiled = tree.compiled(box_fingerprint(props, 0), || compile_box(props));

      (index, compiled)
    });

    finish_box(index, props, &compiled);

    index
  }

  fn update_box(&self, sink: &u32, slot: Slot, leaf: &StaticLeaf<BoxValue<f32, GuiExt>>) {
    with_node(*sink, |node| {
      let binding = node
        .slots
        .iter()
        .find(|binding| binding.slot == slot)
        .cloned();

      if let Some(binding) = binding {
        node.set_inline(
          binding.custom_property(),
          render_box_leaf(binding.property, leaf),
        );
      }
    });
  }

  fn update_focus_order(&self, sink: &u32, focus_order: Option<FocusOrder>) {
    with_tree(|tree| {
      let target = tree.carrier(*sink);

      if let Some(node) = tree.node(target) {
        write_focus_order(node, focus_order);
      }
    });
  }

  fn update_access(&self, sink: &u32, access: &[AccessProp]) {
    with_tree(|tree| {
      let target = tree.carrier(*sink);

      if let Some(node) = tree.node(target) {
        write_access(node, access);
      }

      write_referenced_ids(tree, access);
    });
  }

  fn create_control(&self, props: &ControlProps<Self>) -> u32 {
    let (index, compiled) = with_tree(|tree| {
      let index = if let Some(native) = native_element(props.kind) {
        if native.is_bare() {
          let root = tree.element(native.tag);

          if let Some(node) = tree.node(root) {
            node.write_native_attributes(native, props);
            node.bare = true;
          }

          root
        }
        else {
          let root = tree.element("label");
          let input = tree.element(native.tag);

          if let Some(node) = tree.node(input) {
            node.write_native_attributes(native, props);
            node.set_attribute("class", NATIVE_CLASS);
          }

          tree.attach(root, input, None);

          if let Some(node) = tree.node(root) {
            node.carrier = Some(input);
          }

          root
        }
      }
      else {
        let (tag, attribute) = drawn_element(props.kind);
        let root = tree.element(tag);

        if let Some(node) = tree.node(root) {
          if let Some((name, value)) = attribute {
            node.set_attribute(name, value);
          }

          if let Some(label) = &props.label {
            node.set_attribute("aria-label", label);
          }
        }

        root
      };

      if let Some(node) = tree.node(index) {
        node.control = true;
        node.set_attribute(CONTROL_ATTRIBUTE, "");
      }

      let compiled = tree.compiled(box_fingerprint(&props.box_props, 1), || {
        compile_control(props)
      });

      (index, compiled)
    });

    finish_box(index, &props.box_props, &compiled);

    index
  }

  fn set_control_state(&self, sink: &u32, state: &ControlState) {
    with_tree(|tree| {
      let root = *sink;
      let carrier = tree.carrier(root);

      if carrier == root {
        if let Some(node) = tree.node(root) {
          let disabled_natively = takes_disabled(node.tag());

          node.set_flag("data-skene-checked", state.checked == Toggle::On);
          node.set_flag("data-skene-indeterminate", state.checked == Toggle::Mixed);
          node.set_flag("data-skene-disabled", state.disabled);
          node.set_flag("data-skene-invalid", state.invalid);

          if disabled_natively {
            node.set_flag("disabled", state.disabled);
          }
        }
      }
      else if let Some(node) = tree.node(carrier) {
        node.set_flag("checked", state.checked == Toggle::On);
        node.set_flag("indeterminate", state.checked == Toggle::Mixed);
        node.set_flag("disabled", state.disabled);

        if state.invalid {
          node.set_attribute("aria-invalid", "true");
        }
        else {
          node.remove_attribute("aria-invalid");
        }
      }

      if let Some(node) = tree.node(root) {
        node.set_flag("data-skene-open", state.open);
      }
    });
  }

  fn create_overlay(&self, props: &OverlayProps<Self>) -> u32 {
    let (index, compiled) = with_tree(|tree| {
      let index = if props.modal {
        tree.element("dialog")
      }
      else {
        let index = tree.element("div");

        if let Some(node) = tree.node(index) {
          node.set_attribute("popover", if props.hint { "hint" } else { "manual" });

          // A hint is a tooltip; a popup takes the role its box asks for.
          if props.hint {
            node.set_attribute("role", "tooltip");
          }
          else if let Some(role) = props.box_props.role.filter(|role| role.is_spelled()) {
            node.set_attribute("role", aria_role(role));
          }
        }

        index
      };
      let compiled = tree.compiled(overlay_fingerprint(props), || compile_overlay(props));

      (index, compiled)
    });

    finish_box(index, &props.box_props, &compiled);

    index
  }

  fn set_overlay_open(&self, _: &u32, _: bool, _: Option<&u32>) {}

  fn create_run(&self, props: &RunProps<Self>) -> u32 {
    with_tree(|tree| {
      let index = match props.link.as_deref() {
        Some(href) => {
          let index = tree.element("a");

          if let Some(node) = tree.node(index) {
            for (name, value) in link_attributes(href, props.link_policy) {
              node.set_attribute(name, &value);
            }
          }

          index
        }
        None => tree.element("span"),
      };
      let compiled = tree.compiled(run_fingerprint(props), || compile_run(props));

      tree.assign_style(index, &compiled);

      index
    })
  }

  fn update_run(&self, sink: &u32, slot: Slot, leaf: &StaticLeaf<RunValue<GuiExt>>) {
    with_node(*sink, |node| {
      let binding = node
        .slots
        .iter()
        .find(|binding| binding.slot == slot)
        .cloned();

      if let Some(binding) = binding {
        node.set_inline(
          binding.custom_property(),
          render_run_leaf(binding.property, leaf),
        );
      }
    });
  }

  fn set_text(&self, sink: &u32, text: &str) {
    with_node(*sink, |node| node.text = Some(text.to_owned()));
  }

  fn create_anchor(&self) -> u32 {
    with_tree(|tree| {
      let ordinal = tree.anchors;

      tree.anchors += 1;

      tree.create(Node::comment(anchor_text(ordinal)))
    })
  }

  fn create_scope(&self) -> u32 {
    with_tree(|tree| {
      let index = tree.element("div");

      if let Some(node) = tree.node(index) {
        node.set_attribute(SCOPE_ATTRIBUTE, "");
        node.set_inline("display".to_owned(), "contents".to_owned());
      }

      index
    })
  }

  fn bind_node(&self, sink: &u32, node: NodeId) {
    with_tree(|tree| {
      if let Some(record) = tree.node(*sink) {
        record.realizes = Some(node);
      }
    });
  }

  fn attach(&self, parent: &u32, child: &u32, before: Option<&u32>) {
    with_tree(|tree| {
      if !tree.attach_caption(*parent, *child) {
        tree.attach(*parent, *child, before.copied());
      }
    });
  }

  fn detach(&self, parent: &u32, child: &u32) {
    with_tree(|tree| {
      if !tree.detach_caption(*parent, *child) {
        tree.detach(*parent, *child);
      }
    });
  }

  fn destroy(&self, sink: u32) {
    with_tree(|tree| tree.destroy(sink));
  }

  fn define_tokens(&self, scope: &u32, definitions: &Definitions<WebTokenValue>) {
    with_tree(|tree| {
      tree.register(definitions);

      let previous = tree.node(*scope).and_then(|node| node.token_class.clone());

      if previous.is_some() && all_unconditional(definitions) {
        if let Some(node) = tree.node(*scope) {
          for (property, value) in inline_definitions(definitions) {
            node.set_inline(property, value);
          }
        }

        return;
      }

      let class = tree.swap_class(&compile_definitions(definitions), previous.as_deref());

      if let Some(node) = tree.node(*scope) {
        node.token_class = Some(class);
      }
    });
  }

  fn define_root_tokens(&self, definitions: &Definitions<WebTokenValue>) {
    with_tree(|tree| {
      tree.register(definitions);

      let previous = tree.root_class.take();
      let class = tree.swap_class(&compile_root_definitions(definitions), previous.as_deref());

      tree.root_class = Some(class);
    });
  }

  fn listen(&self, _: &u32, _: EventKind) {}

  fn focus(&self, _: &u32) {}

  fn blur(&self, _: &u32) {}

  fn set_value(&self, sink: &u32, value: &str) {
    with_tree(|tree| {
      let target = tree.carrier(*sink);

      if let Some(node) = tree.node(target) {
        if node.tag() == "textarea" {
          node.text = Some(value.to_owned());
        }
        else if node.tag() == "select" {
          node.chosen = Some(value.to_owned());
        }
        else {
          node.set_attribute("value", value);
        }
      }
    });
  }

  fn set_options(&self, sink: &u32, options: &[(String, String)]) {
    with_tree(|tree| {
      let target = tree.carrier(*sink);

      if let Some(node) = tree.node(target) {
        node.options = options.to_vec();
      }
    });
  }

  fn set_selection(&self, _: &u32, _: TextSelection) {}

  fn set_checked(&self, sink: &u32, checked: bool) {
    with_tree(|tree| {
      let target = tree.carrier(*sink);

      if let Some(node) = tree.node(target) {
        node.set_flag("checked", checked);
      }
    });
  }

  fn observe_size(&self, _: &u32, _: Rc<dyn Fn(Size<f32>)>) {}

  fn media(&self) -> MediaSignals<f32> {
    media_signals()
  }

  fn hydration(&self) -> Option<ReadSignal<bool>> {
    Some(with_media(|media| media.absent))
  }

  fn set_scheme(&self, scheme: Option<ColorScheme>) {
    set_scheme(scheme);
  }

  /// The server renders once, so nothing scheduled ever runs.
  fn schedule(&self, _: Duration, _: Box<dyn FnOnce()>) -> Timer {
    Timer(0)
  }

  fn cancel(&self, _: Timer) {}

  fn stored_scheme(&self) -> Option<ColorScheme> {
    stored_scheme()
  }
}

impl WebMarkup for Web {
  fn create_element(&self, tag: &'static str) -> u32 {
    with_tree(|tree| tree.element(tag))
  }

  fn set_attribute(&self, sink: &u32, name: &str, value: &str) {
    with_node(*sink, |node| node.set_attribute(name, value));
  }

  fn set_declarations(&self, sink: &u32, declarations: &Declarations) {
    with_node(*sink, |node| {
      for (property, value) in declarations.iter() {
        node.set_inline(property.to_owned(), value.to_owned());
      }
    });
  }

  fn set_text(&self, sink: &u32, text: &str) {
    with_node(*sink, |node| node.text = Some(text.to_owned()));
  }
}

/// Marks the page as one whose reader may choose a scheme, so its head carries the
/// blocking script applying the stored choice before the first paint; the choice
/// itself is the reader's and never known to a server.
pub fn set_scheme(_scheme: Option<ColorScheme>) {
  PREEMPT_SCHEME.with(|preempt| preempt.set(true));
}

/// The color scheme the reader chose on a previous visit; never known to a server.
#[inline(always)]
#[must_use]
pub const fn stored_scheme() -> Option<ColorScheme> {
  None
}

/// Opts the page into view transitions: the stylesheet carries
/// `@view-transition { navigation: auto }`, so a browser that has them animates the
/// cross-document navigations between the served pages; one that does not cuts.
pub fn enable_view_transitions() {
  VIEW_TRANSITIONS.with(|enabled| enabled.set(true));
}

/// Sets the document title, serialized into the page head.
pub fn set_document_title(title: &str) {
  with_tree(|tree| tree.title = Some(title.to_owned()));
}

/// Sets the page's language, a BCP 47 tag written as its root's `lang`.
pub fn set_document_language(language: &str) {
  with_tree(|tree| tree.language = Some(language.to_owned()));
}

/// The opening of a served document up to its body's content: the doctype, the root in
/// `language` when one is set, and the head, which starts with the character set and
/// the viewport and holds `head` and `styles` after them.
#[must_use]
pub fn document_opening(language: Option<&str>, head: &str, styles: &str) -> String {
  let mut out = String::from("<!DOCTYPE html><html");

  if let Some(language) = language {
    out.push_str(" lang=\"");
    escape_attr(language, &mut out);
    out.push('"');
  }

  out.push_str(
    "><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
     content=\"width=device-width, initial-scale=1\">",
  );
  out.push_str(head);
  out.push_str(styles);
  out.push_str("</head><body>");

  out
}

/// The closing of a served document: the hydration `state` in the script the client
/// reads it from, and the end of the body and the document.
#[must_use]
pub fn document_closing(state: &str) -> String {
  format!(
    "<script id=\"{STATE_SCRIPT_ID}\" type=\"application/json\">{}</script></body></html>",
    state.replace('<', "\\u003c"),
  )
}

/// Adds an element to the page head, serialized in registration order.
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) {
  with_tree(|tree| {
    tree.head.push((
      tag.to_owned(),
      attributes
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect(),
    ));
  });
}

/// A server render, in parts: the stylesheet, the head, the body markup, the hydration
/// state and the page's language.
#[must_use]
pub struct Rendered {
  styles: String,
  head: String,
  body: String,
  state: String,
  language: Option<String>,
}

impl Rendered {
  /// The `<style>` block: the reset, the slot and token registrations, and the interned
  /// rules in interning order.
  #[inline(always)]
  #[must_use]
  pub fn styles(&self) -> &str {
    &self.styles
  }

  /// The registered head content: the title and the elements added through
  /// [`add_head_element`].
  #[inline(always)]
  #[must_use]
  pub fn head(&self) -> &str {
    &self.head
  }

  /// The settled view tree: the page body.
  #[inline(always)]
  #[must_use]
  pub fn body(&self) -> &str {
    &self.body
  }

  /// The encoded resource state the client adopts on hydration.
  #[inline(always)]
  #[must_use]
  pub fn state(&self) -> &str {
    &self.state
  }

  /// The page's language, when the app set one.
  #[inline(always)]
  #[must_use]
  pub fn language(&self) -> Option<&str> {
    self.language.as_deref()
  }

  /// A complete HTML document: [`document_opening`], the tree, and
  /// [`document_closing`].
  #[must_use]
  pub fn page(&self) -> String {
    let mut page = document_opening(self.language(), &self.head, &self.styles);

    page.push_str(&self.body);
    page.push_str(&document_closing(&self.state));

    page
  }
}

/// Registers the segment `start` begins as a deferred boundary: a streaming render
/// flushes what precedes it, waits until `settled` reports true, then emits its
/// content in place. A whole render never consults it.
pub fn register_boundary(start: NodeId, settled: BoundarySettled) {
  if let Some(sink) = Scene::<Web>::with(|scene| scene.sink(start)) {
    BOUNDARIES.with(|boundaries| boundaries.borrow_mut().insert(sink, settled));
  }
}

/// Whether `index` starts a boundary whose content has not settled: the streaming
/// walk's pause test.
fn boundary_pending(index: u32) -> bool {
  BOUNDARIES.with(|boundaries| {
    boundaries
      .borrow()
      .get(&index)
      .is_some_and(|settled| !settled())
  })
}

/// Starts a render clean: a fresh tree and scene, the root node, and the scope the app
/// builds under.
fn begin_render() -> (NodeId, Owner) {
  TREE.with(|tree| *tree.borrow_mut() = Tree::new());
  Scene::<Web>::with(|scene| *scene = Scene::new());
  PREEMPT_SCHEME.with(|preempt| preempt.set(false));
  VIEW_TRANSITIONS.with(|enabled| enabled.set(false));
  BOUNDARIES.with(|boundaries| boundaries.borrow_mut().clear());
  crate::hydration::reset_sources();
  forget_static_routes();
  task::install_executor();

  let root = Scene::<Web>::with(|scene| scene.create(NodeKind::Box, ROOT));

  (root, skene_core::root_owner().child())
}

/// Builds `app` under `scope` as the child of `root`, with `boundary` in context for
/// the tasks it starts when one is given.
fn build_app<V, F>(root: NodeId, scope: Owner, boundary: Option<Boundary>, app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
{
  scope.enter(|| {
    if let Some(boundary) = boundary {
      provide_context(boundary);
    }

    let node = app().into_view().build();

    Scene::<Web>::with(|scene| scene.attach(root, node, None));
  });
}

/// Tears a render down so the next one starts clean.
fn end_render(scope: Owner) {
  scope.dispose();
  task::clear();
  crate::hydration::reset_sources();
  BOUNDARIES.with(|boundaries| boundaries.borrow_mut().clear());
  Scene::<Web>::with(|scene| *scene = Scene::new());
  TREE.with(|tree| *tree.borrow_mut() = Tree::new());
}

/// Renders `app` over the new tree: builds it under a fresh scope, drives its async
/// work to a fixpoint, serializes the settled tree with its stylesheet, head and
/// resource state, and tears the render down so the next call starts clean.
pub fn render_to_string<V, F>(app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
{
  let (root, scope) = begin_render();

  build_app(root, scope, None, app);
  task::stabilize();

  let state = crate::hydration::collect_state();
  let (styles, head, body, language) = with_tree(|tree| {
    (
      tree.render_styles(),
      tree.render_head(),
      tree.render_body(),
      tree.language.clone(),
    )
  });

  end_render(scope);

  Rendered {
    styles,
    head,
    body,
    state,
    language,
  }
}

/// One piece of a streamed render, in emission order.
pub enum StreamPiece<'a> {
  /// The settled shell's styles, head and language, before any body chunk.
  Shell {
    /// The `<style>` block.
    styles: &'a str,
    /// The registered head content.
    head: &'a str,
    /// The page's language, when the app set one.
    language: Option<&'a str>,
  },
  /// One body chunk: a `<style>` delta for what was interned since the previous
  /// flush, when anything was, then the markup up to the next deferred boundary, or
  /// after the last.
  Chunk(String),
  /// The encoded hydration state, once the walk completes.
  Tail(String),
}

/// Renders `app` as an in-order stream, each piece handed to `flush` as it is
/// produced: the shell once it settles, a body chunk at each deferred boundary, and
/// the hydration state last.
///
/// The shell settles against a boundary of its own and its styles render, then the
/// body walks in document order. At each deferred boundary the walk flushes the chunk
/// built so far, blocks until that boundary's content settles, and resumes emitting it
/// in place, opening the chunk with the styles interned since the previous flush, so a
/// client with no JavaScript receives a correct, styled document progressively, each
/// chunk appended to the last.
pub fn render_to_stream_with<V, F>(app: F, mut flush: impl FnMut(StreamPiece<'_>))
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
{
  let (root, scope) = begin_render();
  let shell = Boundary::new();

  build_app(root, scope, Some(shell), app);
  task::stabilize_until(move || shell.is_settled());

  let (styles, head, language) = with_tree(|tree| {
    (
      tree.render_styles(),
      tree.render_head(),
      tree.language.clone(),
    )
  });

  flush(StreamPiece::Shell {
    styles: &styles,
    head: &head,
    language: language.as_deref(),
  });

  let mut cursor = vec![Frame {
    index: ROOT,
    next_child: 0,
  }];

  // TODO: once HTML's declarative out-of-order patching (`<template for>`) is Baseline
  // widely available, stream each deferred boundary as soon as it settles rather than in
  // document order, behind a switch that is on by default and that an app may turn off.
  loop {
    let mut chunk = String::new();
    let pending = with_tree(|tree| {
      chunk.push_str(&tree.render_style_delta());

      tree.stream(&mut cursor, &mut chunk, boundary_pending)
    });

    flush(StreamPiece::Chunk(chunk));

    let Some(index) = pending
    else {
      break;
    };

    task::stabilize_until(|| !boundary_pending(index));
    // Dropped whether or not it settled: settled emits its content, unsettled emits
    // what it has, and either way the walk must not pause on it again.
    BOUNDARIES.with(|boundaries| boundaries.borrow_mut().remove(&index));
  }

  flush(StreamPiece::Tail(crate::hydration::collect_state()));
  end_render(scope);
}
