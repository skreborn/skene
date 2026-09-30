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
mod media;
mod restore;
mod sheet;
mod task;
mod view_transition;

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  thread::LocalKey,
  time::Duration,
};

use rustc_hash::FxHashMap;
use skene_common::gui::GuiExt;
use skene_core::{
  AccessProp,
  Availability,
  BoxProps,
  BoxValue,
  ClipboardAction,
  ColorScheme,
  ControlProps,
  ControlState,
  Definitions,
  EventKind,
  FocusOrder,
  IntoView,
  Media,
  MediaSignals,
  NodeId,
  NodeKind,
  Observation,
  OverlayProps,
  PickerOption,
  Placement,
  Platform,
  Point,
  PointerEvent,
  RunProps,
  RunValue,
  Scene,
  Size,
  Slot,
  StaticBoxStyle,
  StaticLeaf,
  TextSelection,
  Timer,
  View,
  notify_change,
  notify_dismiss,
  notify_toggle,
};
use skene_reactive::{Memo, ReadSignal};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
  Document,
  Element,
  HtmlDialogElement,
  HtmlElement,
  HtmlInputElement,
  HtmlSelectElement,
  HtmlTextAreaElement,
  ResizeObserver,
  ResizeObserverEntry,
};

pub use self::{
  dev::boot_with_dev_state,
  head::{
    add_head_element,
    add_head_title,
    remove_head_entry,
    set_document_language,
    set_head_title,
  },
  history::{current_path, on_pop, push_path},
  media::{media_query, set_scheme, stored_scheme, watch_media},
  restore::adopt_restoration,
  task::{drive, install_executor},
  view_transition::{commit_navigation, enable_view_transitions, view_transitions_enabled},
};
use self::{
  events::{Events, Listener},
  sheet::Sheet,
};
use super::{
  Web,
  markup::{FormElement, MARKER_ATTRIBUTE, STYLE_ID},
  realize::{Realized, Realizer, Record, Sink},
  token::WebTokenValue,
};
use crate::{Declarations, WebDom, WebMarkup, hydration::STATE_SCRIPT_ID};

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
}

/// What the reader changed on a native element before the client arrived.
enum Pending {
  Toggle(bool),
  Change(String),
}

/// What discarded markup held, keyed by the element's index in its document order: the
/// scrolled offsets and the focused element.
struct Saved {
  tags: Vec<String>,
  scrolls: Vec<(usize, f64, f64)>,
  focused: Option<usize>,
}

impl Saved {
  /// What the subtrees `roots` hold, in document order; `None` when nothing is
  /// scrolled or focused inside them.
  fn capture(document: &Document, roots: &[web_sys::Node]) -> Option<Self> {
    let elements = subtree_elements(roots);
    let active = document.active_element();
    let mut saved = Self {
      tags: elements.iter().map(Element::tag_name).collect(),
      scrolls: Vec::new(),
      focused: None,
    };

    for (index, element) in elements.iter().enumerate() {
      let (left, top) = (element.scroll_left(), element.scroll_top());

      if left != 0 || top != 0 {
        saved.scrolls.push((index, f64::from(left), f64::from(top)));
      }

      if active.as_ref() == Some(element) {
        saved.focused = Some(index);
      }
    }

    (!saved.scrolls.is_empty() || saved.focused.is_some()).then_some(saved)
  }

  /// Gives what was held to `root`'s subtree, when it has the shape the content had.
  fn restore(self, root: &web_sys::Node) {
    let elements = subtree_elements(std::slice::from_ref(root));

    if elements.len() != self.tags.len()
      || elements
        .iter()
        .zip(&self.tags)
        .any(|(element, tag)| element.tag_name() != *tag)
    {
      return;
    }

    // Focus lands before the offsets, which it would otherwise scroll away.
    if let Some(html) = self
      .focused
      .and_then(|index| elements.get(index))
      .and_then(|element| element.dyn_ref::<HtmlElement>())
    {
      let _ = html.focus();
    }

    for (index, left, top) in self.scrolls {
      #[expect(
        clippy::cast_possible_truncation,
        reason = "a saved scroll offset is the whole number of pixels an element reported"
      )]
      if let Some(element) = elements.get(index) {
        element.set_scroll_left(left as i32);
        element.set_scroll_top(top as i32);
      }
    }
  }
}

/// Every element under `roots`, each root first, in document order.
fn subtree_elements(roots: &[web_sys::Node]) -> Vec<Element> {
  let mut elements = Vec::new();

  for root in roots {
    let Some(element) = root.dyn_ref::<Element>()
    else {
      continue;
    };

    elements.push(element.clone());

    let descendants = element.query_selector_all("*").expect("query the subtree");

    elements.extend((0..descendants.length()).filter_map(|index| {
      descendants
        .item(index)
        .and_then(|node| node.dyn_into::<Element>().ok())
    }));
  }

  elements
}

/// What a hidden node's subtree held that leaving the document loses: the focused
/// element and the offsets of its scrolled elements.
struct Kept {
  focused: Option<HtmlElement>,
  scrolls: Vec<(Element, i32, i32)>,
}

