//! Root-delegated event delivery routed through core's dispatch.
//!
//! One listener per DOM event type on the mount root finds the nearest element the
//! scene knows, resolves its node and hands the event to core, which bubbles through
//! the scene; the folded verdict decides `preventDefault`. `scroll` is taken in the
//! capture phase, enter and leave are synthesized from `pointerover` and `pointerout`
//! against the scene's ancestry, and a native toggle's `change` reports the platform's
//! flip while its click is dispatched observed: the observers up the path run once per
//! press and the behavior, whose flip the platform performed, is skipped. The only
//! per-node listener is a `<dialog>`'s `cancel`.

use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};
use skene_core::{
  EventKind,
  KeyEvent,
  Modifiers,
  NodeId,
  PointerButton,
  PointerEvent,
  Prevented,
  Scene,
  ScrollEvent,
  dispatch_click,
  dispatch_click_observed,
  dispatch_key_down,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  notify_blur,
  notify_change,
  notify_dismiss,
  notify_focus,
  notify_pointer_enter,
  notify_pointer_leave,
  notify_scroll,
  notify_toggle,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Element, EventTarget, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement};

use super::{Realized, with_dom};
use crate::Web;

/// The property stamped on every element the scene knows, holding its sink.
const SINK_PROPERTY: &str = "__skeneSink";

/// A live DOM listener, removed from its target when dropped.
pub(super) struct Listener {
  target: Option<EventTarget>,
  name: &'static str,
  capture: bool,
  closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
}

impl Listener {
  /// Listens for `name` on `target`, running `handler` with each event.
  ///
  /// An event arriving while the scene is borrowed is delivered from a microtask
  /// instead, once the borrow is released; its verdict then no longer reaches the
  /// platform's default.
  pub(super) fn new(
    target: EventTarget,
    name: &'static str,
    capture: bool,
    handler: impl Fn(&web_sys::Event) + 'static,
  ) -> Self {
    let handler = Rc::new(handler);
    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
      if Scene::<Web>::is_borrowed() {
        let handler = Rc::clone(&handler);

        wasm_bindgen_futures::spawn_local(async move { handler(&event) });

        return;
      }

      handler(&event);
    }) as Box<dyn FnMut(web_sys::Event)>);

    let _ = target.add_event_listener_with_callback_and_bool(
      name,
      closure.as_ref().unchecked_ref(),
      capture,
    );

    Self {
      target: Some(target),
      name,
      capture,
      closure: Some(closure),
    }
  }

  /// A listener on nothing.
  pub(super) const fn none() -> Self {
    Self {
      target: None,
      name: "",
      capture: false,
      closure: None,
    }
  }
}

impl Drop for Listener {
  fn drop(&mut self) {
    if let (Some(target), Some(closure)) = (&self.target, &self.closure) {
      let _ = target.remove_event_listener_with_callback_and_bool(
        self.name,
        closure.as_ref().unchecked_ref(),
        self.capture,
      );
    }
  }
}

/// The root's delegated listeners and the popups currently open, each with the anchor
/// it was opened against.
pub(super) struct Events {
  root: Element,
  listeners: FxHashMap<&'static str, Listener>,
  pub(super) open_popups: FxHashMap<u32, Option<u32>>,
}

impl Events {
  pub(super) fn new(root: Element) -> Self {
    Self {
      root,
      listeners: FxHashMap::default(),
      open_popups: FxHashMap::default(),
    }
  }

  /// Installs the root listeners serving `kind`, once each.
  pub(super) fn ensure(&mut self, kind: EventKind) {
    let names: &[&'static str] = match kind {
      EventKind::Click => &["click"],
      EventKind::KeyDown => &["keydown"],
      EventKind::PointerDown => &["pointerdown"],
      EventKind::PointerUp => &["pointerup"],
      EventKind::PointerMove => &["pointermove"],
      EventKind::PointerEnter | EventKind::PointerLeave => &["pointerover", "pointerout"],
      EventKind::Focus => &["focusin"],
      EventKind::Blur => &["focusout"],
      EventKind::Scroll => &["scroll"],
      EventKind::Change => &["input"],
      EventKind::Toggle => &["change"],
      EventKind::Dismiss => &["click", "keydown", "pointerdown"],
    };

    for name in names {
      if self.listeners.contains_key(name) {
        continue;
      }

      let handler: fn(&web_sys::Event) = match *name {
        "click" => on_click,
        "keydown" => on_key_down,
        "pointerdown" => on_pointer_down,
        "pointerup" => on_pointer_up,
        "pointermove" => on_pointer_move,
        "pointerover" => on_pointer_over,
        "pointerout" => on_pointer_out,
        "focusin" => on_focus_in,
        "focusout" => on_focus_out,
        "scroll" => on_scroll,
        "input" => on_input,
        _ => on_change,
      };
      let capture = *name == "scroll";

      self.listeners.insert(
        name,
        Listener::new(self.root.clone().into(), name, capture, handler),
      );
    }
  }
}

