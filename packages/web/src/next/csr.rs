//! The browser leaf's realization of the driver: a DOM sink under one mount root, the
//! stylesheet mirrored from the shared interner, root-delegated events routed through
//! core's dispatch, hydration of the server leaf's markup, and the environment as
//! signals.
//!
//! One record per node sits beside the scene, holding the DOM node, the native
//! element carrying a control's state, the classes and slots the node was compiled to,
//! and what it listens for; the scene alone holds structure. Elements are the same
//! choices the server leaf makes, so a served page adopts by its creation-order
//! markers and its anchor comments without a rebuild.

mod events;
mod media;
mod sheet;

use std::{cell::RefCell, rc::Rc, thread::LocalKey};

use rustc_hash::FxHashMap;
use skene_core::{
  AccessProp,
  FocusOrder,
  Placement,
  Size,
  Toggle,
  next::{
    BoxProps,
    BoxValue,
    ControlProps,
    ControlState,
    Definitions,
    Driver,
    EventKind,
    IntoView,
    MediaSignals,
    NodeId,
    NodeKind,
    OverlayProps,
    PixelExt,
    RunProps,
    RunValue,
    Scene,
    Slot,
    View,
    notify_change,
    notify_dismiss,
    notify_toggle,
  },
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{
  Document,
  Element,
  HtmlDialogElement,
  HtmlElement,
  HtmlInputElement,
  HtmlTextAreaElement,
  ResizeObserver,
  ResizeObserverEntry,
};

pub use self::media::{media_query, set_scheme, stored_scheme, watch_media};
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
    compile_overlay,
    compile_root_definitions,
    compile_run,
    inline_definitions,
    overlay_fingerprint,
    render_box_value,
    render_run_value,
    run_fingerprint,
  },
  condition::CONTROL_ATTRIBUTE,
  markup::{
    MARKER_ATTRIBUTE,
    NATIVE_CLASS,
    NativeElement,
    STYLE_ID,
    access_attribute,
    anchor_text,
    drawn_element,
    element_tag,
    focus_order_attribute,
    link_attributes,
    native_element,
  },
  token::WebTokenValue,
};

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
  Control { native: Option<NativeElement> },
  Overlay { modal: bool, placement: Placement },
  Run,
  Anchor,
  Scope,
}

/// One node's record beside the scene: its DOM node, the native element carrying a
/// control's state, focus and value when the control has one apart from its root, and
/// what the driver keeps per node.
struct Node {
  dom: web_sys::Node,
  carrier: Option<Element>,
  realized: Realized,
  id: Option<NodeId>,
  style_class: Option<Rc<str>>,
  token_class: Option<Rc<str>>,
  slots: Rc<[SlotBinding]>,
  listened: u16,
  observers: Vec<Rc<dyn Fn(Size<f32>)>>,
  cancel: Option<Listener>,
  adopted: bool,
}