impl Kept {
  /// What the subtree of `root`, realizing `nodes`, holds; `None` when nothing in it is
  /// focused or scrolled.
  fn capture(dom: &Dom, root: &web_sys::Node, nodes: &[NodeId]) -> Option<Self> {
    let focused = dom
      .document()
      .active_element()
      .filter(|active| root.contains(Some(active)))
      .and_then(|active| active.dyn_into::<HtmlElement>().ok());
    let scrolls: Vec<_> = nodes
      .iter()
      .filter_map(|node| dom.node(*node)?.dom_element().cloned())
      .filter_map(|element| {
        let (left, top) = (element.scroll_left(), element.scroll_top());

        (left != 0 || top != 0).then_some((element, left, top))
      })
      .collect();

    (focused.is_some() || !scrolls.is_empty()).then_some(Self { focused, scrolls })
  }

  /// Gives the focus back where it has gone nowhere since, then each element its
  /// offsets.
  fn restore(self, document: &Document) {
    // Focus lands before the offsets, which it would otherwise scroll away.
    if let Some(focused) = self.focused
      && document
        .active_element()
        .is_none_or(|active| Some(&active) == document.body().as_deref())
    {
      let _ = focused.focus();
    }

    for (element, left, top) in self.scrolls {
      element.set_scroll_left(left);
      element.set_scroll_top(top);
    }
  }
}

/// The engine's state-keeping move, `moveBefore`, where it has one.
#[derive(Clone)]
struct Mover(Option<js_sys::Function>);

impl Mover {
  /// The engine's move, looked up once.
  fn detect() -> Self {
    let function = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("Element"))
      .and_then(|element| js_sys::Reflect::get(&element, &JsValue::from_str("prototype")))
      .and_then(|prototype| js_sys::Reflect::get(&prototype, &JsValue::from_str("moveBefore")))
      .ok()
      .and_then(|function| function.dyn_into::<js_sys::Function>().ok());

    Self(function)
  }

  /// Moves `node` under `parent`, immediately before `before` when given, else last:
  /// in place, keeping its focus and animations, where the engine can, else by one
  /// insertion.
  fn move_node(
    &self,
    parent: &web_sys::Node,
    node: &web_sys::Node,
    before: Option<&web_sys::Node>,
  ) {
    let moved = self.0.as_ref().is_some_and(|function| {
      function
        .call2(parent, node, before.map_or(&JsValue::NULL, AsRef::as_ref))
        .is_ok()
    });

    if !moved {
      let _ = parent.insert_before(node, before);
    }
  }
}

/// The server's nodes awaiting adoption, and what adoption found.
struct Hydration {
  elements: FxHashMap<u32, Element>,
  comments: FxHashMap<String, web_sys::Node>,
  stale: Vec<Element>,
  mismatches: u32,
  pending: Vec<(NodeId, Pending)>,
}

impl Hydration {
  /// Indexes the server's elements under `root` by marker and its anchors by text.
  fn seed(root: &Element) -> Self {
    let mut elements = FxHashMap::default();
    let marked = root
      .query_selector_all(&format!("[{MARKER_ATTRIBUTE}]"))
      .expect("query the hydration markers");

    for index in 0..marked.length() {
      if let Some(element) = marked
        .item(index)
        .and_then(|node| node.dyn_into::<Element>().ok())
        && let Some(marker) = element
          .get_attribute(MARKER_ATTRIBUTE)
          .and_then(|value| value.parse::<u32>().ok())
      {
        elements.insert(marker, element);
      }
    }

    let mut comments = FxHashMap::default();
    let mut pending: Vec<web_sys::Node> = vec![root.clone().into()];

    while let Some(node) = pending.pop() {
      let children = node.child_nodes();

      for index in 0..children.length() {
        if let Some(child) = children.item(index) {
          if child.node_type() == COMMENT_NODE {
            comments.insert(child.node_value().unwrap_or_default(), child);
          }
          else {
            pending.push(child);
          }
        }
      }
    }

    Self {
      elements,
      comments,
      stale: Vec::new(),
      mismatches: 0,
      pending: Vec::new(),
    }
  }
}

/// The browser's sink: the document the app mounts in, with the hydration in progress.
struct Page {
  /// The document the app mounts in.
  document: Document,
  /// The hydration in progress.
  hydration: Option<Hydration>,
  /// The hydration marker the next element created takes.
  markers: u32,
}

impl Sink for Page {
  type Element = web_sys::Node;
  type Extra = Extra;
  type Sheet = Sheet;

  const TITLE_TOOLTIPS: bool = false;

  /// The server's element at this creation index while hydrating, else a fresh `tag`.
  fn create_element(&mut self, tag: &'static str) -> (web_sys::Node, bool) {
    let marker = self.markers;

    self.markers += 1;

    if let Some(hydration) = &mut self.hydration {
      match hydration.elements.remove(&marker) {
        Some(element) if element.tag_name().eq_ignore_ascii_case(tag) => {
          let _ = element.remove_attribute(MARKER_ATTRIBUTE);

          return (element.into(), true);
        }
        Some(element) => {
          hydration.mismatches += 1;
          tracing::error!(
            "hydration mismatch: server rendered <{}> where the client builds <{tag}> (marker {marker})",
            element.tag_name().to_lowercase(),
          );
          hydration.stale.push(element);
        }
        None => {
          tracing::debug!("no serialized <{tag}> for marker {marker}; building fresh");
        }
      }
    }

    (
      self
        .document
        .create_element(tag)
        .expect("an element")
        .into(),
      false,
    )
  }