/// Stamps `element` with its sink.
pub(super) fn stamp(element: &Element, sink: u32) {
  let _ = js_sys::Reflect::set(
    element,
    &JsValue::from_str(SINK_PROPERTY),
    &JsValue::from_f64(f64::from(sink)),
  );
}

/// The sink stamped on `element`, if the scene knows it.
pub(super) fn sink_of(element: &Element) -> Option<u32> {
  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  js_sys::Reflect::get(element, &JsValue::from_str(SINK_PROPERTY))
    .ok()
    .and_then(|value| value.as_f64())
    .map(|value| value as u32)
}

/// The nearest element at or above `target` the scene knows, with its node.
fn resolve(target: Option<EventTarget>) -> Option<(Element, NodeId)> {
  let mut current = target.and_then(|target| target.dyn_into::<Element>().ok());

  while let Some(element) = current {
    if let Some(sink) = sink_of(&element) {
      let node = with_dom(|dom| dom.node(sink).and_then(|node| node.id));

      return node.map(|node| (element, node));
    }

    current = element.parent_element();
  }

  None
}

/// The node an event at `target` reaches.
fn node_at(target: Option<EventTarget>) -> Option<NodeId> {
  resolve(target).map(|(_, node)| node)
}

/// Whether `node` sits inside a control the platform toggles itself.
fn inside_native_toggle(node: NodeId) -> bool {
  let control = Scene::<Web>::with(|scene| {
    scene
      .enclosing_control(node)
      .and_then(|control| scene.record(control))
      .map(|record| *record.sink())
  });

  control.is_some_and(|sink| {
    with_dom(|dom| {
      dom.node(sink).is_some_and(|node| {
        matches!(node.realized, Realized::Control { native: Some(native) } if native.is_toggle())
      })
    })
  })
}

/// The node of the modal overlay `element` is the root of, when a click at `x`, `y`
/// lands on its backdrop.
fn backdrop_dismissal(element: &Element, node: NodeId, x: f64, y: f64) -> Option<NodeId> {
  let modal =
    Scene::<Web>::with(|scene| scene.record(node).map(|record| *record.sink())).and_then(|sink| {
      with_dom(|dom| {
        dom
          .node(sink)
          .is_some_and(|node| matches!(node.realized, Realized::Overlay { modal: true, .. }))
      })
      .then_some(())
    });

  modal?;

  let rect = element.get_bounding_client_rect();

  (x < rect.left() || x > rect.right() || y < rect.top() || y > rect.bottom()).then_some(node)
}

/// Applies the folded verdict to the platform.
fn apply(prevented: Prevented, event: &web_sys::Event) {
  if prevented.platform {
    event.prevent_default();
  }
}

fn on_click(event: &web_sys::Event) {
  let Some((element, node)) = resolve(event.target())
  else {
    return;
  };

  if let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>()
    && event.target().as_ref() == Some(element.unchecked_ref())
    && let Some(dismissed) = backdrop_dismissal(
      &element,
      node,
      f64::from(mouse.client_x()),
      f64::from(mouse.client_y()),
    )
  {
    notify_dismiss::<Web>(dismissed);

    return;
  }

  if inside_native_toggle(node) {
    // A press on the label reaches here twice, once at the label and once
    // synthesized at the input; the input's is the one that toggled, and its verdict
    // never reaches the platform, whose flip `change` reports.
    if toggle_input(event).is_some() {
      let _ = dispatch_click_observed::<Web>(node);
    }

    return;
  }

  apply(dispatch_click::<Web>(node), event);
}

fn on_key_down(event: &web_sys::Event) {
  let Some(keyboard) = event.dyn_ref::<web_sys::KeyboardEvent>()
  else {
    return;
  };

  if keyboard.key() == "Escape" {
    for popup in open_popups() {
      notify_dismiss::<Web>(popup);
    }
  }

  let Some(node) = node_at(event.target())
  else {
    return;
  };
  let key_event = KeyEvent {
    key: keyboard.key(),
    modifiers: Modifiers {
      ctrl: keyboard.ctrl_key(),
      shift: keyboard.shift_key(),
      alt: keyboard.alt_key(),
      meta: keyboard.meta_key(),
    },
  };

  apply(dispatch_key_down::<Web>(node, &key_event), event);
}

/// The nodes of the popups currently open.
fn open_popups() -> Vec<NodeId> {
  with_dom(|dom| {
    dom
      .events
      .open_popups
      .keys()
      .filter_map(|sink| dom.node(*sink).and_then(|node| node.id))
      .collect()
  })
}

/// The portable payload of a DOM pointer event, with local coordinates relative to
/// `element`.
fn pointer_payload(pointer: &web_sys::PointerEvent, element: &Element) -> PointerEvent {
  let rect = element.get_bounding_client_rect();

  #[expect(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
  PointerEvent {
    x: pointer.client_x() as f32,
    y: pointer.client_y() as f32,
    local_x: pointer.client_x() as f32 - rect.left() as f32,
    local_y: pointer.client_y() as f32 - rect.top() as f32,
    button: match pointer.button() {
      0 => Some(PointerButton::Primary),
      1 => Some(PointerButton::Auxiliary),
      2 => Some(PointerButton::Secondary),
      _ => None,
    },
    modifiers: Modifiers {
      ctrl: pointer.ctrl_key(),
      shift: pointer.shift_key(),
      alt: pointer.alt_key(),
      meta: pointer.meta_key(),
    },
  }
}