impl Node {
  fn new(dom: impl Into<web_sys::Node>, realized: Realized, adopted: bool) -> Self {
    Self {
      dom: dom.into(),
      carrier: None,
      realized,
      id: None,
      style_class: None,
      token_class: None,
      slots: Rc::from([]),
      listened: 0,
      observers: Vec::new(),
      cancel: None,
      adopted,
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
}

/// What the reader changed on a native element before the client arrived.
enum Pending {
  Toggle(bool),
  Change(String),
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
  document: Document,
  nodes: Vec<Option<Node>>,
  free: Vec<u32>,
  markers: u32,
  anchors: u32,
  sheet: Sheet,
  events: Events,
  root_class: Option<Rc<str>>,
  hydration: Option<Hydration>,
  mismatches: u32,
  resize: Option<SizeObserver>,
  root_node: Option<NodeId>,
}

thread_local! {
  static SCENE: RefCell<Scene<Web>> = RefCell::new(Scene::new());
  static DOM: RefCell<Option<Dom>> = const { RefCell::new(None) };
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
        .expect("the web driver to be installed for this thread"),
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

  fn assign_style(&mut self, sink: u32, compiled: &Rc<Compiled>) {
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
      match focus_order_attribute(focus_order, node.is_control()) {
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

    for index in 0..names.length() {
      if let Some(name) = names.get(index).as_string()
        && name.starts_with("aria-")
        && name != "aria-label"
      {
        let _ = carrier.remove_attribute(&name);
      }
    }

    for prop in access {
      let (name, value) = access_attribute(prop);

      let _ = carrier.set_attribute(name, &value);
    }
  }

  /// Realizes the props every box-like node shares onto `sink`.
  fn finish_box(&mut self, sink: u32, props: &BoxProps<Web>, compiled: &Rc<Compiled>) {
    self.assign_style(sink, compiled);
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

        #[expect(clippy::cast_precision_loss)]
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

/// Writes `on` as the presence of the attribute `name` on `element`.
fn set_flag(element: &Element, name: &str, on: bool) {
  if on {
    let _ = element.set_attribute(name, "");
  }
  else {
    let _ = element.remove_attribute(name);
  }
}

/// Positions an open popup against its anchor: below it for `Bottom`, above it for
/// `Top`, start-aligned either way; other placements keep their viewport position.
fn position_at_anchor(popup: &HtmlElement, anchor: &Element, placement: Placement) {
  let rect = anchor.get_bounding_client_rect();
  let style = popup.style();
  let top = match placement {
    Placement::Top => rect.top() - f64::from(popup.offset_height()),
    Placement::Bottom => rect.bottom(),
    _ => return,
  };

  let _ = style.set_property("inset", "auto");
  let _ = style.set_property("margin", "0");
  let _ = style.set_property("position", "fixed");
  let _ = style.set_property("left", &format!("{}px", rect.left()));
  let _ = style.set_property("top", &format!("{top}px"));
}

impl Driver for Web {
  type Scalar = f32;
  type Sink = u32;
  type StyleExt = PixelExt;
  type TokenValue = WebTokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn bind_node(&self, sink: &u32, node: NodeId) {
    with_dom(|dom| {
      if let Some(record) = dom.node_mut(*sink) {
        record.id = Some(node);
      }
    });
  }

  fn create_box(&self, props: &BoxProps<Self>) -> u32 {
    with_dom(|dom| {
      let sink = dom.create_element(element_tag(props.role), Realized::Box);

      if let Some(role) = props.role.filter(|role| role.is_widget())
        && let Some(element) = dom.node(sink).and_then(Node::element)
      {
        let _ = element.set_attribute("role", crate::aria_role(role));
      }

      let compiled = dom
        .sheet
        .compiled(box_fingerprint(props, 0), || compile_box(props));

      dom.finish_box(sink, props, &compiled);

      sink
    })
  }

  fn update_box(&self, sink: &u32, slot: Slot, value: &BoxValue<f32, PixelExt>) {
    with_dom(|dom| {
      if let Some(node) = dom.node(*sink)
        && let Some(binding) = node.slots.iter().find(|binding| binding.slot == slot)
        && let Some(html) = node.html()
      {
        let _ = html.style().set_property(
          &binding.custom_property(),
          &render_box_value(binding.property, value),
        );
      }
    });
  }

  fn update_focus_order(&self, sink: &u32, focus_order: Option<FocusOrder>) {
    with_dom(|dom| dom.write_focus_order(*sink, focus_order));
  }

  fn update_access(&self, sink: &u32, access: &[AccessProp]) {
    with_dom(|dom| dom.write_access(*sink, access));
  }

  fn create_control(&self, props: &ControlProps<Self>) -> u32 {
    with_dom(|dom| {
      let sink = if let Some(native) = native_element(props.kind) {
        let root = dom.create_element(
          "label",
          Realized::Control {
            native: Some(native),
          },
        );
        let (input, adopted) = dom.take_element(native.tag);

        if let Some(kind) = native.kind {
          let _ = input.set_attribute("type", kind);
        }

        let _ = input.set_attribute("class", NATIVE_CLASS);

        if let Some(placeholder) = &props.placeholder {
          let _ = input.set_attribute("placeholder", placeholder);
        }

        if native.tag == "textarea" {
          let _ = input.set_attribute("rows", &props.lines.to_string());
        }

        if let Some(label) = &props.label {
          let _ = input.set_attribute("aria-label", label);
        }

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
      else {
        let (tag, attribute) = drawn_element(props.kind);
        let root = dom.create_element(tag, Realized::Control { native: None });

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
      }

      let compiled = dom
        .sheet
        .compiled(box_fingerprint(&props.box_props, 1), || {
          compile_control(props)
        });

      dom.finish_box(sink, &props.box_props, &compiled);

      sink
    })
  }

  fn set_control_state(&self, sink: &u32, state: &ControlState) {
    with_dom(|dom| {
      let Some(node) = dom.node(*sink)
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

          if root.tag_name().eq_ignore_ascii_case("button") {
            set_flag(root, "disabled", state.disabled);
          }
        }
        Some(carrier) => {
          if let Some(input) = carrier.dyn_ref::<HtmlInputElement>() {
            input.set_checked(state.checked == Toggle::On);
            input.set_indeterminate(state.checked == Toggle::Mixed);
            input.set_disabled(state.disabled);
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

  fn create_overlay(&self, props: &OverlayProps<Self>) -> u32 {
    with_dom(|dom| {
      let realized = Realized::Overlay {
        modal: props.modal,
        placement: props.placement,
      };
      let sink = if props.modal {
        dom.create_element("dialog", realized)
      }
      else {
        let sink = dom.create_element("div", realized);

        if let Some(element) = dom.node(sink).and_then(Node::element) {
          let _ = element.set_attribute("popover", "manual");
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

  fn set_overlay_open(&self, sink: &u32, open: bool, anchor: Option<&u32>) {
    let Some((element, modal, placement, anchor)) = with_dom(|dom| {
      let node = dom.node(*sink)?;
      let Realized::Overlay { modal, placement } = node.realized
      else {
        return None;
      };
      let element = node.element()?.clone();
      let anchor_element = anchor.and_then(|anchor| dom.node(*anchor)?.element().cloned());

      if !modal {
        if open {
          dom.events.open_popups.insert(*sink, anchor.copied());
        }
        else {
          dom.events.open_popups.remove(sink);
        }
      }

      Some((element, modal, placement, anchor_element))
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

      if let Some(anchor) = anchor {
        position_at_anchor(html, &anchor, placement);
      }
    }
    else {
      let _ = html.hide_popover();
    }
  }

  fn set_overlay_placement(&self, sink: &u32, placement: Placement) {
    with_dom(|dom| {
      if let Some(node) = dom.node_mut(*sink)
        && let Realized::Overlay {
          placement: held, ..
        } = &mut node.realized
      {
        *held = placement;
      }
    });
  }

  fn create_run(&self, props: &RunProps<Self>) -> u32 {
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

  fn update_run(&self, sink: &u32, slot: Slot, value: &RunValue<PixelExt>) {
    with_dom(|dom| {
      if let Some(node) = dom.node(*sink)
        && let Some(binding) = node.slots.iter().find(|binding| binding.slot == slot)
        && let Some(html) = node.html()
      {
        let _ = html.style().set_property(
          &binding.custom_property(),
          &render_run_value(binding.property, value),
        );
      }
    });
  }

  fn set_text(&self, sink: &u32, text: &str) {
    with_dom(|dom| {
      if let Some(node) = dom.node(*sink)
        && !(node.adopted && node.dom.text_content().as_deref() == Some(text))
      {
        node.dom.set_text_content(Some(text));
      }
    });
  }

  fn create_anchor(&self) -> u32 {
    with_dom(|dom| {
      let text = anchor_text(dom.anchors);

      dom.anchors += 1;

      let (comment, adopted) = dom.take_comment(&text);

      dom.insert(Node::new(comment, Realized::Anchor, adopted))
    })
  }

  fn create_scope(&self) -> u32 {
    with_dom(|dom| {
      let sink = dom.create_element("div", Realized::Scope);

      if let Some(html) = dom.node(sink).and_then(Node::html) {
        let _ = html.style().set_property("display", "contents");
      }

      sink
    })
  }

  fn attach(&self, parent: &u32, child: &u32, before: Option<&u32>) {
    let Some((parent, child, before)) = with_dom(|dom| {
      let parent = dom.node(*parent)?.dom.clone();
      let child_node = dom.node(*child)?;

      if child_node.adopted && child_node.dom.parent_node().as_ref() == Some(&parent) {
        return None;
      }

      let before = before
        .and_then(|before| dom.node(*before))
        .map(|node| node.dom.clone());

      Some((parent, child_node.dom.clone(), before))
    })
    else {
      return;
    };

    let _ = parent.insert_before(&child, before.as_ref());
  }

  fn detach(&self, parent: &u32, child: &u32) {
    let Some((parent, child)) = with_dom(|dom| {
      Some((
        dom.node(*parent)?.dom.clone(),
        dom.node(*child)?.dom.clone(),
      ))
    })
    else {
      return;
    };

    if child.parent_node().as_ref() == Some(&parent) {
      let _ = parent.remove_child(&child);
    }
  }

  fn destroy(&self, sink: u32) {
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

      dom.events.open_popups.remove(&sink);
    });
  }

  fn define_tokens(&self, scope: &u32, definitions: &Definitions<WebTokenValue>) {
    with_dom(|dom| {
      dom.sheet.register(definitions);

      let previous = dom.node(*scope).and_then(|node| node.token_class.clone());

      if previous.is_some() && all_unconditional(definitions) {
        if let Some(html) = dom.node(*scope).and_then(Node::html) {
          let style = html.style();

          for (property, value) in inline_definitions(definitions) {
            let _ = style.set_property(&property, &value);
          }
        }

        return;
      }

      let class = dom
        .sheet
        .swap_class(&compile_definitions(definitions), previous.as_deref());

      if let Some(node) = dom.node_mut(*scope) {
        if let Some(element) = node.element() {
          swap_class_attribute(element, previous.as_deref(), &class);
        }

        node.token_class = Some(class);
      }
    });
  }

  fn define_root_tokens(&self, definitions: &Definitions<WebTokenValue>) {
    with_dom(|dom| {
      dom.sheet.register(definitions);

      let previous = dom.root_class.take();
      let class = dom
        .sheet
        .swap_class(&compile_root_definitions(definitions), previous.as_deref());

      dom.root_class = Some(class);
    });
  }

  fn listen(&self, sink: &u32, kind: EventKind) {
    with_dom(|dom| {
      dom.events.ensure(kind);

      let sink = *sink;
      let Some(node) = dom.node_mut(sink)
      else {
        return;
      };

      node.listened |= 1 << (kind as u16);

      if kind == EventKind::Dismiss
        && node.cancel.is_none()
        && matches!(node.realized, Realized::Overlay { modal: true, .. })
        && let Some(element) = node.element()
      {
        node.cancel = Some(Listener::new(
          element.clone().into(),
          "cancel",
          false,
          move |event| {
            event.prevent_default();

            if let Some(node) = with_dom(|dom| dom.node(sink).and_then(|node| node.id)) {
              notify_dismiss::<Web>(node);
            }
          },
        ));
      }
    });
  }

  fn focus(&self, sink: &u32) {
    let carrier = with_dom(|dom| dom.node(*sink).and_then(Node::carrier).cloned());

    if let Some(html) = carrier
      .as_ref()
      .and_then(|carrier| carrier.dyn_ref::<HtmlElement>())
    {
      let _ = html.focus();
    }
  }

  fn blur(&self, sink: &u32) {
    let carrier = with_dom(|dom| dom.node(*sink).and_then(Node::carrier).cloned());

    if let Some(html) = carrier
      .as_ref()
      .and_then(|carrier| carrier.dyn_ref::<HtmlElement>())
    {
      let _ = html.blur();
    }
  }

  fn set_value(&self, sink: &u32, value: &str) {
    with_dom(|dom| {
      let Some(carrier) = dom.node(*sink).and_then(Node::carrier)
      else {
        return;
      };

      if let Some(input) = carrier.dyn_ref::<HtmlInputElement>() {
        input.set_value(value);
      }
      else if let Some(area) = carrier.dyn_ref::<HtmlTextAreaElement>() {
        area.set_value(value);
      }
    });
  }

  fn set_checked(&self, sink: &u32, checked: bool) {
    with_dom(|dom| {
      if let Some(input) = dom
        .node(*sink)
        .and_then(Node::carrier)
        .and_then(|carrier| carrier.dyn_ref::<HtmlInputElement>())
      {
        input.set_checked(checked);
      }
    });
  }

  fn observe_size(&self, sink: &u32, report: Rc<dyn Fn(Size<f32>)>) {
    with_dom(|dom| {
      let observer = dom.resize_observer();
      let Some(node) = dom.node_mut(*sink)
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
  }

  fn media(&self) -> MediaSignals<f32> {
    media::signals()
  }
}

/// Installs the browser leaf for this thread, rooted at `root`, adopting the server's
/// nodes under it when `hydrating`.
fn install_at(root: Element, hydrating: bool) {
  skene_reactive::Runtime::init_on_ui_thread();

  let document = web_sys::window()
    .expect("a window")
    .document()
    .expect("a document");
  let hydration = hydrating.then(|| Hydration::seed(&root));

  DOM.with(|dom| {
    let mut dom = dom.borrow_mut();

    assert!(
      dom.is_none(),
      "the web driver is already installed for this thread",
    );

    *dom = Some(Dom::new(document, root, hydration));
  });

  let root_node = Scene::<Web>::with(|scene| scene.create(NodeKind::Box, ROOT));

  with_dom(|dom| dom.root_node = Some(root_node));
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

/// Builds `view` under the installed root, returning its node.
///
/// # Panics
///
/// Panics if the driver is not installed for this thread.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  skene_core::root_owner().enter(|| {
    let node = view();
    let root = with_dom(|dom| dom.root_node).expect("the root node exists once installed");

    Scene::<Web>::with(|scene| scene.attach(root, node, None));

    node
  })
}

/// Hydrates a server-rendered tree under `root`, adopting its elements by their
/// creation-order markers and its anchors by their comments, the server's `<style>`
/// block in place of a fresh one, and the resource `state` serialized alongside them.
///
/// A native element whose live value or checked state differs from the serialized one
/// reports the difference through the control's change or toggle path once the tree
/// is built. Every mismatch is reported to the console and counted in
/// [`hydration_mismatches`].
///
/// # Panics
///
/// Panics if the driver is already installed for this thread.
pub fn hydrate<V, F>(root: Element, state: &str, app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
{
  install_at(root, true);
  crate::csr::stage_hydration_state(state);
  crate::csr::set_settled(false);
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

  crate::csr::set_settled(true);
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

  let _ = tracing::subscriber::set_global_default(crate::csr::logging::ConsoleSubscriber);

  let document = web_sys::window()
    .expect("a window")
    .document()
    .expect("a document");
  let body = document.body().expect("a body");
  let state = document
    .get_element_by_id(crate::STATE_SCRIPT_ID)
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

  if document.get_element_by_id(crate::STATE_SCRIPT_ID).is_some() {
    hydrate_page(app);

    return;
  }

  console_error_panic_hook::set_once();

  let _ = tracing::subscriber::set_global_default(crate::csr::logging::ConsoleSubscriber);

  let body = document.body().expect("a body");

  install(body.into());

  let _ = mount(|| app().into_view().build());
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
#[cfg(feature = "test")]
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
  Scene::<Web>::with(|scene| *scene = Scene::new());
  skene_reactive::Runtime::reset();
  skene_core::reset_root_owner();
}