  /// The server's comment of `text` while hydrating, else a fresh one.
  fn create_comment(&mut self, text: String) -> (web_sys::Node, bool) {
    if let Some(hydration) = &mut self.hydration {
      if let Some(comment) = hydration.comments.remove(&text) {
        return (comment, true);
      }

      tracing::debug!("no serialized anchor {text}; building fresh");
    }

    (self.document.create_comment(&text).into(), false)
  }

  fn bind(&mut self, element: &web_sys::Node, node: NodeId) {
    if let Some(element) = element.dyn_ref::<Element>() {
      events::stamp(element, node);
    }
  }

  fn discard(&mut self, _: &web_sys::Node) {}

  fn append_child(&mut self, parent: &web_sys::Node, child: &web_sys::Node) {
    if child.parent_node().as_ref() != Some(parent) {
      let _ = parent.append_child(child);
    }
  }

  fn set_attribute(&mut self, element: &web_sys::Node, name: &str, value: &str) {
    if let Some(element) = element.dyn_ref::<Element>() {
      let _ = element.set_attribute(name, value);
    }
  }

  fn remove_attribute(&mut self, element: &web_sys::Node, name: &str) {
    if let Some(element) = element.dyn_ref::<Element>() {
      let _ = element.remove_attribute(name);
    }
  }

  fn remove_attributes(&mut self, element: &web_sys::Node, matches: impl Fn(&str) -> bool) {
    let Some(element) = element.dyn_ref::<Element>()
    else {
      return;
    };
    let names = element.get_attribute_names();

    for index in 0..names.length() {
      if let Some(name) = names.get(index).as_string()
        && matches(&name)
      {
        let _ = element.remove_attribute(&name);
      }
    }
  }

  fn set_property(&mut self, element: &web_sys::Node, property: &str, value: &str) {
    if let Some(html) = element.dyn_ref::<HtmlElement>() {
      let _ = html.style().set_property(property, value);
    }
  }

  fn remove_property(&mut self, element: &web_sys::Node, property: &str) {
    if let Some(html) = element.dyn_ref::<HtmlElement>() {
      let _ = html.style().remove_property(property);
    }
  }

  fn custom_properties(&self, element: &web_sys::Node) -> Vec<String> {
    let Some(html) = element.dyn_ref::<HtmlElement>()
    else {
      return Vec::new();
    };
    let style = html.style();

    (0..style.length())
      .map(|index| style.item(index))
      .filter(|property| property.starts_with("--"))
      .collect()
  }

  fn text(&self, element: &web_sys::Node) -> Option<String> {
    element.text_content()
  }

  fn set_text(&mut self, element: &web_sys::Node, text: &str) {
    element.set_text_content(Some(text));
  }

  fn swap_class(&mut self, element: &web_sys::Node, previous: Option<&str>, class: &str) {
    if let Some(element) = element.dyn_ref::<Element>() {
      swap_class_attribute(element, previous, class);
    }
  }

  fn set_checked(&mut self, element: &web_sys::Node, checked: bool) {
    if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
      input.set_checked(checked);
    }
  }

  fn set_indeterminate(&mut self, element: &web_sys::Node, indeterminate: bool) {
    if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
      input.set_indeterminate(indeterminate);
    }
  }

  /// Sets the live value, keeping a focused entry's selection within it.
  fn set_value(&mut self, element: &web_sys::Node, value: &str) {
    let focused = self.document.active_element().as_ref().map(AsRef::as_ref) == Some(element);
    let length = u32::try_from(value.encode_utf16().count()).unwrap_or(u32::MAX);

    if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
      if input.value() != value {
        let kept = focused
          .then(|| Option::zip(input.selection_start().ok()?, input.selection_end().ok()?))
          .flatten();

        input.set_value(value);

        if let Some((start, end)) = kept {
          let _ = input.set_selection_range(start.min(length), end.min(length));
        }
      }
    }
    else if let Some(area) = element.dyn_ref::<HtmlTextAreaElement>() {
      if area.value() != value {
        let kept = focused
          .then(|| Option::zip(area.selection_start().ok()?, area.selection_end().ok()?))
          .flatten();

        area.set_value(value);

        if let Some((start, end)) = kept {
          let _ = area.set_selection_range(start.min(length), end.min(length));
        }
      }
    }
    else if let Some(select) = element.dyn_ref::<HtmlSelectElement>() {
      select.set_value(value);
    }
  }

  fn set_options(&mut self, element: &web_sys::Node, options: &[PickerOption]) {
    let Some(select) = element.dyn_ref::<HtmlSelectElement>()
    else {
      return;
    };

    select.set_inner_html("");

    for option in options {
      let element = self.document.create_element("option").expect("an option");

      let _ = element.set_attribute("value", &option.value);

      if option.disabled {
        let _ = element.set_attribute("disabled", "");
      }

      element.set_text_content(Some(&option.name));

      let _ = select.append_child(&element);
    }
  }

  /// Notes the change against what the server serialized, for the control to report
  /// once the tree is built.
  fn note_live_state(&mut self, node: NodeId, input: &web_sys::Node, native: FormElement) {
    let pending = if let Some(input) = input.dyn_ref::<HtmlInputElement>() {
      if native.is_toggle() {
        let serialized = input.has_attribute("checked");

        (input.checked() != serialized).then(|| Pending::Toggle(input.checked()))
      }
      else {
        let serialized = input.get_attribute("value").unwrap_or_default();

        (input.value() != serialized).then(|| Pending::Change(input.value()))
      }
    }
    else if let Some(area) = input.dyn_ref::<HtmlTextAreaElement>() {
      let serialized = area.text_content().unwrap_or_default();

      (area.value() != serialized).then(|| Pending::Change(area.value()))
    }
    else if let Some(select) = input.dyn_ref::<HtmlSelectElement>() {
      let serialized = select
        .query_selector("option[selected]")
        .ok()
        .flatten()
        .and_then(|option| option.get_attribute("value"))
        .unwrap_or_default();

      (select.value() != serialized).then(|| Pending::Change(select.value()))
    }
    else {
      None
    };

    if let (Some(pending), Some(hydration)) = (pending, &mut self.hydration) {
      hydration.pending.push((node, pending));
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
  skene_core::end_turn();
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
  /// each adopted element exactly the client's classes, and forgets which nodes were
  /// adopted.
  fn finish_hydration(&mut self) -> Vec<(NodeId, Pending)> {
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
        node.write_classes();
      }

      node.adopted = false;
    }

    crate::hydration::finish();

    hydration.pending
  }
}

