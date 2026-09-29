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
  Part,
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
  Toggle,
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
  compile::{
    Compiled,
    SlotBinding,
    all_unconditional,
    box_fingerprint,
    compile_box,
    compile_control,
    compile_definitions,
    compile_layout,
    compile_overlay,
    compile_root_definitions,
    compile_run,
    control_fingerprint,
    inline_definitions,
    layout_fingerprint,
    overlay_fingerprint,
    render_box_leaf,
    render_run_leaf,
    run_fingerprint,
  },
  condition::{CONTROL_ATTRIBUTE, DRAGS_ATTRIBUTE, NATIVE_CLASS},
  markup::{
    MARKER_ATTRIBUTE,
    NATIVE_ACCESS_ATTRIBUTES,
    NativeElement,
    SCOPE_ATTRIBUTE,
    STYLE_ID,
    access_attributes,
    anchor_text,
    aria_role,
    box_tag,
    drawn_element,
    element_id,
    focus_order_attribute,
    is_form_element,
    link_attributes,
    native_attributes,
    native_element,
    referenced_nodes,
    replaced_by_access,
    runs_script,
    scrolls,
    takes_disabled,
  },
  token::WebTokenValue,
};
use crate::{Declarations, WebDom, WebMarkup, hydration::STATE_SCRIPT_ID};

/// The sink of the mount root; its children are the app.
const ROOT: u32 = 0;

/// The DOM node type of a comment.
const COMMENT_NODE: u16 = 8;

/// The shared size observer with its live callback.
type SizeObserver = (ResizeObserver, Closure<dyn FnMut(js_sys::Array)>);

/// One observer of a node's size, with the size to report to it.
type SizeReport = (Rc<dyn Fn(Size<f32>)>, Size<f32>);

/// What a node's DOM handle is realized as.
enum Realized {
  Box,
  Control {
    native: Option<NativeElement>,
    caption_beside: bool,
  },
  Overlay {
    modal: bool,
    placement: Placement,
    position: Option<Point<f32>>,
  },
  Run,
  Anchor,
  Scope {
    /// The custom properties its definitions are written inline as, over its token class.
    inline: Vec<String>,
  },
}

/// One node's record beside the scene: its DOM node, the native element carrying a
/// control's state, focus and value when the control has one apart from its root, and
/// what the platform keeps per node.
struct Node {
  /// The DOM node realizing it.
  dom: web_sys::Node,
  /// The native element carrying a control's state, focus and value, where the control
  /// has one apart from its root.
  carrier: Option<Element>,
  /// The `<label for>` a text entry's caption lives in, kept before the entry among
  /// its siblings.
  caption: Option<Element>,
  /// What it is realized as.
  realized: Realized,
  /// The scene node it realizes, once bound.
  id: Option<NodeId>,
  /// The class of its compiled style.
  style_class: Option<Rc<str>>,
  /// The class of a scope's token definitions.
  token_class: Option<Rc<str>>,
  /// The slots its class references.
  slots: Rc<[SlotBinding]>,
  /// The reports of the box's size.
  observers: Vec<Rc<dyn Fn(Size<f32>)>>,
  /// A modal's `cancel` listener, reporting a dismissal.
  cancel: Option<Listener>,
  /// Whether the hydration in progress adopted it from the server's markup.
  adopted: bool,
  /// Whether the box scrolls its content, so it takes a place in the focus order.
  scrolls: bool,
  /// The canvas a painted box paints into.
  canvas: Option<Box<canvas::PaintedCanvas>>,
}

impl Node {
  fn new(dom: impl Into<web_sys::Node>, realized: Realized, adopted: bool) -> Self {
    Self {
      dom: dom.into(),
      carrier: None,
      caption: None,
      realized,
      id: None,
      style_class: None,
      token_class: None,
      slots: Rc::from([]),
      observers: Vec::new(),
      cancel: None,
      adopted,
      scrolls: false,
      canvas: None,
    }
  }

  /// The node's element; `None` for an anchor.
  fn element(&self) -> Option<&Element> {
    self.dom.dyn_ref::<Element>()
  }

  /// The element carrying the node's state: its native input, else itself.
  fn carrier(&self) -> Option<&Element> {
    self.carrier.as_ref().or_else(|| self.element())
  }

  fn html(&self) -> Option<&HtmlElement> {
    self.dom.dyn_ref::<HtmlElement>()
  }

  const fn is_control(&self) -> bool {
    matches!(self.realized, Realized::Control { .. })
  }

  /// Records the custom properties a scope's adopted markup defines inline.
  fn adopt_inline_definitions(&mut self) {
    let (Realized::Scope { inline }, Some(html)) =
      (&mut self.realized, self.dom.dyn_ref::<HtmlElement>())
    else {
      return;
    };

    let style = html.style();

    inline.extend(
      (0..style.length())
        .map(|index| style.item(index))
        .filter(|property| property.starts_with("--")),
    );
  }