fn dispatch_pointer(event: &web_sys::Event, dispatch: fn(NodeId, &PointerEvent) -> Prevented) {
  let Some(pointer) = event.dyn_ref::<web_sys::PointerEvent>()
  else {
    return;
  };
  let Some((element, node)) = resolve(event.target())
  else {
    return;
  };
  let payload = pointer_payload(pointer, &element);

  apply(dispatch(node, &payload), event);
}

fn on_pointer_down(event: &web_sys::Event) {
  let target = event
    .target()
    .and_then(|target| target.dyn_into::<web_sys::Node>().ok());
  let outside: Vec<NodeId> = with_dom(|dom| {
    let contains = |sink: u32| {
      dom
        .node(sink)
        .is_some_and(|node| node.dom.contains(target.as_ref()))
    };

    dom
      .events
      .open_popups
      .iter()
      .filter(|(popup, anchor)| !contains(**popup) && !anchor.is_some_and(contains))
      .filter_map(|(popup, _)| dom.node(*popup).and_then(|node| node.id))
      .collect()
  });

  for popup in outside {
    notify_dismiss::<Web>(popup);
  }

  dispatch_pointer(event, dispatch_pointer_down::<Web>);
}

fn on_pointer_up(event: &web_sys::Event) {
  dispatch_pointer(event, dispatch_pointer_up::<Web>);
}

fn on_pointer_move(event: &web_sys::Event) {
  dispatch_pointer(event, dispatch_pointer_move::<Web>);
}

/// `node` and its ancestors, innermost first.
fn ancestry(node: Option<NodeId>) -> Vec<NodeId> {
  let mut chain = Vec::new();

  Scene::<Web>::with(|scene| {
    let mut current = node;

    while let Some(node) = current {
      chain.push(node);
      current = scene.parent(node);
    }
  });

  chain
}

/// The nodes on `from`'s ancestry that `to`'s lacks: those a pointer moving from one
/// to the other leaves, innermost first.
fn crossed(from: Option<NodeId>, to: Option<NodeId>) -> Vec<NodeId> {
  let kept: FxHashSet<NodeId> = ancestry(to).into_iter().collect();

  ancestry(from)
    .into_iter()
    .filter(|node| !kept.contains(node))
    .collect()
}

fn related(event: &web_sys::Event) -> Option<EventTarget> {
  event
    .dyn_ref::<web_sys::MouseEvent>()
    .and_then(web_sys::MouseEvent::related_target)
}

fn on_pointer_over(event: &web_sys::Event) {
  let entered = crossed(node_at(event.target()), node_at(related(event)));

  for node in entered.into_iter().rev() {
    notify_pointer_enter::<Web>(node);
  }
}

fn on_pointer_out(event: &web_sys::Event) {
  for node in crossed(node_at(event.target()), node_at(related(event))) {
    notify_pointer_leave::<Web>(node);
  }
}

fn on_focus_in(event: &web_sys::Event) {
  if let Some(node) = node_at(event.target()) {
    notify_focus::<Web>(node);
  }
}

fn on_focus_out(event: &web_sys::Event) {
  if let Some(node) = node_at(event.target()) {
    notify_blur::<Web>(node);
  }
}

fn on_scroll(event: &web_sys::Event) {
  let Some((element, node)) = resolve(event.target())
  else {
    return;
  };

  #[expect(clippy::cast_precision_loss)]
  notify_scroll::<Web>(
    node,
    &ScrollEvent {
      x: element.scroll_left() as f32,
      y: element.scroll_top() as f32,
    },
  );
}

/// The input targeted by `event`, when it is a checkbox or a radio.
fn toggle_input(event: &web_sys::Event) -> Option<HtmlInputElement> {
  event
    .target()
    .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
    .filter(|input| matches!(input.type_().as_str(), "checkbox" | "radio"))
}

fn on_input(event: &web_sys::Event) {
  if toggle_input(event).is_some() {
    return;
  }

  let Some(target) = event.target()
  else {
    return;
  };
  let value = if let Some(input) = target.dyn_ref::<HtmlInputElement>() {
    input.value()
  }
  else if let Some(area) = target.dyn_ref::<HtmlTextAreaElement>() {
    area.value()
  }
  else if let Some(select) = target.dyn_ref::<HtmlSelectElement>() {
    select.value()
  }
  else {
    return;
  };

  if let Some(node) = node_at(Some(target)) {
    notify_change::<Web>(node, &value);
  }
}

fn on_change(event: &web_sys::Event) {
  let Some(input) = toggle_input(event)
  else {
    return;
  };

  if let Some(node) = node_at(Some(input.clone().into())) {
    notify_toggle::<Web>(node, input.checked());
  }
}
