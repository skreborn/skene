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

use std::{rc::Rc, str::FromStr};

use rustc_hash::FxHashMap;
use skene_core::{
  Code,
  EventKind,
  Key,
  KeyEvent,
  Modifiers,
  NamedKey,
  NodeId,
  PointerButton,
  PointerEvent,
  Prevented,
  Scene,
  ScrollEvent,
  TextSelection,
  dispatch_click,
  dispatch_click_observed,
  dispatch_context_menu,
  dispatch_key_down,
  dispatch_key_up,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  notify_change,
  notify_dismiss,
  notify_focus_moved,
  notify_pointer_moved,
  notify_scroll,
  notify_selection_change,
  notify_toggle,
};
use unicode_segmentation::UnicodeSegmentation;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Element, EventTarget, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement};

use super::{Realized, end_turn, with_dom};
use crate::{Web, condition::DRAGS_ATTRIBUTE};

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
  /// Listens for `name` on `target`, running `handler` with each event as one turn.
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

        wasm_bindgen_futures::spawn_local(async move {
          handler(&event);
          end_turn();
        });

        return;
      }

      handler(&event);
      end_turn();
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

/// The root's delegated listeners, the popups currently open, each with the anchor
/// it was opened against, and the text entry focus last left.
pub(super) struct Events {
  /// The element the listeners are delegated to.
  root: Element,
  /// The delegated listeners, by event name.
  listeners: FxHashMap<&'static str, Listener>,
  /// The `<input>` or `<textarea>` focus last left, which a clipboard action from an
  /// open popup acts on in place of the popup's own focus.
  pub(super) last_entry: Option<Element>,
}

impl Events {
  pub(super) fn new(root: Element) -> Self {
    Self {
      root,
      listeners: FxHashMap::default(),
      last_entry: None,
    }
  }

  /// Installs the root listeners serving `kind`, once each.
  pub(super) fn ensure(&mut self, kind: EventKind) {
    let names: &[&'static str] = match kind {
      EventKind::Click => &["click"],
      EventKind::KeyDown => &["keydown"],
      EventKind::KeyUp => &["keyup"],
      EventKind::PointerDown => &["pointerdown"],
      EventKind::PointerUp => &["pointerup"],
      EventKind::PointerMove => &["pointermove"],
      EventKind::PointerEnter | EventKind::PointerLeave => &["pointerover", "pointerout"],
      EventKind::Focus => &["focusin"],
      EventKind::Blur => &["focusout"],
      EventKind::Scroll => &["scroll"],
      EventKind::Change => &["input"],
      EventKind::SelectionChange => &["selectionchange"],
      EventKind::Toggle => &["change"],
      EventKind::Dismiss => &["click", "keydown", "pointerdown"],
      EventKind::ContextMenu => &["contextmenu"],
    };

    for name in names {
      if self.listeners.contains_key(name) {
        continue;
      }

      let handler: fn(&web_sys::Event) = match *name {
        "click" => on_click,
        "keydown" => on_key_down,
        "keyup" => on_key_up,
        "pointerdown" => on_pointer_down,
        "pointerup" => on_pointer_up,
        "pointermove" => on_pointer_move,
        "pointerover" => on_pointer_over,
        "pointerout" => on_pointer_out,
        "focusin" => on_focus_in,
        "focusout" => on_focus_out,
        "scroll" => on_scroll,
        "input" => on_input,
        "selectionchange" => on_selection_change,
        "contextmenu" => on_context_menu,
        _ => on_change,
      };
      let capture = *name == "scroll";
      // The selection change is the document's event: it names no target of its own.
      let target: EventTarget = if *name == "selectionchange" {
        match web_sys::window().and_then(|window| window.document()) {
          Some(document) => document.into(),
          None => continue,
        }
      }
      else {
        self.root.clone().into()
      };

      self
        .listeners
        .insert(name, Listener::new(target, name, capture, handler));
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
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a sink is stamped as a whole, non-negative u32"
  )]
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
        matches!(node.realized, Realized::Control { native: Some(native), .. } if native.is_toggle())
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

  let prevented = node_at(event.target())
    .map(|node| dispatch_key_down::<Web>(node, &key_event(keyboard)))
    .unwrap_or_default();

  apply(prevented, event);

  if keyboard.key() == "Escape"
    && !prevented.platform
    && let Some(popup) = topmost_popup()
  {
    event.prevent_default();
    notify_dismiss::<Web>(popup);
  }
}

fn on_key_up(event: &web_sys::Event) {
  let Some(keyboard) = event.dyn_ref::<web_sys::KeyboardEvent>()
  else {
    return;
  };
  let Some(node) = node_at(event.target())
  else {
    return;
  };

  apply(dispatch_key_up::<Web>(node, &key_event(keyboard)), event);
}