/// Replaces `previous` with `class` in `element`'s class list.
fn swap_class_attribute(element: &Element, previous: Option<&str>, class: &str) {
  let list = element.class_list();

  if let Some(previous) = previous
    && previous != class
  {
    let _ = list.remove_1(previous);
  }

  let _ = list.add_1(class);
}

/// Whether the engine places a box against a CSS anchor (`position-area`), decided
/// once per page.
fn anchors_in_css() -> bool {
  thread_local! {
    static SUPPORTED: Cell<Option<bool>> = const { Cell::new(None) };
  }

  SUPPORTED.with(|cell| {
    if let Some(known) = cell.get() {
      return known;
    }

    let supported =
      web_sys::css::supports_with_value("position-area", "block-end").unwrap_or(false);

    cell.set(Some(supported));

    supported
  })
}

/// The async clipboard, where the page has one: a secure context.
fn clipboard(dom: &Dom) -> Option<web_sys::Clipboard> {
  let navigator = dom.document().default_view()?.navigator();

  js_sys::Reflect::has(&navigator, &JsValue::from_str("clipboard"))
    .ok()
    .filter(|has| *has)
    .map(|_| navigator.clipboard())
}

/// The text entry a clipboard action acts on: the active one, or the one focus left
/// for the open popup it now sits in.
fn clipboard_entry(dom: &Dom) -> Option<Element> {
  let active = dom.document().active_element()?;

  if entry_selection(&active).is_some() {
    return Some(active);
  }

  let active: &web_sys::Node = &active;
  let popups: Vec<NodeId> = Scene::<Web>::with(|scene| {
    scene
      .open_overlays()
      .iter()
      .filter(|overlay| !scene.is_modal(**overlay))
      .copied()
      .collect()
  });
  let in_popup = popups.iter().any(|sink| {
    dom
      .node(*sink)
      .is_some_and(|node| node.element.contains(Some(active)))
  });

  in_popup.then(|| dom.events.last_entry.clone()).flatten()
}

/// The selection of an `<input>` or `<textarea>`: its start and end in UTF-16 units,
/// and the text between them.
fn entry_selection(entry: &Element) -> Option<(u32, u32, String)> {
  let (value, start, end) = if let Some(input) = entry.dyn_ref::<HtmlInputElement>() {
    (
      input.value(),
      input.selection_start().ok()??,
      input.selection_end().ok()??,
    )
  }
  else if let Some(editor) = entry.dyn_ref::<HtmlTextAreaElement>() {
    (
      editor.value(),
      editor.selection_start().ok()??,
      editor.selection_end().ok()??,
    )
  }
  else {
    return None;
  };
  let units: Vec<u16> = value.encode_utf16().collect();
  let length = u32::try_from(units.len()).unwrap_or(u32::MAX);
  let start = start.min(length);
  let end = end.min(length).max(start);
  let selected = &units[start as usize..end as usize];

  Some((start, end, String::from_utf16_lossy(selected)))
}

/// The document's selection as text, if any.
fn document_selection(dom: &Dom) -> Option<String> {
  let selection = dom.document().default_view()?.get_selection().ok()??;

  Some(String::from(selection.to_string()))
}

/// Replaces the selection of an `<input>` or `<textarea>` with `text`, reporting the
/// edit as the platform would.
fn replace_entry_selection(entry: &Element, text: &str) {
  let replaced = if let Some(input) = entry.dyn_ref::<HtmlInputElement>() {
    input.set_range_text(text).is_ok()
  }
  else if let Some(editor) = entry.dyn_ref::<HtmlTextAreaElement>() {
    editor.set_range_text(text).is_ok()
  }
  else {
    false
  };

  if replaced && let Ok(event) = web_sys::Event::new("input") {
    let _ = entry.dispatch_event(&event);
  }
}

/// Pins an open popup's top-left corner to `point` in viewport pixels, shifted back into
/// the viewport where it would overflow.
fn position_at_point(popup: &HtmlElement, point: Point<f32>) {
  let style = popup.style();

  let _ = style.set_property("inset", "auto");
  let _ = style.set_property("margin", "0");
  let _ = style.set_property("position", "fixed");
  let _ = style.set_property("left", &format!("{}px", point.x));
  let _ = style.set_property("top", &format!("{}px", point.y));

  let Some(window) = web_sys::window()
  else {
    return;
  };
  let extent = |value: Result<JsValue, JsValue>| value.ok().and_then(|value| value.as_f64());
  let (Some(viewport_width), Some(viewport_height)) =
    (extent(window.inner_width()), extent(window.inner_height()))
  else {
    return;
  };
  let width = f64::from(popup.offset_width());
  let height = f64::from(popup.offset_height());
  let x = f64::from(point.x).min(viewport_width - width).max(0.);
  let y = f64::from(point.y).min(viewport_height - height).max(0.);

  let _ = style.set_property("left", &format!("{x}px"));
  let _ = style.set_property("top", &format!("{y}px"));
}