  /// Replaces a scope's inline definitions with `declarations`.
  fn set_inline_definitions(&mut self, declarations: Vec<(String, String)>) {
    let (Realized::Scope { inline }, Some(html)) =
      (&mut self.realized, self.dom.dyn_ref::<HtmlElement>())
    else {
      return;
    };

    let style = html.style();

    for property in inline.drain(..) {
      let _ = style.remove_property(&property);
    }

    for (property, value) in declarations {
      let _ = style.set_property(&property, &value);

      inline.push(property);
    }
  }
}

/// What the reader changed on a native element before the client arrived.
enum Pending {
  Toggle(bool),
  Change(String),
}

/// What content leaving a segment held, keyed by the element's index in the
/// content's document order: the scrolled offsets and the focused element.
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

/// The server's nodes awaiting adoption, and what adoption found.
struct Hydration {
  elements: FxHashMap<u32, Element>,
  comments: FxHashMap<String, web_sys::Node>,
  mismatches: u32,
  pending: Vec<(u32, Pending)>,
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
      mismatches: 0,
      pending: Vec::new(),
    }
  }
}

/// The browser leaf's state: the document, the node records, the stylesheet sink, the
/// delegated events and the hydration in progress.
struct Dom {
  /// The document the app mounts in.
  document: Document,
  /// The node records, indexed by sink.
  nodes: Vec<Option<Node>>,
  /// The sinks of the empty records, reused before the records grow.
  free: Vec<u32>,
  /// The hydration marker the next element created takes.
  markers: u32,
  /// The ordinal the next anchor's comment takes.
  anchors: u32,
  /// The stylesheet sink.
  sheet: Sheet,
  /// The delegated events.
  events: Events,
  /// The class of the root's token definitions.
  root_class: Option<Rc<str>>,
  /// The hydration in progress.
  hydration: Option<Hydration>,
  /// How many nodes the last hydration failed to adopt.
  mismatches: u32,
  /// The shared size observer, made when the first size is observed.
  resize: Option<SizeObserver>,
  /// The scene node of the mount root.
  root_node: Option<NodeId>,
  /// What content leaving a segment held, by the sink of the anchor following it.
  saved: FxHashMap<u32, Saved>,
  /// The observer reporting each canvas's size, made when the first box paints.
  pixels: Option<canvas::Pixels>,
  /// The overlays asked open before their element was connected, each with its anchor.
  pending_open: FxHashMap<u32, Option<u32>>,
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
  /// The flush the turn's writes scheduled, run when the turn ends.
  static TURN: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

/// Ends the turn now: runs the flush its writes scheduled, which otherwise runs at the
/// next microtask checkpoint.
///
/// An event, a mount and a hydration each end their own turn; a test calls this where
/// it reads the page after a direct write.
pub fn end_turn() {
  let flush = TURN.with(|turn| turn.borrow_mut().take());

  if let Some(flush) = flush {
    flush();
  }
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
  fn new(document: Document, root: Element, hydration: Option<Hydration>) -> Self {
    let head = document.head().expect("a head");
    let sheet = match hydration
      .as_ref()
      .and_then(|_| document.get_element_by_id(STYLE_ID))
    {
      Some(block) => Sheet::adopt(&document, &head, block),
      None => Sheet::new(&document, &head),
    };
    let events = Events::new(root.clone());

    events::stamp(&root, ROOT);

    Self {
      document,
      nodes: vec![Some(Node::new(root, Realized::Box, false))],
      free: Vec::new(),
      markers: 1,
      anchors: 0,
      sheet,
      events,
      root_class: None,
      hydration,
      mismatches: 0,
      resize: None,
      root_node: None,
      saved: FxHashMap::default(),
      pixels: None,
      pending_open: FxHashMap::default(),
      timers: FxHashMap::default(),
      next_timer: 0,
    }
  }

  fn node(&self, sink: u32) -> Option<&Node> {
    self.nodes.get(sink as usize).and_then(Option::as_ref)
  }

  fn node_mut(&mut self, sink: u32) -> Option<&mut Node> {
    self.nodes.get_mut(sink as usize).and_then(Option::as_mut)
  }

  fn insert(&mut self, node: Node) -> u32 {
    if let Some(element) = node.element() {
      let sink = self
        .free
        .last()
        .copied()
        .unwrap_or_else(|| u32::try_from(self.nodes.len()).expect("at most 2^32 nodes"));

      events::stamp(element, sink);
    }

    if let Some(sink) = self.free.pop() {
      self.nodes[sink as usize] = Some(node);

      return sink;
    }

    let sink = u32::try_from(self.nodes.len()).expect("at most 2^32 nodes");

    self.nodes.push(Some(node));

    sink
  }

  /// The element for the node created next: the server's element at this creation
  /// index while hydrating, else a fresh `tag`; with whether it was adopted.
  fn take_element(&mut self, tag: &str) -> (Element, bool) {
    let marker = self.markers;

    self.markers += 1;

    if let Some(hydration) = &mut self.hydration {
      match hydration.elements.remove(&marker) {
        Some(element) if element.tag_name().eq_ignore_ascii_case(tag) => {
          let _ = element.remove_attribute(MARKER_ATTRIBUTE);

          return (element, true);
        }
        Some(element) => {
          hydration.mismatches += 1;
          tracing::error!(
            "hydration mismatch: server rendered <{}> where the client builds <{tag}> (marker {marker})",
            element.tag_name().to_lowercase(),
          );
        }
        None => {
          tracing::debug!("no serialized <{tag}> for marker {marker}; building fresh");
        }
      }
    }

    (
      self.document.create_element(tag).expect("an element"),
      false,
    )
  }

  /// The comment for the anchor created next: the server's comment of `text` while
  /// hydrating, else a fresh one; with whether it was adopted.
  fn take_comment(&mut self, text: &str) -> (web_sys::Node, bool) {
    if let Some(hydration) = &mut self.hydration {
      if let Some(comment) = hydration.comments.remove(text) {
        return (comment, true);
      }

      tracing::debug!("no serialized anchor {text}; building fresh");
    }

    (self.document.create_comment(text).into(), false)
  }

  fn create_element(&mut self, tag: &str, realized: Realized) -> u32 {
    let (element, adopted) = self.take_element(tag);

    self.insert(Node::new(element, realized, adopted))
  }

  /// Takes the overlays asked open before their element was connected whose element now
  /// is, each with its anchor.
  fn take_connected_opens(&mut self) -> Vec<(u32, Option<u32>)> {
    if self.pending_open.is_empty() {
      return Vec::new();
    }

    let connected: Vec<_> = self
      .pending_open
      .iter()
      .filter(|(sink, _)| {
        self
          .node(**sink)
          .and_then(Node::element)
          .is_some_and(|element| element.is_connected())
      })
      .map(|(sink, anchor)| (*sink, *anchor))
      .collect();

    for (sink, _) in &connected {
      self.pending_open.remove(sink);
    }

    connected
  }

  /// The part `sink` is mounted as, when it is one.
  fn part_of(&self, sink: u32) -> Option<Part> {
    let node = self.node(sink)?.id?;

    Scene::<Web>::with(|scene| scene.part(node))
  }

  /// Attaches `child` into the caption of `parent` when `parent` keeps its caption
  /// beside it and `child` is its `Label` part, making the `<label>` on the first,
  /// `for` the control where it is its own `<input>`, `<textarea>` or `<select>`, and
  /// placing it before the control; `false` for any other pair, which attaches as
  /// usual.
  fn attach_caption(&mut self, parent: u32, child: u32) -> bool {
    let Some((node, labels)) = self.node(parent).and_then(|entry| match entry.realized {
      Realized::Control {
        native,
        caption_beside: true,
      } => Some((entry.id?, native.is_some_and(NativeElement::is_bare))),
      _ => None,
    })
    else {
      return false;
    };

    if self.part_of(child) != Some(Part::Label) {
      return false;
    }

    let caption = if let Some(caption) = self.node(parent).and_then(|node| node.caption.clone()) {
      caption
    }
    else {
      let (caption, _) = self.take_element("label");
      let id = element_id(node);

      if labels {
        let _ = caption.set_attribute("for", &id);
      }

      if let Some(entry) = self.node_mut(parent) {
        if labels && let Some(element) = entry.element() {
          let _ = element.set_attribute("id", &id);
        }

        if let Some(container) = entry.dom.parent_node()
          && caption.parent_node().as_ref() != Some(&container)
        {
          let _ = container.insert_before(&caption, Some(&entry.dom));
        }

        entry.caption = Some(caption.clone());
      }

      caption
    };
    let Some(child) = self.node(child).map(|node| node.dom.clone())
    else {
      return true;
    };

    if child.parent_node().as_ref() != Some(caption.as_ref()) {
      let _ = caption.append_child(&child);
    }

    true
  }

  /// Detaches `child` from the caption of `parent` when it sits there, dropping the
  /// caption once empty; `false` when it does not.
  fn detach_caption(&mut self, parent: u32, child: u32) -> bool {
    let Some(caption) = self.node(parent).and_then(|node| node.caption.clone())
    else {
      return false;
    };
    let Some(child) = self.node(child).map(|node| node.dom.clone())
    else {
      return false;
    };

    if child.parent_node().as_ref() != Some(caption.as_ref()) {
      return false;
    }

    let _ = caption.remove_child(&child);

    if !caption.has_child_nodes() {
      caption.remove();

      if let Some(entry) = self.node_mut(parent) {
        entry.caption = None;
      }
    }

    true
  }

  fn assign_style(&mut self, sink: u32, compiled: &Rc<Compiled>) {
    self.sheet.register_slots(&compiled.slots);

    let previous = self.node(sink).and_then(|node| node.style_class.clone());
    let class = self
      .sheet
      .swap_class(&compiled.template, previous.as_deref());

    if let Some(node) = self.node_mut(sink) {
      if let Some(element) = node.element() {
        swap_class_attribute(element, previous.as_deref(), &class);
      }

      node.style_class = Some(class);
      node.slots = Rc::from(compiled.slots.as_slice());
    }
  }

  fn write_focus_order(&self, sink: u32, focus_order: Option<FocusOrder>) {
    if let Some(node) = self.node(sink)
      && let Some(carrier) = node.carrier()
    {
      match focus_order_attribute(focus_order, node.is_control(), node.scrolls) {
        Some(tabindex) => {
          let _ = carrier.set_attribute("tabindex", &tabindex);
        }
        None => {
          let _ = carrier.remove_attribute("tabindex");
        }
      }
    }
  }

  fn write_access(&self, sink: u32, access: &[AccessProp]) {
    let Some(carrier) = self.node(sink).and_then(Node::carrier)
    else {
      return;
    };
    let names = carrier.get_attribute_names();
    let form = is_form_element(&carrier.tag_name());

    for index in 0..names.length() {
      if let Some(name) = names.get(index).as_string()
        && replaced_by_access(&name)
      {
        let _ = carrier.remove_attribute(&name);
      }
    }

    if form {
      for name in NATIVE_ACCESS_ATTRIBUTES {
        let _ = carrier.remove_attribute(name);
      }
    }

    for (name, value) in access_attributes(access, form) {
      // A plain description is the server's `title`; with script the tooltip is ours.
      if name == "title" {
        continue;
      }

      let _ = carrier.set_attribute(name, &value);

      if name == "value"
        && let Some(input) = carrier.dyn_ref::<HtmlInputElement>()
      {
        input.set_value(&value);
      }
    }

    for node in referenced_nodes(access) {
      let Some(target) = Scene::<Web>::with(|scene| scene.sink(node))
      else {
        continue;
      };

      if let Some(element) = self.node(target).and_then(Node::carrier) {
        let _ = element.set_attribute("id", &element_id(node));
      }
    }
  }

  /// Realizes the props every box-like node shares onto `sink`.
  fn finish_box(&mut self, sink: u32, props: &BoxProps<Web>, compiled: &Rc<Compiled>) {
    self.assign_style(sink, compiled);

    if let Some(node) = self.node_mut(sink) {
      node.scrolls = scrolls::<Web>(&props.style);
    }

    self.write_focus_order(sink, props.focus_order);
    self.write_access(sink, &props.access);
  }

  /// Notes what the reader changed on the adopted native `input` of `sink` before the
  /// client arrived, against what the server serialized.
  fn note_live_state(&mut self, sink: u32, input: &Element, native: NativeElement) {
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
      hydration.pending.push((sink, pending));
    }
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
        let Some(sink) = events::sink_of(&target)
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

  /// The sink of the anchor realized as the comment `node`, if any.
  fn anchor_sink(&self, node: &web_sys::Node) -> Option<u32> {
    if node.node_type() != COMMENT_NODE {
      return None;
    }

    self
      .nodes
      .iter()
      .position(|record| {
        record
          .as_ref()
          .is_some_and(|record| matches!(record.realized, Realized::Anchor) && record.dom == *node)
      })
      .and_then(|index| u32::try_from(index).ok())
  }

  /// Keeps what `roots` hold on the anchor `following` them, for the content that next
  /// enters the segment before it.
  fn save(&mut self, roots: &[web_sys::Node], following: Option<web_sys::Node>) {
    let Some(saved) = Saved::capture(&self.document, roots)
    else {
      return;
    };

    if let Some(anchor) = following.and_then(|node| self.anchor_sink(&node)) {
      self.saved.insert(anchor, saved);
    }
  }

  /// Ends hydration: reports what the server rendered and the client never built, and
  /// forgets which nodes were adopted.
  fn finish_hydration(&mut self) -> Vec<(u32, Pending)> {
    let Some(hydration) = self.hydration.take()
    else {
      return Vec::new();
    };
    let unadopted = hydration.elements.len() + hydration.comments.len();

    if unadopted > 0 {
      tracing::error!("hydration mismatch: {unadopted} server node(s) were never adopted");
    }

    self.mismatches = hydration.mismatches + u32::try_from(unadopted).unwrap_or(u32::MAX);

    for node in self.nodes.iter_mut().flatten() {
      node.adopted = false;
    }

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

/// Writes the attributes a control's native `element` carries.
fn write_native_attributes(element: &Element, native: NativeElement, props: &ControlProps<Web>) {
  for (name, value) in native_attributes(native, props) {
    let _ = element.set_attribute(name, &value);
  }
}

/// Writes `on` as the presence of the attribute `name` on `element`.
fn set_flag(element: &Element, name: &str, on: bool) {
  if on {
    let _ = element.set_attribute(name, "");
  }
  else {
    let _ = element.remove_attribute(name);
  }
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

/// Positions an open popup against its anchor: below it for `Bottom`, above it for
/// `Top`, start-aligned either way; other placements keep their viewport position.
///
/// Where the engine has CSS anchor positioning the anchor is named after its sink and
/// the popup follows it through `position-area`; elsewhere the popup is placed at the
/// anchor's measured rectangle as it opens.
/// The async clipboard, where the page has one: a secure context.
fn clipboard(dom: &Dom) -> Option<web_sys::Clipboard> {
  let navigator = dom.document.default_view()?.navigator();

  js_sys::Reflect::has(&navigator, &JsValue::from_str("clipboard"))
    .ok()
    .filter(|has| *has)
    .map(|_| navigator.clipboard())
}

/// The text entry a clipboard action acts on: the active one, or the one focus left
/// for the open popup it now sits in.
fn clipboard_entry(dom: &Dom) -> Option<Element> {
  let active = dom.document.active_element()?;

  if entry_selection(&active).is_some() {
    return Some(active);
  }

  let active: &web_sys::Node = &active;
  let popups: Vec<u32> = Scene::<Web>::with(|scene| {
    scene
      .open_overlays()
      .iter()
      .filter(|overlay| !scene.is_modal(**overlay))
      .filter_map(|overlay| scene.sink(*overlay))
      .collect()
  });
  let in_popup = popups.iter().any(|sink| {
    dom
      .node(*sink)
      .is_some_and(|node| node.dom.contains(Some(active)))
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
  let selection = dom.document.default_view()?.get_selection().ok()??;

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

fn position_at_anchor(popup: &HtmlElement, anchor: &Element, sink: u32, placement: Placement) {
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
    let name = format!("--skene-anchor-{sink}");

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
  type Sink = u32;
  type StyleExt = GuiExt;
  type TokenValue = WebTokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn bind_node(sink: u32, node: NodeId) {
    with_dom(|dom| {
      if let Some(record) = dom.node_mut(sink) {
        record.id = Some(node);
      }
    });
  }

  fn create_box(props: &BoxProps<Self>) -> u32 {
    with_dom(|dom| {
      let sink = dom.create_element(
        box_tag(props.role, props.destination.is_some()),
        Realized::Box,
      );

      if let Some(element) = dom.node(sink).and_then(Node::element) {
        if let Some(role) = props.role.filter(|role| role.is_spelled()) {
          let _ = element.set_attribute("role", aria_role(role));
        }

        if let Some(href) = props
          .destination
          .as_deref()
          .filter(|href| !runs_script(href))
        {
          let _ = element.set_attribute("href", href);
        }
      }

      let compiled = dom
        .sheet
        .compiled(box_fingerprint(props, 0), || compile_box(props));

      dom.finish_box(sink, props, &compiled);

      sink
    })
  }

  fn update_box(sink: u32, slot: Slot, leaf: &StaticLeaf<BoxValue<f32, GuiExt>>) {
    with_dom(|dom| {
      if let Some(node) = dom.node(sink)
        && let Some(binding) = node.slots.iter().find(|binding| binding.slot == slot)
        && let Some(html) = node.html()
      {
        let _ = html
          .style()
          .set_property(&binding.custom_property(), &render_box_leaf(leaf));
      }
    });
  }

  fn update_focus_order(sink: u32, focus_order: Option<FocusOrder>) {
    with_dom(|dom| dom.write_focus_order(sink, focus_order));
  }

  fn update_access(sink: u32, access: &[AccessProp]) {
    with_dom(|dom| dom.write_access(sink, access));
  }

  fn create_control(props: &ControlProps<Self>) -> u32 {
    with_dom(|dom| {
      let sink = if let Some(native) = native_element(props.kind) {
        let realized = Realized::Control {
          native: Some(native),
          caption_beside: props.label_beside,
        };

        if native.is_bare() {
          let root = dom.create_element(native.tag, realized);

          if let Some(node) = dom.node(root)
            && let Some(element) = node.element()
          {
            write_native_attributes(element, native, props);

            if node.adopted {
              let element = element.clone();

              dom.note_live_state(root, &element, native);
            }
          }

          root
        }
        else {
          let root = dom.create_element("label", realized);
          let (input, adopted) = dom.take_element(native.tag);

          write_native_attributes(&input, native, props);

          let _ = input.set_attribute("class", NATIVE_CLASS);

          if adopted {
            dom.note_live_state(root, &input, native);
          }
          else if let Some(node) = dom.node(root) {
            let _ = node.dom.append_child(&input);
          }

          if let Some(node) = dom.node_mut(root) {
            node.carrier = Some(input);
          }

          root
        }
      }
      else {
        let (tag, attribute) = drawn_element(props.kind);
        let root = dom.create_element(
          tag,
          Realized::Control {
            native: None,
            caption_beside: props.label_beside,
          },
        );

        if let Some(element) = dom.node(root).and_then(Node::element) {
          if let Some((name, value)) = attribute {
            let _ = element.set_attribute(name, value);
          }

          if let Some(label) = &props.label {
            let _ = element.set_attribute("aria-label", label);
          }
        }

        root
      };

      if let Some(element) = dom.node(sink).and_then(Node::element) {
        let _ = element.set_attribute(CONTROL_ATTRIBUTE, "");

        if props.drags {
          let _ = element.set_attribute(DRAGS_ATTRIBUTE, "");
        }
      }

      let compiled = dom
        .sheet
        .compiled(control_fingerprint(props), || compile_control(props));

      dom.finish_box(sink, &props.box_props, &compiled);

      sink
    })
  }

  fn set_control_state(sink: u32, state: &ControlState) {
    with_dom(|dom| {
      let Some(node) = dom.node(sink)
      else {
        return;
      };
      let Some(root) = node.element()
      else {
        return;
      };

      match &node.carrier {
        None => {
          set_flag(root, "data-skene-checked", state.checked == Toggle::On);
          set_flag(
            root,
            "data-skene-indeterminate",
            state.checked == Toggle::Mixed,
          );
          set_flag(root, "data-skene-disabled", state.disabled);
          set_flag(root, "data-skene-invalid", state.invalid);

          if takes_disabled(&root.tag_name()) {
            set_flag(root, "disabled", state.disabled);
          }
        }
        Some(carrier) => {
          if let Some(input) = carrier.dyn_ref::<HtmlInputElement>() {
            input.set_checked(state.checked == Toggle::On);
            input.set_indeterminate(state.checked == Toggle::Mixed);
            input.set_disabled(state.disabled);
            set_flag(
              carrier,
              "data-skene-indeterminate",
              state.checked == Toggle::Mixed,
            );
          }
          else if let Some(area) = carrier.dyn_ref::<HtmlTextAreaElement>() {
            area.set_disabled(state.disabled);
          }

          if state.invalid {
            let _ = carrier.set_attribute("aria-invalid", "true");
          }
          else {
            let _ = carrier.remove_attribute("aria-invalid");
          }
        }
      }

      set_flag(root, "data-skene-open", state.open);
    });
  }

  fn create_overlay(props: &OverlayProps<Self>) -> u32 {
    with_dom(|dom| {
      let realized = Realized::Overlay {
        modal: props.modal,
        placement: props.placement,
        position: None,
      };
      let sink = if props.modal {
        let sink = dom.create_element("dialog", realized);

        if let Some(node) = dom.node_mut(sink)
          && let Some(element) = node.element()
        {
          node.cancel = Some(Listener::new(
            element.clone().into(),
            "cancel",
            false,
            move |event| {
              event.prevent_default();

              if let Some(node) = with_dom(|dom| dom.node(sink).and_then(|node| node.id)) {
                notify_dismiss::<Self>(node);
              }
            },
          ));
        }

        sink
      }
      else {
        let sink = dom.create_element("div", realized);

        if let Some(element) = dom.node(sink).and_then(Node::element) {
          let _ = element.set_attribute("popover", if props.hint { "hint" } else { "manual" });

          // A hint is a tooltip; a popup takes the role its box asks for.
          if props.hint {
            let _ = element.set_attribute("role", "tooltip");
          }
          else if let Some(role) = props.box_props.role.filter(|role| role.is_spelled()) {
            let _ = element.set_attribute("role", aria_role(role));
          }
        }

        sink
      };
      let compiled = dom
        .sheet
        .compiled(overlay_fingerprint(props), || compile_overlay(props));

      dom.finish_box(sink, &props.box_props, &compiled);

      sink
    })
  }

  fn set_overlay_open(sink: u32, open: bool, anchor: Option<u32>) {
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
      let element = node.element()?.clone();
      let anchor_element = anchor.and_then(|anchor| {
        let element = dom.node(anchor)?.element().cloned()?;

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

  fn set_overlay_position(sink: u32, position: Option<Point<f32>>) {
    with_dom(|dom| {
      if let Some(node) = dom.node_mut(sink)
        && let Realized::Overlay { position: held, .. } = &mut node.realized
      {
        *held = position;
      }
    });
  }

  fn set_overlay_placement(sink: u32, placement: Placement) {
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

  fn apply_layout(sink: u32, layout: &StaticBoxStyle<Self>) {
    with_dom(|dom| {
      let compiled = dom.sheet.compiled(layout_fingerprint::<Self>(layout), || {
        compile_layout::<Self>(layout)
      });

      dom.assign_style(sink, &compiled);
    });
  }

  fn create_run(props: &RunProps<Self>) -> u32 {
    with_dom(|dom| {
      let sink = match props.link.as_deref() {
        Some(href) => {
          let sink = dom.create_element("a", Realized::Run);

          if let Some(element) = dom.node(sink).and_then(Node::element) {
            for (name, value) in link_attributes(href, props.link_policy) {
              let _ = element.set_attribute(name, &value);
            }
          }

          sink
        }
        None => dom.create_element("span", Realized::Run),
      };
      let compiled = dom
        .sheet
        .compiled(run_fingerprint(props), || compile_run(props));

      dom.assign_style(sink, &compiled);

      sink
    })
  }

  fn update_run(sink: u32, slot: Slot, leaf: &StaticLeaf<RunValue<GuiExt>>) {
    with_dom(|dom| {
      if let Some(node) = dom.node(sink)
        && let Some(binding) = node.slots.iter().find(|binding| binding.slot == slot)
        && let Some(html) = node.html()
      {
        let _ = html
          .style()
          .set_property(&binding.custom_property(), &render_run_leaf(leaf));
      }
    });
  }

  fn set_text(sink: u32, text: &str) {
    with_dom(|dom| {
      if let Some(node) = dom.node(sink)
        && !(node.adopted && node.dom.text_content().as_deref() == Some(text))
      {
        node.dom.set_text_content(Some(text));
      }
    });
  }

  fn create_anchor() -> u32 {
    with_dom(|dom| {
      let text = anchor_text(dom.anchors);

      dom.anchors += 1;

      let (comment, adopted) = dom.take_comment(&text);

      dom.insert(Node::new(comment, Realized::Anchor, adopted))
    })
  }

  fn create_scope() -> u32 {
    with_dom(|dom| {
      let sink = dom.create_element("div", Realized::Scope { inline: Vec::new() });

      if let Some(node) = dom.node_mut(sink) {
        node.adopt_inline_definitions();
      }

      if let Some(html) = dom.node(sink).and_then(Node::html) {
        let _ = html.set_attribute(SCOPE_ATTRIBUTE, "");
        let _ = html.style().set_property("display", "contents");
      }

      sink
    })
  }

  fn attach(parent: u32, child: u32, before: Option<u32>) {
    let Some((parent, child, caption, before, saved)) = with_dom(|dom| {
      if dom.attach_caption(parent, child) {
        return None;
      }

      let parent = dom.node(parent)?.dom.clone();
      let child_node = dom.node(child)?;

      if child_node.adopted && child_node.dom.parent_node().as_ref() == Some(&parent) {
        return None;
      }

      let child = child_node.dom.clone();
      let caption = child_node.caption.clone();
      let saved = before.and_then(|before| dom.saved.remove(&before));
      let before = before
        .and_then(|before| dom.node(before))
        .map(|node| node.dom.clone());

      Some((parent, child, caption, before, saved))
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

    for (overlay, anchor) in with_dom(Dom::take_connected_opens) {
      Self::set_overlay_open(overlay, true, anchor);
    }
  }

  fn detach(parent: u32, child: u32) {
    let Some((parent, child, caption)) = with_dom(|dom| {
      if dom.detach_caption(parent, child) {
        return None;
      }

      Some((
        dom.node(parent)?.dom.clone(),
        dom.node(child)?.dom.clone(),
        dom.node(child)?.caption.clone(),
      ))
    })
    else {
      return;
    };

    if child.parent_node().as_ref() == Some(&parent) {
      with_dom(|dom| dom.save(std::slice::from_ref(&child), child.next_sibling()));

      let _ = parent.remove_child(&child);
    }

    if let Some(caption) = caption
      && caption.parent_node().as_ref() == Some(&parent)
    {
      let _ = parent.remove_child(&caption);
    }
  }

  fn destroy(sink: u32) {
    with_dom(|dom| {
      let Some(node) = dom.nodes.get_mut(sink as usize).and_then(Option::take)
      else {
        return;
      };

      dom.free.push(sink);

      for class in node.style_class.iter().chain(&node.token_class) {
        dom.sheet.release(class);
      }

      if !node.observers.is_empty()
        && let (Some((observer, _)), Some(element)) = (&dom.resize, node.element())
      {
        observer.unobserve(element);
      }

      if let (Some(pixels), Some(canvas)) = (&dom.pixels, &node.canvas) {
        pixels.unobserve(&canvas.element);
      }

      dom.saved.remove(&sink);
      dom.pending_open.remove(&sink);
    });
  }

  fn define_tokens(scope: u32, definitions: &Definitions<WebTokenValue>) {
    with_dom(|dom| {
      dom.sheet.register(definitions);

      let previous = dom.node(scope).and_then(|node| node.token_class.clone());

      if previous.is_some() && all_unconditional(definitions) {
        if let Some(node) = dom.node_mut(scope) {
          node.set_inline_definitions(inline_definitions(definitions));
        }

        return;
      }

      let class = dom
        .sheet
        .swap_class(&compile_definitions(definitions), previous.as_deref());

      if let Some(node) = dom.node_mut(scope) {
        if let Some(element) = node.element() {
          swap_class_attribute(element, previous.as_deref(), &class);
        }

        node.set_inline_definitions(Vec::new());
        node.token_class = Some(class);
      }
    });
  }

  fn define_root_tokens(definitions: &Definitions<WebTokenValue>) {
    with_dom(|dom| {
      dom.sheet.register(definitions);

      let previous = dom.root_class.take();
      let class = dom
        .sheet
        .swap_class(&compile_root_definitions(definitions), previous.as_deref());

      dom.root_class = Some(class);
    });
  }

  fn listen(_sink: u32, kind: EventKind) {
    with_dom(|dom| dom.events.ensure(kind));
  }

  fn localize_pointer(sink: u32, event: &PointerEvent) -> PointerEvent {
    let element = with_dom(|dom| dom.node(sink).and_then(Node::element).cloned());

    element.map_or(*event, |element| events::localized(event, &element))
  }

  fn focus(sink: u32) {
    let carrier = with_dom(|dom| dom.node(sink).and_then(Node::carrier).cloned());

    if let Some(html) = carrier
      .as_ref()
      .and_then(|carrier| carrier.dyn_ref::<HtmlElement>())
    {
      let _ = html.focus();
    }
  }

  fn blur(sink: u32) {
    let carrier = with_dom(|dom| dom.node(sink).and_then(Node::carrier).cloned());

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

  fn set_value(sink: u32, value: &str) {
    with_dom(|dom| {
      let Some(carrier) = dom.node(sink).and_then(Node::carrier)
      else {
        return;
      };
      let focused = dom.document.active_element().as_ref() == Some(carrier);
      let length = u32::try_from(value.encode_utf16().count()).unwrap_or(u32::MAX);

      if let Some(input) = carrier.dyn_ref::<HtmlInputElement>() {
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
      else if let Some(area) = carrier.dyn_ref::<HtmlTextAreaElement>() {
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
      else if let Some(select) = carrier.dyn_ref::<HtmlSelectElement>() {
        select.set_value(value);
      }
    });
  }

  fn set_options(sink: u32, options: &[PickerOption]) {
    with_dom(|dom| {
      let Some(select) = dom
        .node(sink)
        .and_then(Node::carrier)
        .and_then(|carrier| carrier.dyn_ref::<HtmlSelectElement>())
      else {
        return;
      };

      select.set_inner_html("");

      for option in options {
        let element = dom.document.create_element("option").expect("an option");

        let _ = element.set_attribute("value", &option.value);

        if option.disabled {
          let _ = element.set_attribute("disabled", "");
        }

        element.set_text_content(Some(&option.name));

        let _ = select.append_child(&element);
      }
    });
  }

  fn set_selection(sink: u32, selection: TextSelection) {
    with_dom(|dom| {
      let Some(carrier) = dom.node(sink).and_then(Node::carrier)
      else {
        return;
      };

      events::select(carrier, selection);
    });
  }

  fn set_checked(sink: u32, checked: bool) {
    with_dom(|dom| {
      if let Some(input) = dom
        .node(sink)
        .and_then(Node::carrier)
        .and_then(|carrier| carrier.dyn_ref::<HtmlInputElement>())
      {
        input.set_checked(checked);
      }
    });
  }

  fn observe_size(sink: u32, report: Rc<dyn Fn(Size<f32>)>) -> Observation {
    let kept = Rc::clone(&report);

    with_dom(|dom| {
      let observer = dom.resize_observer();
      let Some(node) = dom.node_mut(sink)
      else {
        return;
      };

      if let Some(element) = node.element() {
        if node.observers.is_empty() {
          observer.observe(element);
        }

        node.observers.push(report);
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
          .observers
          .retain(|observer| !Rc::ptr_eq(observer, &kept));

        if node.observers.is_empty()
          && let Some(element) = node.element()
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
        .document
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
        && let Some(window) = dom.document.default_view()
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
  fn create_element(tag: &'static str) -> u32 {
    with_dom(|dom| dom.create_element(tag, Realized::Box))
  }

  fn set_attribute(sink: u32, name: &str, value: &str) {
    with_dom(|dom| {
      if let Some(element) = dom.node(sink).and_then(Node::element) {
        let _ = element.set_attribute(name, value);
      }
    });
  }

  fn set_declarations(sink: u32, declarations: &Declarations) {
    with_dom(|dom| {
      if let Some(html) = dom.node(sink).and_then(Node::html) {
        let style = html.style();

        for (property, value) in declarations.iter() {
          let _ = style.set_property(property, value);
        }
      }
    });
  }

  fn set_text(sink: u32, text: &str) {
    <Self as Platform>::set_text(sink, text);
  }
}

impl WebDom for Web {
  fn element(sink: u32) -> Option<Element> {
    with_dom(|dom| dom.node(sink).and_then(Node::element).cloned())
  }

  fn document() -> Document {
    document()
  }
}

/// Installs the browser leaf for this thread, rooted at `root`, adopting the server's
/// nodes under it when `hydrating`.
fn install_at(root: Element, hydrating: bool) {
  skene_reactive::Runtime::set_scheduler(|flush| {
    TURN.with(|turn| *turn.borrow_mut() = Some(flush));
    wasm_bindgen_futures::spawn_local(async { end_turn() });
  });
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

    *dom = Some(Dom::new(document, root, hydration));
  });

  let root_node = Scene::<Web>::with(|scene| scene.create(NodeKind::Box, ROOT));

  with_dom(|dom| dom.root_node = Some(root_node));
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
    let root = with_dom(|dom| dom.root_node).expect("the root node exists once installed");

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
    let Some(node) = with_dom(|dom| dom.node(sink).and_then(|node| node.id))
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

    Some((scene.sink(start)?, scene.sink(end)?))
  })
  else {
    return Vec::new();
  };

  with_dom(|dom| {
    let (Some(start), Some(end)) = (
      dom.node(start).map(|node| node.dom.clone()),
      dom.node(end).map(|node| node.dom.clone()),
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
    let Some(hydration) = dom.hydration.as_mut()
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
  with_dom(|dom| dom.document.clone())
}

/// The element realizing `node`; `None` for an anchor or a node no longer live.
#[must_use]
pub fn element(node: NodeId) -> Option<Element> {
  let sink = Scene::<Web>::with(|scene| scene.record(node).map(|record| *record.sink()))?;

  with_dom(|dom| dom.node(sink).and_then(Node::element).cloned())
}

/// Uninstalls the browser leaf: removes its stylesheet and listeners, empties the
/// scene and resets the reactive runtime, so one thread can run many independent
/// mounts.
pub fn uninstall() {
  DOM.with(|dom| {
    if let Some(mut dom) = dom.borrow_mut().take() {
      dom.sheet.remove_all();

      if let Some((observer, _)) = &dom.resize {
        observer.disconnect();
      }
    }
  });
  media::reset();
  view_transition::reset();
  head::reset();
  Scene::<Web>::with(|scene| *scene = Scene::new());
  skene_reactive::Runtime::reset();
  skene_core::reset_root_owner();
  TURN.with(|turn| *turn.borrow_mut() = None);
}
