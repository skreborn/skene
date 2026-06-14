//! # skene DOM backend
//!
//! A [`Backend`] implementation that realizes skene's node tree against the
//! browser DOM, compiled to WebAssembly. Layout, painting, text shaping and
//! hit-testing are all offloaded to the browser; this crate only creates
//! elements, assigns styles (via a shared stylesheet — see
//! [`stylesheet`]), wires DOM event listeners back to the core, and mounts the
//! root view with [`launch`].

mod css;
mod stylesheet;

use std::{cell::RefCell, rc::Rc};

use rustc_hash::FxHashMap;
use skene_core::IntoView;
use skene_renderer::{
  Backend,
  ElementKind,
  Event,
  EventKind,
  EventSink,
  KeyEvent,
  Modifiers,
  NodeId,
  PointerButton,
  PointerEvent,
  Style,
};
use stylesheet::StyleStore;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element};

const ROOT_ID: NodeId = NodeId::from_raw(0);

/// The DOM backend. Owns the element registry, the stylesheet store and the
/// live event-listener closures.
pub struct DomBackend {
  document: Document,
  nodes: FxHashMap<NodeId, Element>,
  /// The generated class currently assigned to each node (so it can be swapped).
  node_class: FxHashMap<NodeId, String>,
  styles: StyleStore,
  next_id: u64,
  sink: Option<EventSink>,
  /// Keeps event-listener closures alive for the lifetime of the backend.
  closures: Vec<Closure<dyn FnMut(web_sys::Event)>>,
}

impl DomBackend {
  /// Create a backend mounted at `mount` (the element new views are inserted
  /// under).
  ///
  /// # Panics
  ///
  /// Panics if there is no `window`/`document` (i.e. not running in a browser).
  #[must_use]
  pub fn new(mount: Element) -> DomBackend {
    let document = web_sys::window()
      .expect("no window")
      .document()
      .expect("no document");
    let styles = StyleStore::new(&document);
    let mut nodes = FxHashMap::default();
    nodes.insert(ROOT_ID, mount);
    DomBackend {
      document,
      nodes,
      node_class: FxHashMap::default(),
      styles,
      next_id: 1,
      sink: None,
      closures: Vec::new(),
    }
  }

  fn alloc_id(&mut self) -> NodeId {
    let id = NodeId::from_raw(self.next_id);
    self.next_id += 1;
    id
  }

  fn element(&self, node: NodeId) -> &Element {
    self.nodes.get(&node).expect("DOM backend: unknown node id")
  }
}

fn tag_for(kind: ElementKind) -> &'static str {
  match kind {
    ElementKind::Box => "div",
    ElementKind::Text => "span",
    ElementKind::Button => "button",
  }
}

fn dom_event_name(kind: EventKind) -> &'static str {
  match kind {
    EventKind::Click => "click",
    EventKind::PointerDown => "pointerdown",
    EventKind::PointerUp => "pointerup",
    EventKind::PointerMove => "pointermove",
    EventKind::KeyDown => "keydown",
    EventKind::KeyUp => "keyup",
    EventKind::Input => "input",
  }
}

fn pointer_button(button: i16) -> PointerButton {
  match button {
    0 => PointerButton::Primary,
    1 => PointerButton::Middle,
    2 => PointerButton::Secondary,
    n => PointerButton::Other(u16::try_from(n).unwrap_or(0)),
  }
}

/// Convert a DOM event into the abstract [`Event`] for the given kind.
fn convert_event(kind: EventKind, ev: &web_sys::Event) -> Option<Event> {
  match kind {
    EventKind::Click | EventKind::PointerDown | EventKind::PointerUp | EventKind::PointerMove => {
      let me = ev.dyn_ref::<web_sys::MouseEvent>()?;
      let pe = PointerEvent {
        x: f64::from(me.offset_x()),
        y: f64::from(me.offset_y()),
        button: pointer_button(me.button()),
        modifiers: Modifiers {
          shift: me.shift_key(),
          ctrl: me.ctrl_key(),
          alt: me.alt_key(),
          meta: me.meta_key(),
        },
      };
      Some(match kind {
        EventKind::Click => Event::Click(pe),
        EventKind::PointerDown => Event::PointerDown(pe),
        EventKind::PointerUp => Event::PointerUp(pe),
        _ => Event::PointerMove(pe),
      })
    }
    EventKind::KeyDown | EventKind::KeyUp => {
      let ke = ev.dyn_ref::<web_sys::KeyboardEvent>()?;
      let event = KeyEvent {
        key: ke.key(),
        code: ke.code(),
        modifiers: Modifiers {
          shift: ke.shift_key(),
          ctrl: ke.ctrl_key(),
          alt: ke.alt_key(),
          meta: ke.meta_key(),
        },
      };
      Some(match kind {
        EventKind::KeyDown => Event::KeyDown(event),
        _ => Event::KeyUp(event),
      })
    }
    EventKind::Input => {
      let target = ev.target()?;
      let input = target.dyn_ref::<web_sys::HtmlInputElement>()?;
      Some(Event::Input(input.value()))
    }
  }
}