/// Positions an open popup against its anchor: below it for `Bottom`, above it for
/// `Top`, start-aligned either way; other placements keep their viewport position.
///
/// Where the engine has CSS anchor positioning the anchor is named after its sink and
/// the popup follows it through `position-area`; elsewhere the popup is placed at the
/// anchor's measured rectangle as it opens.
fn position_at_anchor(popup: &HtmlElement, anchor: &Element, sink: NodeId, placement: Placement) {
  let area = match placement {
    Placement::Top => "block-start span-inline-end",
    Placement::Bottom => "block-end span-inline-end",
    _ => return,
  };
  let style = popup.style();

  let _ = style.set_property("inset", "auto");
  let _ = style.set_property("margin", "0");
  let _ = style.set_property("position", "fixed");

  if anchors_in_css()
    && let Some(anchor) = anchor.dyn_ref::<HtmlElement>()
  {
    let name = format!("--skene-anchor-{}-{}", sink.index(), sink.generation());

    let _ = anchor.style().set_property("anchor-name", &name);
    let _ = style.set_property("position-anchor", &name);
    let _ = style.set_property("position-area", area);

    return;
  }

  let rect = anchor.get_bounding_client_rect();
  let top = match placement {
    Placement::Top => rect.top() - f64::from(popup.offset_height()),
    _ => rect.bottom(),
  };

  let _ = style.set_property("left", &format!("{}px", rect.left()));
  let _ = style.set_property("top", &format!("{top}px"));
}

impl Platform for Web {
  type Scalar = f32;
  type StyleExt = GuiExt;
  type TokenValue = WebTokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(sink: NodeId, props: &BoxProps<Self>) {
    with_dom(|dom| dom.realizer.create_box(sink, props));
  }

  fn update_box(sink: NodeId, slot: Slot, leaf: &StaticLeaf<BoxValue<f32, GuiExt>>) {
    with_dom(|dom| dom.realizer.update_box(sink, slot, leaf));
  }

  fn update_focus_order(sink: NodeId, focus_order: Option<FocusOrder>) {
    with_dom(|dom| dom.realizer.update_focus_order(sink, focus_order));
  }

  fn update_access(sink: NodeId, access: &[AccessProp]) {
    with_dom(|dom| dom.realizer.update_access(sink, access));
  }

  fn create_control(sink: NodeId, props: &ControlProps<Self>) {
    with_dom(|dom| dom.realizer.create_control(sink, props));
  }

  fn set_control_state(sink: NodeId, state: &ControlState) {
    with_dom(|dom| dom.realizer.set_control_state(sink, *state));
  }

  fn create_overlay(sink: NodeId, props: &OverlayProps<Self>) {
    with_dom(|dom| {
      dom.realizer.create_overlay(sink, props);

      if props.modal
        && let Some(node) = dom.node_mut(sink)
      {
        node.extra.cancel = Some(Listener::new(
          node.element.clone().into(),
          "cancel",
          false,
          move |event| {
            event.prevent_default();

            if with_dom(|dom| dom.node(sink).is_some()) {
              notify_dismiss::<Self>(sink);
            }
          },
        ));
      }
    });
  }

  fn set_overlay_open(sink: NodeId, open: bool, anchor: Option<NodeId>) {
    let Some((element, modal, placement, position, anchor)) = with_dom(|dom| {
      let node = dom.node(sink)?;
      let Realized::Overlay {
        modal,
        placement,
        position,
      } = node.realized
      else {
        return None;
      };
      let element = node.dom_element()?.clone();
      let anchor_element = anchor.and_then(|anchor| {
        let element = dom.node(anchor)?.dom_element().cloned()?;

        Some((anchor, element))
      });

      if open && !element.is_connected() {
        dom.pending_open.insert(sink, anchor);

        return None;
      }

      dom.pending_open.remove(&sink);

      Some((element, modal, placement, position, anchor_element))
    })
    else {
      return;
    };

    if modal {
      if let Some(dialog) = element.dyn_ref::<HtmlDialogElement>() {
        if open {
          let _ = dialog.show_modal();
        }
        else {
          dialog.close();
        }
      }

      return;
    }

    let Some(html) = element.dyn_ref::<HtmlElement>()
    else {
      return;
    };

    if open {
      let _ = html.show_popover();

      if let Some(position) = position {
        position_at_point(html, position);
      }
      else if let Some((sink, anchor)) = anchor {
        position_at_anchor(html, &anchor, sink, placement);
      }
    }
    else {
      let _ = html.hide_popover();
    }
  }

  fn set_overlay_position(sink: NodeId, position: Option<Point<f32>>) {
    with_dom(|dom| {
      if let Some(node) = dom.node_mut(sink)
        && let Realized::Overlay { position: held, .. } = &mut node.realized
      {
        *held = position;
      }
    });
  }

