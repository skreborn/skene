//! The browser leaf's realization of the platform: a DOM sink under one mount root, the
//! stylesheet mirrored from the shared interner, root-delegated events routed through
//! core's dispatch, hydration of the server leaf's markup, and the environment as
//! signals.
//!
//! One record per node sits beside the scene, holding the DOM node, the native
//! element carrying a control's state, the classes and slots the node was compiled to,
//! and what it listens for; the scene alone holds structure. Elements are the same
//! choices the server leaf makes, so a served page adopts by its creation-order
//! markers and its anchor comments without a rebuild, keeping the scroll offsets and
//! the focus the reader gave it before the bundle arrived.
//!
//! Content leaving a segment (a `derived` rebuild, a lazy placeholder discarded)
//! leaves its scroll offsets and focus behind on the segment's end anchor, whose node
//! id is stable across the rebuild; the content that next enters the segment with the
//! same shape takes them back.

mod canvas;
mod dev;
mod events;
mod head;
mod history;
mod keep;
mod media;
mod page;
mod platform;
mod restore;
mod sheet;
mod task;
mod view_transition;

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use rustc_hash::FxHashMap;
use skene_dev::STATE_SCRIPT_ID;
use skene_foundation::{EventKind, NodeId, NodeKind, Realization, Scene, Size, Timer, View};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element, HtmlElement, ResizeObserver, ResizeObserverEntry};

pub use self::{
  dev::boot_with_dev_state,
  head::{add_head_element, remove_head_entry, set_document_language},
  history::{current_path, on_pop, push_path},
  media::{media_query, persisted_scheme, set_scheme, watch_media},
  restore::adopt_restoration,
  task::{drive, install_executor},
  view_transition::{commit_navigation, enable_view_transitions, view_transitions_enabled},
};
use self::{
  events::{Entry, Events, Listener, report_shown},
  keep::{Kept, Mover, Saved},
  page::{Hydration, Page},
  sheet::Sheet,
};
use super::{
  interner::CLASS_PREFIX,
  markup::{MARKER_ATTRIBUTE, STYLE_ID},
  platform::web::Web,
  realize::{Realized, Realizer, Record},
  token::WebTokenValue,
};

/// The DOM node type of a comment.
const COMMENT_NODE: u16 = 8;

/// The shared size observer with its live callback.
type SizeObserver = (ResizeObserver, Closure<dyn FnMut(js_sys::Array)>);

/// One observer of a node's size, with the size to report to it.
type SizeReport = (Rc<dyn Fn(Size<f32>)>, Size<f32>);

/// What the browser keeps per node beside the shared record.
#[derive(Default)]
struct Extra {
  /// The reports of the box's size.
  observers: Vec<Rc<dyn Fn(Size<f32>)>>,
  /// A modal's `cancel` listener, reporting a dismissal.
  cancel: Option<Listener>,
  /// The canvas a painted box paints into.
  canvas: Option<Box<canvas::PaintedCanvas>>,
}

/// A node's record in the browser.
type Node = Record<Page>;

impl Record<Page> {
  /// The node's element; `None` for an anchor.
  fn dom_element(&self) -> Option<&Element> {
    self.element.dyn_ref::<Element>()
  }

  /// The node's element as an HTML element; `None` for an anchor.
  fn html(&self) -> Option<&HtmlElement> {
    self.element.dyn_ref::<HtmlElement>()
  }

  /// The element carrying the node's state: its native input, else itself; `None` for
  /// an anchor.
  fn carrier_element(&self) -> Option<&Element> {
    self.carrier().dyn_ref::<Element>()
  }

  /// Sets the element's classes to exactly the node's own, dropping any other.
  fn write_classes(&self) {
    let Some(element) = self.dom_element()
    else {
      return;
    };
    let classes: Vec<&str> = [self.style_class.as_deref(), self.token_class.as_deref()]
      .into_iter()
      .flatten()
      .collect();

    if classes.is_empty() {
      let _ = element.remove_attribute("class");
    }
    else {
      let _ = element.set_attribute("class", &classes.join(" "));
    }
  }