/// The portable key event of a DOM keyboard event.
fn key_event(keyboard: &web_sys::KeyboardEvent) -> KeyEvent {
  KeyEvent {
    key: Key::from_str(&keyboard.key()).unwrap_or(Key::Named(NamedKey::Unidentified)),
    code: Code::from_str(&keyboard.code()).unwrap_or(Code::Unidentified),
    modifiers: Modifiers {
      ctrl: keyboard.ctrl_key(),
      shift: keyboard.shift_key(),
      alt: keyboard.alt_key(),
      meta: keyboard.meta_key(),
    },
    repeat: keyboard.repeat(),
    composing: keyboard.is_composing(),
  }
}

/// The node of the popup opened last among those open.
fn topmost_popup() -> Option<NodeId> {
  Scene::<Web>::with(|scene| {
    scene
      .open_overlays()
      .iter()
      .rev()
      .copied()
      .find(|overlay| !scene.is_modal(*overlay))
  })
}

/// `event` with its local position measured from `element`'s top-left corner.
pub(super) fn localized(event: &PointerEvent, element: &Element) -> PointerEvent {
  let rect = element.get_bounding_client_rect();

  #[expect(
    clippy::cast_possible_truncation,
    reason = "an element's position in CSS pixels is precise enough in f32"
  )]
  PointerEvent {
    local_x: event.x - rect.left() as f32,
    local_y: event.y - rect.top() as f32,
    ..*event
  }
}

/// The portable payload of a DOM pointer event, local to `element`.
fn pointer_payload(pointer: &web_sys::PointerEvent, element: &Element) -> PointerEvent {
  #[expect(
    clippy::cast_precision_loss,
    reason = "a pointer's position in CSS pixels is precise enough in f32"
  )]
  let (x, y) = (pointer.client_x() as f32, pointer.client_y() as f32);
  let event = PointerEvent {
    x,
    y,
    local_x: x,
    local_y: y,
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
  };

  localized(&event, element)
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

/// A press: the popups it lands outside of are dismissed by the scene's rule, a modal's
/// backdrop left to its click, then the press is dispatched.
fn on_pointer_down(event: &web_sys::Event) {
  let at = node_at(event.target());
  let dismissed: Vec<NodeId> = Scene::<Web>::with(|scene| {
    let overlays = scene.open_overlays().to_vec();

    scene
      .dismissed_by_press(&overlays, at)
      .into_iter()
      .filter(|overlay| !scene.is_modal(*overlay))
      .collect()
  });

  for popup in dismissed {
    notify_dismiss::<Web>(popup);
  }

  capture_drag(event);
  dispatch_pointer(event, dispatch_pointer_down::<Web>);
}

/// Keeps a primary press inside a control that follows the pointer with that control,
/// so the press's moves and its release reach the control wherever the pointer goes.
fn capture_drag(event: &web_sys::Event) {
  let Some(pointer) = event.dyn_ref::<web_sys::PointerEvent>()
  else {
    return;
  };

  if pointer.button() != 0 {
    return;
  }

  if let Some(control) = event
    .target()
    .and_then(|target| target.dyn_into::<Element>().ok())
    .and_then(|element| {
      element
        .closest(&format!("[{DRAGS_ATTRIBUTE}]"))
        .ok()
        .flatten()
    })
  {
    let _ = control.set_pointer_capture(pointer.pointer_id());
  }
}

fn on_pointer_up(event: &web_sys::Event) {
  dispatch_pointer(event, dispatch_pointer_up::<Web>);
}

fn on_pointer_move(event: &web_sys::Event) {
  dispatch_pointer(event, dispatch_pointer_move::<Web>);
}

/// The element a pointer event came from or went to.
fn related(event: &web_sys::Event) -> Option<EventTarget> {
  event
    .dyn_ref::<web_sys::MouseEvent>()
    .and_then(web_sys::MouseEvent::related_target)
}

/// The element focus came from or went to.
fn focus_related(event: &web_sys::Event) -> Option<EventTarget> {
  event
    .dyn_ref::<web_sys::FocusEvent>()
    .and_then(web_sys::FocusEvent::related_target)
}

/// The pointer reaching a node: the move from the node it was over is notified whole.
fn on_pointer_over(event: &web_sys::Event) {
  notify_pointer_moved::<Web>(node_at(event.target()));
}

fn on_pointer_out(event: &web_sys::Event) {
  // A move onto another node is notified by the `pointerover` that follows it.
  if node_at(related(event)).is_none() {
    notify_pointer_moved::<Web>(None);
  }
}

/// Focus reaching a node: the move from the node the scene holds focus on is notified
/// whole.
fn on_focus_in(event: &web_sys::Event) {
  notify_focus_moved::<Web>(node_at(event.target()));
}