  fn set_overlay_placement(sink: NodeId, placement: Placement) {
    with_dom(|dom| {
      if let Some(node) = dom.node_mut(sink)
        && let Realized::Overlay {
          placement: held, ..
        } = &mut node.realized
      {
        *held = placement;
      }
    });
  }

  fn apply_layout(sink: NodeId, layout: &StaticBoxStyle<Self>) {
    with_dom(|dom| dom.realizer.apply_layout(sink, layout));
  }

  fn create_run(sink: NodeId, props: &RunProps<Self>) {
    with_dom(|dom| dom.realizer.create_run(sink, props));
  }

  fn update_run(sink: NodeId, slot: Slot, leaf: &StaticLeaf<RunValue<GuiExt>>) {
    with_dom(|dom| dom.realizer.update_run(sink, slot, leaf));
  }

  fn set_text(sink: NodeId, text: &str) {
    with_dom(|dom| dom.realizer.set_text(sink, text));
  }

  fn create_anchor(sink: NodeId) {
    with_dom(|dom| dom.realizer.create_anchor(sink));
  }

  fn create_scope(sink: NodeId) {
    with_dom(|dom| dom.realizer.create_scope(sink));
  }

  fn attach(parent: NodeId, child: NodeId, before: Option<NodeId>) {
    let Some((parent, child, caption, before, saved, kept, document)) = with_dom(|dom| {
      if dom.place_caption(parent, child) {
        return None;
      }

      let parent = dom.node(parent)?.element.clone();
      let child_node = dom.node(child)?;

      if child_node.adopted && child_node.element.parent_node().as_ref() == Some(&parent) {
        return None;
      }

      let child_dom = child_node.element.clone();
      let caption = child_node.caption.clone();
      let saved = before.and_then(|before| dom.saved.remove(&before));
      let before = before
        .and_then(|before| dom.node(before))
        .map(|node| node.element.clone());

      Some((
        parent,
        child_dom,
        caption,
        before,
        saved,
        dom.kept.remove(&child),
        dom.document().clone(),
      ))
    })
    else {
      return;
    };

    let _ = parent.insert_before(&child, before.as_ref());

    if let Some(caption) = caption
      && caption.parent_node().as_ref() != Some(&parent)
    {
      let _ = parent.insert_before(&caption, Some(&child));
    }

    if let Some(saved) = saved {
      saved.restore(&child);
    }

    if let Some(kept) = kept {
      kept.restore(&document);
    }

    for (overlay, anchor) in with_dom(Dom::take_connected_opens) {
      Self::set_overlay_open(overlay, true, anchor);
    }
  }

  fn relocate(parent: NodeId, child: NodeId, before: Option<NodeId>) {
    if with_dom(|dom| dom.lift_caption(child)) {
      Self::attach(parent, child, before);

      return;
    }

    let Some((parent, child, caption, before, mover)) = with_dom(|dom| {
      if dom.place_caption(parent, child) {
        return None;
      }

      let parent = dom.node(parent)?.element.clone();
      let child_node = dom.node(child)?;
      let child_dom = child_node.element.clone();
      let caption = child_node.caption.clone();
      let before = before
        .and_then(|before| dom.node(before))
        .map(|node| node.element.clone());

      Some((parent, child_dom, caption, before, dom.mover.clone()))
    })
    else {
      return;
    };

    mover.move_node(&parent, &child, before.as_ref());

    if let Some(caption) = caption {
      mover.move_node(&parent, &caption, Some(&child));
    }

    for (overlay, anchor) in with_dom(Dom::take_connected_opens) {
      Self::set_overlay_open(overlay, true, anchor);
    }
  }

  fn hide(parent: NodeId, child: NodeId) {
    let (nodes, overlays) = Scene::<Self>::with(|scene| {
      let mut nodes = vec![child];
      let mut next = 0;

      while let Some(node) = nodes.get(next).copied() {
        nodes.extend(scene.children(node));
        next += 1;
      }

      let overlays: Vec<_> = scene
        .open_overlays()
        .iter()
        .copied()
        .filter(|overlay| scene.ancestry(*overlay).contains(&child))
        .map(|overlay| (overlay, scene.overlay_anchor(overlay)))
        .collect();

      (nodes, overlays)
    });

    with_dom(|dom| {
      if let Some(kept) = dom
        .node(child)
        .map(|node| node.element.clone())
        .and_then(|root| Kept::capture(dom, &root, &nodes))
      {
        dom.kept.insert(child, kept);
      }

      dom.pending_open.extend(overlays);
    });
    Self::remove(parent, child);
  }

  fn remove(parent: NodeId, child: NodeId) {
    let Some((parent, child, caption)) = with_dom(|dom| {
      if dom.lift_caption(child) {
        return None;
      }

      Some((
        dom.node(parent)?.element.clone(),
        dom.node(child)?.element.clone(),
        dom.node(child)?.caption.clone(),
      ))
    })
    else {
      return;
    };

    if child.parent_node().as_ref() == Some(&parent) {
      let _ = parent.remove_child(&child);
    }

    if let Some(caption) = caption
      && caption.parent_node().as_ref() == Some(&parent)
    {
      let _ = parent.remove_child(&caption);
    }
  }