  /// Removes from a native view's element every compiled class that is not the node's
  /// own, keeping the view's own classes.
  fn write_native_classes(&self) {
    let Some(element) = self.dom_element()
    else {
      return;
    };
    let list = element.class_list();
    let own = [self.style_class.as_deref(), self.token_class.as_deref()];
    let stale: Vec<String> = (0..list.length())
      .filter_map(|index| list.item(index))
      .filter(|class| class.starts_with(CLASS_PREFIX) && !own.contains(&Some(class.as_str())))
      .collect();

    for class in stale {
      let _ = list.remove_1(&class);
    }
  }
}

/// The browser leaf's state: the realization of the scene into the document, the
/// delegated events and what the page keeps across its changes.
struct Dom {
  /// The realization: the node records, the document and the stylesheet.
  realizer: Realizer<Page>,
  /// The delegated events.
  events: Events,
  /// How many nodes the last hydration failed to adopt.
  mismatches: u32,
  /// The shared size observer, made when the first size is observed.
  resize: Option<SizeObserver>,
  /// The scene node of the mount root.
  root_node: NodeId,
  /// What discarded markup held, by the anchor following it, for the content taking
  /// its place.
  saved: FxHashMap<NodeId, Saved>,
  /// What each hidden node's subtree held, for its return.
  kept: FxHashMap<NodeId, Kept>,
  /// The engine's state-keeping move.
  mover: Mover,
  /// The observer reporting each canvas's size, made when the first box paints.
  pixels: Option<canvas::Pixels>,
  /// The overlays asked open before their element was connected, each with its anchor.
  pending_open: FxHashMap<NodeId, Option<NodeId>>,
  /// The timeouts of the scheduled actions.
  timers: FxHashMap<Timer, Timeout>,
  /// The id the next scheduled action takes.
  next_timer: u64,
}

/// A scheduled action: the window's timeout, its callback kept alive until the entry
/// is dropped, and whether it has run.
struct Timeout {
  handle: i32,
  _callback: Closure<dyn FnMut()>,
  fired: Rc<Cell<bool>>,
}

thread_local! {
  static SCENE: RefCell<Scene<Web>> = RefCell::new(Scene::new());
  static DOM: RefCell<Option<Dom>> = const { RefCell::new(None) };
}

/// Ends the turn now: runs the flush its writes scheduled, which otherwise runs at the
/// next microtask checkpoint.
///
/// An event, a mount and a hydration each end their own turn; a test calls this where
/// it reads the page after a direct write.
pub fn end_turn() {
  skene_foundation::end_turn();
}

/// Whether the leaf's own state is borrowed right now, so an event reaching it must
/// wait for the borrow's release.
fn is_dom_borrowed() -> bool {
  DOM.with(|dom| dom.try_borrow_mut().is_err())
}

/// Runs `operation` on the installed state, if the browser leaf is installed; nothing
/// of the user's runs inside it.
fn try_with_dom<R>(operation: impl FnOnce(&mut Dom) -> R) -> Option<R> {
  DOM.with(|dom| dom.borrow_mut().as_mut().map(operation))
}

/// Runs `operation` on the installed state; nothing of the user's runs inside it.
///
/// # Panics
///
/// Panics if the browser leaf is not installed for this thread.
fn with_dom<R>(operation: impl FnOnce(&mut Dom) -> R) -> R {
  DOM.with(|dom| {
    operation(
      dom
        .borrow_mut()
        .as_mut()
        .expect("the web platform to be installed for this thread"),
    )
  })
}