impl Backend for DomBackend {
  fn root(&self) -> NodeId {
    ROOT_ID
  }

  fn create_element(&mut self, kind: ElementKind) -> NodeId {
    let element = self
      .document
      .create_element(tag_for(kind))
      .expect("create element");
    let id = self.alloc_id();
    self.nodes.insert(id, element);
    id
  }

  fn set_style(&mut self, node: NodeId, style: &Style) {
    let declarations = css::declarations(style);
    let class = self.styles.class_for(&declarations);

    // No change? leave the DOM alone.
    if self.node_class.get(&node) == Some(&class) {
      return;
    }

    let element = self.element(node);
    let list = element.class_list();
    if let Some(old) = self.node_class.get(&node) {
      let _ = list.remove_1(old);
    }
    let _ = list.add_1(&class);
    self.node_class.insert(node, class);
  }

  fn set_text(&mut self, node: NodeId, text: &str) {
    self.element(node).set_text_content(Some(text));
  }

  fn set_attribute(&mut self, node: NodeId, name: &str, value: Option<&str>) {
    let element = self.element(node);
    match value {
      Some(v) => {
        let _ = element.set_attribute(name, v);
      }
      None => {
        let _ = element.remove_attribute(name);
      }
    }
  }

  fn insert_child(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    let parent_el = self.element(parent);
    let child_el = self.element(child);
    match before {
      Some(before) => {
        let before_el = self.element(before);
        let _ = parent_el.insert_before(child_el.as_ref(), Some(before_el.as_ref()));
      }
      None => {
        let _ = parent_el.append_child(child_el.as_ref());
      }
    }
  }

  fn remove_child(&mut self, parent: NodeId, child: NodeId) {
    let parent_el = self.element(parent);
    let child_el = self.element(child);
    let _ = parent_el.remove_child(child_el.as_ref());
  }

  fn destroy(&mut self, node: NodeId) {
    self.nodes.remove(&node);
    self.node_class.remove(&node);
  }

  fn listen(&mut self, node: NodeId, kind: EventKind) {
    let Some(sink) = self.sink.clone()
    else {
      return;
    };
    let element = self.element(node).clone();
    let closure = Closure::wrap(Box::new(move |ev: web_sys::Event| {
      if let Some(event) = convert_event(kind, &ev) {
        sink(node, event);
      }
    }) as Box<dyn FnMut(web_sys::Event)>);
    let _ = element
      .add_event_listener_with_callback(dom_event_name(kind), closure.as_ref().unchecked_ref());
    self.closures.push(closure);
  }

  fn set_event_sink(&mut self, sink: EventSink) {
    self.sink = Some(sink);
  }
}

/// Mount and run a skene application in the browser.
///
/// Installs a [`DomBackend`] rooted at `<body>`, builds `app` as the root view,
/// and returns. The reactive runtime, event listeners and view tree are kept
/// alive for the lifetime of the page.
///
/// # Panics
///
/// Panics if there is no `window`, `document` or `<body>` (i.e. not running in
/// a browser).
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  console_error_panic_hook::set_once();

  let window = web_sys::window().expect("no window");
  let document = window.document().expect("no document");
  let body = document.body().expect("document has a <body>");

  let backend = DomBackend::new(body.into());
  let backend: Rc<RefCell<dyn Backend>> = Rc::new(RefCell::new(backend));
  skene_core::set_backend(backend);
  skene_core::mount(app);
}