  fn destroy(sink: NodeId) {
    with_dom(|dom| {
      let Some(node) = dom.realizer.destroy(sink)
      else {
        return;
      };

      if !node.extra.observers.is_empty()
        && let (Some((observer, _)), Some(element)) = (&dom.resize, node.dom_element())
      {
        observer.unobserve(element);
      }

      if let (Some(pixels), Some(canvas)) = (&dom.pixels, &node.extra.canvas) {
        pixels.unobserve(&canvas.element);
      }

      dom.saved.remove(&sink);
      dom.kept.remove(&sink);
      dom.pending_open.remove(&sink);
    });
  }

  fn define_tokens(scope: NodeId, definitions: &Definitions<WebTokenValue>) {
    with_dom(|dom| dom.realizer.define_tokens(scope, definitions));
  }

  fn define_root_tokens(definitions: &Definitions<WebTokenValue>) {
    with_dom(|dom| dom.realizer.define_root_tokens(definitions));
  }

  fn listen(sink: NodeId, kind: EventKind) {
    with_dom(|dom| {
      dom.events.ensure(kind);

      if kind == EventKind::DragStart
        && let Some(element) = dom.node(sink).and_then(Node::dom_element)
      {
        let _ = element.set_attribute("draggable", "true");
      }
    });
  }

  fn localize_pointer(sink: NodeId, event: &PointerEvent) -> PointerEvent {
    let element = with_dom(|dom| dom.node(sink).and_then(Node::dom_element).cloned());

    element.map_or(*event, |element| events::localized(event, &element))
  }

  fn focus(sink: NodeId) {
    let carrier = with_dom(|dom| dom.node(sink).and_then(Node::carrier_element).cloned());

    if let Some(html) = carrier
      .as_ref()
      .and_then(|carrier| carrier.dyn_ref::<HtmlElement>())
    {
      let _ = html.focus();
    }
  }

  fn blur(sink: NodeId) {
    let carrier = with_dom(|dom| dom.node(sink).and_then(Node::carrier_element).cloned());

    if let Some(html) = carrier
      .as_ref()
      .and_then(|carrier| carrier.dyn_ref::<HtmlElement>())
    {
      let _ = html.blur();
    }
  }

  fn focus_visible() -> bool {
    web_sys::window()
      .and_then(|window| window.document())
      .and_then(|document| document.active_element())
      .is_some_and(|element| element.matches(":focus-visible").unwrap_or(false))
  }

  fn set_value(sink: NodeId, value: &str) {
    with_dom(|dom| dom.realizer.set_value(sink, value));
  }

  fn set_options(sink: NodeId, options: &[PickerOption]) {
    with_dom(|dom| dom.realizer.set_options(sink, options));
  }

  fn set_selection(sink: NodeId, selection: TextSelection) {
    with_dom(|dom| {
      let Some(carrier) = dom.node(sink).and_then(Node::carrier_element)
      else {
        return;
      };

      events::select(carrier, selection);
    });
  }

  fn set_checked(sink: NodeId, checked: bool) {
    with_dom(|dom| dom.realizer.set_checked(sink, checked));
  }

  fn observe_size(sink: NodeId, report: Rc<dyn Fn(Size<f32>)>) -> Observation {
    let kept = Rc::clone(&report);

    with_dom(|dom| {
      let observer = dom.resize_observer();
      let Some(node) = dom.node_mut(sink)
      else {
        return;
      };

      if let Some(element) = node.dom_element() {
        if node.extra.observers.is_empty() {
          observer.observe(element);
        }

        node.extra.observers.push(report);
      }
    });

    Observation::new(move || {
      let _ = try_with_dom(|dom| {
        let observer = dom.resize_observer();
        let Some(node) = dom.node_mut(sink)
        else {
          return;
        };

        node
          .extra
          .observers
          .retain(|observer| !Rc::ptr_eq(observer, &kept));

        if node.extra.observers.is_empty()
          && let Some(element) = node.dom_element()
        {
          observer.unobserve(element);
        }
      });
    })
  }

  fn media() -> MediaSignals<f32> {
    media::signals()
  }

  fn hydration() -> Option<ReadSignal<bool>> {
    Some(media::live())
  }

  fn set_scheme(scheme: Option<ColorScheme>) {
    media::set_scheme(scheme);
  }