impl Dom {
  fn new(
    document: Document,
    root: Element,
    root_node: NodeId,
    hydration: Option<Hydration>,
  ) -> Self {
    let head = document.head().expect("a head");
    let sheet = match hydration
      .as_ref()
      .and_then(|_| document.get_element_by_id(STYLE_ID))
    {
      Some(block) => Sheet::adopt(&document, &head, block),
      None => Sheet::new(&document, &head),
    };
    let mut events = Events::new(root.clone());

    events.ensure(EventKind::Focus);
    events.ensure(EventKind::Blur);

    let mut realizer = Realizer::new(
      Page {
        document,
        hydration,
        markers: 1,
        composing: None,
      },
      sheet,
    );

    realizer.insert(root_node, root.into(), "", Realized::Box, false);

    Self {
      realizer,
      events,
      mismatches: 0,
      resize: None,
      root_node,
      saved: FxHashMap::default(),
      kept: FxHashMap::default(),
      mover: Mover::detect(),
      pixels: None,
      pending_open: FxHashMap::default(),
      timers: FxHashMap::default(),
      next_timer: 0,
    }
  }

  /// The document the app mounts in.
  #[inline(always)]
  const fn document(&self) -> &Document {
    &self.realizer.sink.document
  }

  fn node(&self, sink: NodeId) -> Option<&Node> {
    self.realizer.records.get(sink)
  }

  fn node_mut(&mut self, sink: NodeId) -> Option<&mut Node> {
    self.realizer.records.get_mut(sink)
  }

  /// Takes the overlays asked open before their element was connected whose element now
  /// is, each with its anchor.
  fn take_connected_opens(&mut self) -> Vec<(NodeId, Option<NodeId>)> {
    if self.pending_open.is_empty() {
      return Vec::new();
    }

    let connected: Vec<_> = self
      .pending_open
      .iter()
      .filter(|(sink, _)| {
        self
          .node(**sink)
          .and_then(Node::dom_element)
          .is_some_and(|element| element.is_connected())
      })
      .map(|(sink, anchor)| (*sink, *anchor))
      .collect();

    for (sink, _) in &connected {
      self.pending_open.remove(sink);
    }

    connected
  }

  /// Places `child` in the caption `<label>` of `parent` where it is the caption the
  /// control `parent` keeps beside it, the label before the control among its
  /// siblings; `false` for any other pair, which attaches as usual.
  fn place_caption(&mut self, parent: NodeId, child: NodeId) -> bool {
    let Some(label) = self.realizer.enter_caption(parent, child)
    else {
      return false;
    };

    if let Some(control) = self.node(parent)
      && let Some(container) = control.element.parent_node()
      && label.parent_node().as_ref() != Some(&container)
    {
      let _ = container.insert_before(&label, Some(&control.element));
    }

    if let Some(child) = self.node(child)
      && child.element.parent_node().as_ref() != Some(&label)
    {
      let _ = label.append_child(&child.element);
    }

    true
  }

  /// What a node placed under `parent` before `before` is inserted before: the caption
  /// `<label>` `before` keeps beside it under `parent`, else its element; `None` to
  /// append.
  fn insertion_point(
    &self,
    parent: &web_sys::Node,
    before: Option<NodeId>,
  ) -> Option<web_sys::Node> {
    let node = self.node(before?)?;

    Some(
      node
        .caption
        .clone()
        .filter(|caption| caption.parent_node().as_ref() == Some(parent))
        .unwrap_or_else(|| node.element.clone()),
    )
  }

  /// Takes `child` out of the caption `<label>` it sits in, removing the label from the
  /// document; `false` where it sits in none.
  fn lift_caption(&mut self, child: NodeId) -> bool {
    let Some((_, label)) = self.realizer.leave_caption(child)
    else {
      return false;
    };

    if let Some(child) = self.node(child)
      && child.element.parent_node().as_ref() == Some(&label)
    {
      let _ = label.remove_child(&child.element);
    }

    if let Some(container) = label.parent_node() {
      let _ = container.remove_child(&label);
    }

    true
  }

