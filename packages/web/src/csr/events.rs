//! Event delivery for the browser leaf.
//!
//! Bubbling events (click, focusin/focusout, keydown, input) are **delegated**: one
//! listener per event type on the mount root dispatches by walking from the event target
//! up through the tree, running each carrying node's handlers—so a node costs no
//! listener, and an event on a descendant reaches its ancestors exactly as per-node
//! listeners would. Non-bubbling events (pointerenter/pointerleave) attach directly to
//! their node. Every dispatch runs inside a reactive batch, so however many signals a
//! handler writes, each dependent effect runs once.

use std::{cell::RefCell, rc::Rc};

use rustc_hash::FxHashMap;
use skene_core::{NodeId, Prevent, Prevention};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::Element;

/// The property stamped on every element carrying its node id, read back during a
/// delegated dispatch.
const NODE_ID_PROPERTY: &str = "__skene";

/// A live DOM listener callback, kept alive for its registration's lifetime.
type Listener = Closure<dyn FnMut(web_sys::Event)>;

/// The handlers registered for one delegated event type, keyed by node.
pub(super) type Handlers<F> = Rc<RefCell<FxHashMap<NodeId, Vec<Rc<F>>>>>;

/// The browser leaf's event state: per-kind handler registries and the live listeners.
pub(super) struct Events {
  pub(super) click: Handlers<skene_core::ClickHandler>,
  pub(super) focus: Handlers<dyn Fn()>,
  pub(super) blur: Handlers<dyn Fn()>,
  pub(super) key_down: Handlers<skene_core::KeyHandler>,
  pub(super) change: Handlers<dyn Fn(&str)>,
  pub(super) toggle: Handlers<dyn Fn(bool)>,
  pub(super) pointer_down: Handlers<skene_core::PointerHandler>,
  pub(super) pointer_up: Handlers<skene_core::PointerHandler>,
  pub(super) pointer_move: Handlers<skene_core::PointerHandler>,
  /// One live listener per delegated event type, attached to the mount root.
  root_listeners: RefCell<FxHashMap<&'static str, Listener>>,
  /// The live listeners of the non-bubbling, per-node events.
  direct_listeners: RefCell<FxHashMap<NodeId, Vec<Listener>>>,
}

impl Events {
  pub(super) fn new() -> Self {
    Self {
      click: Rc::new(RefCell::new(FxHashMap::default())),
      focus: Rc::new(RefCell::new(FxHashMap::default())),
      blur: Rc::new(RefCell::new(FxHashMap::default())),
      key_down: Rc::new(RefCell::new(FxHashMap::default())),
      change: Rc::new(RefCell::new(FxHashMap::default())),
      toggle: Rc::new(RefCell::new(FxHashMap::default())),
      pointer_down: Rc::new(RefCell::new(FxHashMap::default())),
      pointer_up: Rc::new(RefCell::new(FxHashMap::default())),
      pointer_move: Rc::new(RefCell::new(FxHashMap::default())),
      root_listeners: RefCell::new(FxHashMap::default()),
      direct_listeners: RefCell::new(FxHashMap::default()),
    }
  }

  /// Installs the delegated listener for `event_name` on `root`, once. `dispatch` runs
  /// for each node on the path from the event target up—receiving that node's own
  /// element—inside one reactive batch; its folded verdicts stop the walk and, for the
  /// default, call `preventDefault` so the browser's built-in response is suppressed
  /// too.
  pub(super) fn ensure_root_listener(
    &self,
    root: &Element,
    event_name: &'static str,
    dispatch: impl Fn(NodeId, &Element, &web_sys::Event) -> Prevention + 'static,
  ) {
    let mut listeners = self.root_listeners.borrow_mut();

    if listeners.contains_key(event_name) {
      return;
    }

    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
      let mut current = event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok());

      skene_reactive::Effect::batch(|| {
        let mut prevention = Prevention::default();

        while let Some(element) = current {
          if let Some(id) = node_id_of(&element) {
            prevention.merge(dispatch(id, &element, &event));

            if prevention.bubbling {
              break;
            }
          }

          current = element.parent_element();
        }

        if prevention.default {
          event.prevent_default();
        }
      });
    }) as Box<dyn FnMut(web_sys::Event)>);

    let _ = root.add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref());

    listeners.insert(event_name, closure);
  }

  /// Attaches a per-node listener for a non-bubbling `event_name`, running `handler`
  /// with the raw event inside a reactive batch, until the node is destroyed.
  pub(super) fn add_direct_listener(
    &self,
    node: NodeId,
    element: &Element,
    event_name: &str,
    handler: impl Fn(&web_sys::Event) + 'static,
  ) {
    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
      skene_reactive::Effect::batch(|| handler(&event));
    }) as Box<dyn FnMut(web_sys::Event)>);

    let _ = element.add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref());

    self
      .direct_listeners
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(closure);
  }

  /// Releases every registration for `node`. Dropping a listener closure detaches it.
  pub(super) fn remove(&self, node: NodeId) {
    self.click.borrow_mut().remove(&node);
    self.focus.borrow_mut().remove(&node);
    self.blur.borrow_mut().remove(&node);
    self.key_down.borrow_mut().remove(&node);
    self.change.borrow_mut().remove(&node);
    self.toggle.borrow_mut().remove(&node);
    self.pointer_down.borrow_mut().remove(&node);
    self.pointer_up.borrow_mut().remove(&node);
    self.pointer_move.borrow_mut().remove(&node);
    self.direct_listeners.borrow_mut().remove(&node);
  }
}

/// Runs `node`'s handlers from `handlers` with `payload`, cloning them out first so a
/// handler may re-enter the platform and build or destroy nodes.
pub(super) fn run_handlers<F: ?Sized, P: Copy>(
  handlers: &Handlers<F>,
  node: NodeId,
  payload: P,
  invoke: impl Fn(&F, P),
) {
  let Some(to_run) = handlers.borrow().get(&node).cloned()
  else {
    return;
  };

  for handler in to_run {
    invoke(&handler, payload);
  }
}

/// Runs `node`'s handlers, folding their verdicts; every handler on the node runs.
pub(super) fn run_preventable<F: ?Sized, P: Copy>(
  handlers: &Handlers<F>,
  node: NodeId,
  payload: P,
  invoke: impl Fn(&F, P) -> Prevent,
) -> Prevention {
  let mut prevention = Prevention::default();

  let Some(to_run) = handlers.borrow().get(&node).cloned()
  else {
    return prevention;
  };

  for handler in to_run {
    prevention.absorb(invoke(&handler, payload));
  }

  prevention
}

/// Stamps `element` with `node`'s id, so a delegated dispatch can resolve it.
pub(super) fn stamp_node_id(element: &Element, node: NodeId) {
  #[expect(clippy::cast_precision_loss)]
  let value = JsValue::from_f64(node.value() as f64);
  let _ = js_sys::Reflect::set(element, &JsValue::from_str(NODE_ID_PROPERTY), &value);
}

/// The node id stamped on `element`, if it belongs to this platform.
fn node_id_of(element: &Element) -> Option<NodeId> {
  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  js_sys::Reflect::get(element, &JsValue::from_str(NODE_ID_PROPERTY))
    .ok()
    .and_then(|value| value.as_f64())
    .map(|value| NodeId::new(value as u64))
}