  fn schedule(delay: Duration, action: Box<dyn FnOnce()>) -> Timer {
    with_dom(|dom| {
      dom.timers.retain(|_, timeout| !timeout.fired.get());

      let timer = Timer(dom.next_timer);
      let fired = Rc::new(Cell::new(false));
      let action = Cell::new(Some(action));
      let callback = Closure::<dyn FnMut()>::new({
        let fired = Rc::clone(&fired);

        move || {
          fired.set(true);

          if let Some(action) = action.take() {
            action();
          }
        }
      });
      let handle = dom
        .document()
        .default_view()
        .and_then(|window| {
          window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
              callback.as_ref().unchecked_ref(),
              i32::try_from(delay.as_millis()).unwrap_or(i32::MAX),
            )
            .ok()
        })
        .unwrap_or(0);

      dom.next_timer += 1;
      dom.timers.insert(
        timer,
        Timeout {
          handle,
          _callback: callback,
          fired,
        },
      );

      timer
    })
  }

  fn cancel(timer: Timer) {
    with_dom(|dom| {
      if let Some(timeout) = dom.timers.remove(&timer)
        && !timeout.fired.get()
        && let Some(window) = dom.document().default_view()
      {
        window.clear_timeout_with_handle(timeout.handle);
      }
    });
  }

  fn clipboard_availability(action: ClipboardAction) -> Availability {
    with_dom(|dom| {
      if clipboard(dom).is_none() {
        return Availability::Absent;
      }

      // Outside an entry, only the document's selection is copyable, and nothing is
      // cut or pasted; inside one, every action is listed and applies to its selection.
      match (clipboard_entry(dom), action) {
        (Some(entry), ClipboardAction::Copy | ClipboardAction::Cut) => {
          if entry_selection(&entry).is_some_and(|(start, end, _)| start != end) {
            Availability::Enabled
          }
          else {
            Availability::Disabled
          }
        }
        (Some(_), ClipboardAction::Paste) => Availability::Enabled,
        (None, ClipboardAction::Copy) => {
          if document_selection(dom).is_some_and(|text| !text.is_empty()) {
            Availability::Enabled
          }
          else {
            Availability::Absent
          }
        }
        (None, ClipboardAction::Cut | ClipboardAction::Paste) => Availability::Absent,
      }
    })
  }

  fn clipboard(action: ClipboardAction) {
    let Some((clipboard, entry, text)) = with_dom(|dom| {
      let clipboard = clipboard(dom)?;
      let entry = clipboard_entry(dom);
      let text = match &entry {
        Some(entry) => entry_selection(entry).map(|(_, _, text)| text),
        None => document_selection(dom),
      };

      Some((clipboard, entry, text))
    })
    else {
      return;
    };

    match action {
      ClipboardAction::Copy => {
        if let Some(text) = text {
          let _ = clipboard.write_text(&text);
        }
      }
      ClipboardAction::Cut => {
        if let (Some(text), Some(entry)) = (text, entry) {
          let _ = clipboard.write_text(&text);
          replace_entry_selection(&entry, "");
        }
      }
      ClipboardAction::Paste => {
        if let Some(entry) = entry {
          let read = wasm_bindgen_futures::JsFuture::from(clipboard.read_text());

          wasm_bindgen_futures::spawn_local(async move {
            if let Some(text) = read.await.ok().and_then(|text| text.as_string()) {
              replace_entry_selection(&entry, &text);
            }
          });
        }
      }
    }
  }

  fn stored_scheme() -> Option<ColorScheme> {
    media::stored_scheme()
  }

  fn watch_media(media: &Media<f32>) -> Memo<bool> {
    media::watch_media(media)
  }
}

impl WebMarkup for Web {
  fn create_element(node: NodeId, tag: &'static str) {
    with_dom(|dom| {
      dom.realizer.create_element(node, tag, Realized::Box);
    });
  }

  fn set_attribute(sink: NodeId, name: &str, value: &str) {
    with_dom(|dom| dom.realizer.set_attribute(sink, name, value));
  }

  fn set_declarations(sink: NodeId, declarations: &Declarations) {
    with_dom(|dom| dom.realizer.set_declarations(sink, declarations));
  }

  fn set_text(sink: NodeId, text: &str) {
    <Self as Platform>::set_text(sink, text);
  }
}

impl WebDom for Web {
  fn element(sink: NodeId) -> Option<Element> {
    with_dom(|dom| dom.node(sink).and_then(Node::dom_element).cloned())
  }

  fn document() -> Document {
    document()
  }
}

/// Installs the browser leaf for this thread, rooted at `root`, adopting the server's
/// nodes under it when `hydrating`.
fn install_at(root: Element, hydrating: bool) {
  skene_core::hold_flushes(|| wasm_bindgen_futures::spawn_local(async { end_turn() }));
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
      Scene::<Web>::with(|scene| scene.create(NodeKind::Box)),
      hydration,
    ));
  });

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
/// # Panics
///
/// Panics if the platform is not installed for this thread.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  end_turn();

  let node = skene_core::root_owner().enter(|| {
    let node = view();
    let root = with_dom(|dom| dom.root_node);

    Scene::<Web>::with(|scene| scene.attach(root, node, None));

    node
  });

  end_turn();

  node
}

/// Hydrates a server-rendered tree under `root`.
///
/// It adopts the tree's elements by their creation-order markers and its anchors by
/// their comments, the server's `<style>` block in place of a fresh one, and the
/// resource `state` serialized alongside them.
///
/// A native element whose live value or checked state differs from the serialized one
/// reports the difference through the control's change or toggle path once the tree
/// is built. Every mismatch is reported to the console and counted in
/// [`hydration_mismatches`].
///
/// # Panics
///
/// Panics if the platform is already installed for this thread.
pub fn hydrate<V, F>(root: Element, state: &str, app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
{
  install_at(root, true);
  crate::hydration::stage(state);
  let _ = mount(|| app().into_view().build());

  let pending = with_dom(Dom::finish_hydration);

  for (sink, pending) in pending {
    let Some(node) = with_dom(|dom| dom.node(sink).map(|_| sink))
    else {
      continue;
    };

    match pending {
      Pending::Toggle(checked) => notify_toggle::<Web>(node, checked),
      Pending::Change(value) => notify_change::<Web>(node, &value),
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
///
/// # Panics
///
/// Panics if there is no `window`, `document` or `<body>`.
pub fn hydrate_page<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
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
/// place as [`hydrate_page`] does.
///
/// # Panics
///
/// Panics if there is no `window`, `document` or `<body>`.
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
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

  let _ = mount(|| app().into_view().build());
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
pub fn claim_lazy(start: NodeId) -> bool {
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
pub fn discard_lazy(start: NodeId) {
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
  skene_core::reset_thread::<Web>();
}