  /// The shared observer, created on first use.
  fn resize_observer(&mut self) -> ResizeObserver {
    if let Some((observer, _)) = &self.resize {
      return observer.clone();
    }

    let closure = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
      let mut reports: Vec<SizeReport> = Vec::new();

      for entry in entries.iter() {
        let entry: ResizeObserverEntry = entry.unchecked_into();
        let target = entry.target();
        let Some(sink) = events::node_of(&target)
        else {
          continue;
        };

        #[expect(
          clippy::cast_precision_loss,
          reason = "an element's size in CSS pixels is precise enough in f32"
        )]
        let size = Size {
          width: target.client_width() as f32,
          height: target.client_height() as f32,
        };

        with_dom(|dom| {
          if let Some(node) = dom.node(sink) {
            reports.extend(
              node
                .extra
                .observers
                .iter()
                .map(|observer| (Rc::clone(observer), size)),
            );
          }
        });
      }

      for (report, size) in reports {
        report(size);
      }
    });
    let observer =
      ResizeObserver::new(closure.as_ref().unchecked_ref()).expect("a resize observer");

    self.resize = Some((observer.clone(), closure));

    observer
  }

  /// The anchor realized as the comment `node`, if any.
  fn anchor_sink(&self, node: &web_sys::Node) -> Option<NodeId> {
    if node.node_type() != COMMENT_NODE {
      return None;
    }

    self.realizer.records.iter().find_map(|(anchor, record)| {
      (matches!(record.realized, Realized::Anchor) && record.element == *node).then_some(anchor)
    })
  }

  /// Keeps what the discarded markup `roots` held on the anchor `following` them, for
  /// the content that next enters the segment before it.
  fn save(&mut self, roots: &[web_sys::Node], following: Option<web_sys::Node>) {
    let Some(saved) = Saved::capture(self.document(), roots)
    else {
      return;
    };

    if let Some(anchor) = following.and_then(|node| self.anchor_sink(&node)) {
      self.saved.insert(anchor, saved);
    }
  }

  /// Ends hydration: reports what the server rendered and the client never built or
  /// built differently, removes it, so the page converges on the client's tree, gives
  /// each adopted element exactly the client's classes, a native view's keeping its own
  /// beside them, and forgets which nodes were adopted.
  fn finish_hydration(&mut self) -> Vec<NodeId> {
    let Some(hydration) = self.realizer.sink.hydration.take()
    else {
      return Vec::new();
    };
    let unadopted = hydration.elements.len() + hydration.comments.len();

    if unadopted > 0 {
      tracing::error!("hydration mismatch: {unadopted} server node(s) were never adopted");
    }

    self.mismatches = hydration.mismatches + u32::try_from(unadopted).unwrap_or(u32::MAX);

    for element in hydration.elements.into_values().chain(hydration.stale) {
      element.remove();
    }

    for comment in hydration.comments.into_values() {
      if let Some(parent) = comment.parent_node() {
        let _ = parent.remove_child(&comment);
      }
    }

    for node in self.realizer.records.values_mut() {
      if node.adopted {
        if matches!(node.realized, Realized::Native) {
          node.write_native_classes();
        }
        else {
          node.write_classes();
        }
      }

      node.adopted = false;
    }

    crate::resource_state::finish();

    hydration.pending
  }
}

/// Installs the browser leaf for this thread, rooted at `root`, adopting the server's
/// nodes under it when `hydrating`.
fn install_at(root: Element, hydrating: bool) {
  skene_foundation::hold_flushes(|| wasm_bindgen_futures::spawn_local(async { end_turn() }));
  task::install_executor();

  let document = web_sys::window()
    .expect("a window")
    .document()
    .expect("a document");
  let hydration = hydrating.then(|| Hydration::seed(&root));

  media::set_live(!hydrating);
  DOM.with(|dom| {
    let mut dom = dom.borrow_mut();

    assert!(
      dom.is_none(),
      "the web platform is already installed for this thread",
    );

    *dom = Some(Dom::new(
      document,
      root,
      Scene::<Web>::with(|scene| scene.create(NodeKind::Box, None)),
      hydration,
    ));
  });
  WebTokenValue::claim_system_colors();

  restore::provide_restoration();
}