fn on_focus_out(event: &web_sys::Event) {
  if let Some(entry) = event
    .target()
    .and_then(|target| target.dyn_into::<Element>().ok())
    .filter(|element| {
      element.is_instance_of::<web_sys::HtmlInputElement>()
        || element.is_instance_of::<web_sys::HtmlTextAreaElement>()
    })
  {
    with_dom(|dom| dom.events.last_entry = Some(entry));
  }

  // A move to another node is notified by the `focusin` that follows it.
  if node_at(focus_related(event)).is_none() {
    notify_focus_moved::<Web>(None);
  }
}

/// A context menu request: dispatched where the browser would open its own menu,
/// which stays unless a handler prevents it.
fn on_context_menu(event: &web_sys::Event) {
  let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>()
  else {
    return;
  };
  let Some((element, node)) = resolve(event.target())
  else {
    return;
  };
  #[expect(
    clippy::cast_precision_loss,
    reason = "a pointer's position in CSS pixels is precise enough in f32"
  )]
  let (x, y) = (mouse.client_x() as f32, mouse.client_y() as f32);
  let payload = localized(
    &PointerEvent {
      x,
      y,
      local_x: x,
      local_y: y,
      // A request from the keyboard carries no button.
      button: (mouse.button() == 2).then_some(PointerButton::Secondary),
      modifiers: Modifiers {
        ctrl: mouse.ctrl_key(),
        shift: mouse.shift_key(),
        alt: mouse.alt_key(),
        meta: mouse.meta_key(),
      },
    },
    &element,
  );

  apply(dispatch_context_menu::<Web>(node, &payload), event);
}

fn on_scroll(event: &web_sys::Event) {
  let Some((element, node)) = resolve(event.target())
  else {
    return;
  };

  #[expect(
    clippy::cast_precision_loss,
    reason = "a scroll offset in CSS pixels is precise enough in f32"
  )]
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

/// The selection of the text control holding focus, reported at the node it is; a
/// change of the document's selection anywhere else is nothing to the scene.
fn on_selection_change(_: &web_sys::Event) {
  let Some(element) = web_sys::window()
    .and_then(|window| window.document())
    .and_then(|document| document.active_element())
  else {
    return;
  };
  let Some(selection) = selection_of(&element)
  else {
    return;
  };

  if let Some(node) = node_at(Some(element.into())) {
    notify_selection_change::<Web>(node, selection);
  }
}

/// The selection inside a text control in grapheme clusters of its value; `None` for
/// any other element.
fn selection_of(element: &Element) -> Option<TextSelection> {
  let (value, start, end, backward) = if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
    (
      input.value(),
      input.selection_start().ok()??,
      input.selection_end().ok()??,
      input.selection_direction().ok()?,
    )
  }
  else {
    let area = element.dyn_ref::<HtmlTextAreaElement>()?;

    (
      area.value(),
      area.selection_start().ok()??,
      area.selection_end().ok()??,
      area.selection_direction().ok()?,
    )
  };
  let (start, end) = (cluster_of_unit(&value, start), cluster_of_unit(&value, end));

  Some(if backward.as_deref() == Some("backward") {
    TextSelection::new(end, start)
  }
  else {
    TextSelection::new(start, end)
  })
}

/// Selects `selection`, in grapheme clusters of the control's value, inside a text
/// control; any other element is left alone.
pub(super) fn select(element: &Element, selection: TextSelection) {
  let direction = if selection.is_reversed() {
    "backward"
  }
  else {
    "forward"
  };

  if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
    let value = input.value();

    let _ = input.set_selection_range_with_direction(
      unit_of_cluster(&value, selection.min()),
      unit_of_cluster(&value, selection.max()),
      direction,
    );
  }
  else if let Some(area) = element.dyn_ref::<HtmlTextAreaElement>() {
    let value = area.value();

    let _ = area.set_selection_range_with_direction(
      unit_of_cluster(&value, selection.min()),
      unit_of_cluster(&value, selection.max()),
      direction,
    );
  }
}

/// The cluster of `value` holding the UTF-16 code unit `unit`, or the cluster count
/// at or past the value's end.
fn cluster_of_unit(value: &str, unit: u32) -> usize {
  let mut units = 0;
  let mut index = 0;

  for cluster in value.graphemes(true) {
    units += u32::try_from(cluster.encode_utf16().count()).unwrap_or(u32::MAX);

    if units > unit {
      return index;
    }

    index += 1;
  }

  index
}

/// The UTF-16 code unit in `value` the cluster at `index` starts at, or the value's
/// end in units.
fn unit_of_cluster(value: &str, index: usize) -> u32 {
  let units = value
    .graphemes(true)
    .take(index)
    .map(|cluster| cluster.encode_utf16().count())
    .sum::<usize>();

  u32::try_from(units).unwrap_or(u32::MAX)
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