/// Installs the browser leaf for this thread, rooted at `root`: the element every
/// mounted view is inserted under.
///
/// # Panics
///
/// Panics if it is already installed, or if there is no `window` or `document`.
pub fn install(root: Element) {
  install_at(root, false);
}

/// Builds `view` under the installed root as one turn, returning its node.
///
/// The first mount opens the window: once its turn ends, the control set to autofocus
/// that comes first in the scene's child order is marked `autofocus` and takes focus,
/// or, while a hydration adopts the server's markup, keeps the focus the browser gave
/// the served element and moves none; nothing is focused where no control asked.
///
/// # Panics
///
/// Panics if the platform is not installed for this thread.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  end_turn();

  let node = skene_reactive::Owner::root().enter(|| {
    let node = view();
    let root = with_dom(|dom| dom.root_node);

    Scene::<Web>::with(|scene| scene.attach(root, node, None));

    node
  });

  end_turn();

  let (root, hydrating) = with_dom(|dom| (dom.root_node, dom.realizer.sink.hydration.is_some()));

  if let Some(opening) = Scene::<Web>::with(|scene| scene.take_autofocus(root, |_| false)) {
    with_dom(|dom| dom.realizer.mark_autofocus(opening));

    if !hydrating {
      Web::focus(opening);
      end_turn();
    }
  }

  node
}

/// Hydrates a server-rendered tree under `root`.
///
/// It adopts the tree's elements by their creation-order markers and its anchors by
/// their comments, the server's `<style>` block in place of a fresh one, and the
/// resource `state` serialized alongside them.
///
/// A native element whose live value or checked state differs from the serialized one
/// is the reader's until the tree is built, every write of its content held back, and
/// then reports what it shows as a change its toolkit applied: a text entry's text and
/// selection as an edit, repaired in place to the truth the verdict answers with, and
/// any other control's content, a radio's as its group's choice, repaired by its
/// controller's write where the truth refuses it. Every mismatch is reported to the
/// console and counted in [`hydration_mismatches`].
///
/// # Panics
///
/// Panics if the platform is already installed for this thread.
pub fn hydrate<V, F>(root: Element, state: &str, app: F)
where
  F: FnOnce() -> V + 'static,
  V: View<Web>,
{
  install_at(root, true);
  crate::resource_state::stage(state);
  let _ = mount(|| app().build());

  let pending = with_dom(Dom::finish_hydration);

  for node in pending {
    let Some(carrier) = with_dom(|dom| dom.node(node).and_then(Node::carrier_element).cloned())
    else {
      continue;
    };

    match Entry::of(&carrier) {
      Some(entry) => entry.report(node),
      None => report_shown(node, &carrier),
    }
  }

  // The adopted values have reached their signals; the enhancements may build.
  media::set_live(true);
  end_turn();
}

/// How many nodes the last hydration failed to adopt: server nodes the client built
/// differently, and server nodes the client never built.
#[must_use]
pub fn hydration_mismatches() -> u32 {
  with_dom(|dom| dom.mismatches)
}

/// Hydrates the server-rendered page: adopts `<body>` and reads the state script the
/// server's page assembly embedded.
fn hydrate_page<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: View<Web>,
{
  console_error_panic_hook::set_once();

  let _ = tracing::subscriber::set_global_default(crate::logging::LogSubscriber);

  let document = web_sys::window()
    .expect("a window")
    .document()
    .expect("a document");
  let body = document.body().expect("a body");
  let state = document
    .get_element_by_id(STATE_SCRIPT_ID)
    .and_then(|script| script.text_content())
    .unwrap_or_default();

  hydrate(body.into(), &state, app);
}

/// Mounts and runs a skene application in the browser, rooted at `<body>`.
///
/// A server-rendered page, recognized by its embedded state script, is adopted in
/// place: its `<body>` hydrated with the state the script carries.
///
/// # Panics
///
/// Panics if there is no `window`, `document` or `<body>`.
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: View<Web>,
{
  let document = web_sys::window()
    .expect("a window")
    .document()
    .expect("a document");

  if document.get_element_by_id(STATE_SCRIPT_ID).is_some() {
    hydrate_page(app);

    return;
  }

  console_error_panic_hook::set_once();

  let _ = tracing::subscriber::set_global_default(crate::logging::LogSubscriber);

  let body = document.body().expect("a body");

  install(body.into());

  let _ = mount(|| app().build());
}

/// The DOM nodes between the anchors of the segment `start` begins, in order.
fn segment_dom(start: NodeId) -> Vec<web_sys::Node> {
  let Some((start, end)) = Scene::<Web>::with(|scene| {
    let end = scene.segment_end(start)?;

    Some((start, end))
  })
  else {
    return Vec::new();
  };

  with_dom(|dom| {
    let (Some(start), Some(end)) = (
      dom.node(start).map(|node| node.element.clone()),
      dom.node(end).map(|node| node.element.clone()),
    )
    else {
      return Vec::new();
    };
    let mut between = Vec::new();
    let mut current = start.next_sibling();

    while let Some(node) = current {
      if node == end {
        break;
      }

      current = node.next_sibling();

      between.push(node);
    }

    between
  })
}

/// Claims the server's markup between the anchors of the segment `start` begins.
///
/// The markup stays in place and counts as adopted while the segment's real content is
/// on its way; `false` when not hydrating or when the server rendered nothing there.
#[must_use]
pub(crate) fn claim_lazy(start: NodeId) -> bool {
  let between = segment_dom(start);

  if between.is_empty() {
    return false;
  }

  with_dom(|dom| {
    let Some(hydration) = dom.realizer.sink.hydration.as_mut()
    else {
      return false;
    };
    let mut claimed = false;

    for node in between {
      let mut pending = vec![node];

      while let Some(node) = pending.pop() {
        if node.node_type() == COMMENT_NODE {
          hydration
            .comments
            .remove(&node.node_value().unwrap_or_default());
          claimed = true;

          continue;
        }

        if let Some(element) = node.dyn_ref::<Element>() {
          if let Some(marker) = element
            .get_attribute(MARKER_ATTRIBUTE)
            .and_then(|value| value.parse::<u32>().ok())
          {
            hydration.elements.remove(&marker);
            claimed = true;
          }

          let children = node.child_nodes();

          for index in 0..children.length() {
            if let Some(child) = children.item(index) {
              pending.push(child);
            }
          }
        }
      }
    }

    claimed
  })
}

/// Removes the claimed markup between the anchors of the segment `start` begins.
///
/// The segment's real content arrives to take its place, and a scroll offset or the
/// focus inside the markup passes to that content when it has the same shape.
pub(crate) fn discard_lazy(start: NodeId) {
  let between = segment_dom(start);
  let following = between.last().and_then(web_sys::Node::next_sibling);

  with_dom(|dom| dom.save(&between, following));

  for node in between {
    if let Some(parent) = node.parent_node() {
      let _ = parent.remove_child(&node);
    }
  }
}

/// The owning [`Document`].
#[must_use]
pub fn document() -> Document {
  with_dom(|dom| dom.document().clone())
}

/// The element realizing `node`; `None` for an anchor or a node no longer live.
#[must_use]
pub fn element(node: NodeId) -> Option<Element> {
  with_dom(|dom| dom.node(node).and_then(Node::dom_element).cloned())
}

/// Uninstalls the browser leaf: removes its stylesheet and listeners, empties the
/// scene and resets the reactive runtime, so one thread can run many independent
/// mounts.
pub fn uninstall() {
  DOM.with(|dom| {
    if let Some(mut dom) = dom.borrow_mut().take() {
      dom.realizer.book.sheet.remove_all();

      if let Some((observer, _)) = &dom.resize {
        observer.disconnect();
      }
    }
  });
  media::reset();
  view_transition::reset();
  head::reset();
  skene_foundation::reset_thread::<Web>();
}
