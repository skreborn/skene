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

use std::{cell::Cell, rc::Rc, str::FromStr};

use rustc_hash::FxHashMap;
use skene_foundation::{
  Axes,
  ClickEvent,
  Code,
  Edit,
  EventKind,
  Key,
  KeyDefault,
  KeyEvent,
  Modifiers,
  NamedKey,
  NodeId,
  PointerButton,
  PointerButtons,
  PointerEvent,
  PointerId,
  PointerInfo,
  PointerType,
  Prevented,
  Scene,
  ScrollEvent,
  TextSelection,
  Unit,
  Verdict,
  WheelUnit,
  dispatch_click,
  dispatch_click_observed,
  dispatch_context_menu,
  dispatch_double_click,
  dispatch_key_down,
  dispatch_key_up,
  dispatch_pointer_cancel,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  dispatch_wheel,
  drop_drag,
  end_drag,
  move_drag,
  notify_change,
  notify_dismiss,
  notify_focus_moved,
  notify_pointer_moved,
  notify_scroll,
  notify_selection_change,
  notify_toggle,
  report_edit,
  start_drag,
  text::{Round, TextIndex},
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
  AddEventListenerOptions,
  Element,
  EventTarget,
  HtmlInputElement,
  HtmlSelectElement,
  HtmlTextAreaElement,
  InputEvent,
};

use super::{Node, Realized, end_turn, with_dom};
use crate::{
  condition::{DRAG_MARKER, DRAGGED_ATTRIBUTE, DRAGS_ATTRIBUTE, TARGETED_ATTRIBUTE},
  platform::Web,
};

/// The properties stamped on every element the scene knows, holding its node's index
/// and generation.
const INDEX_PROPERTY: &str = "__skeneIndex";
const GENERATION_PROPERTY: &str = "__skeneGeneration";

thread_local! {
  /// The physical key whose press a context menu served, until its release, which
  /// raises the browser's own menu on some systems.
  static SERVED_MENU_KEY: Cell<Option<Code>> = const { Cell::new(None) };
}

/// A live DOM listener, removed from its target when dropped.
pub(super) struct Listener {
  target: Option<EventTarget>,
  name: &'static str,
  capture: bool,
  closure: Option<Closure<dyn Fn(web_sys::Event)>>,
}

impl Listener {
  /// Listens for `name` on `target`, running `handler` with each event as one turn.
  ///
  /// An event a handler causes as it runs, as a focus it moves fires the next `focusin`,
  /// is delivered at once, inside it. An event arriving while the scene or the leaf's
  /// own state is borrowed, as a focus change made under that borrow dispatches, is
  /// delivered from a microtask instead, once the borrow is released; its verdict then
  /// no longer reaches the platform's default.
  pub(super) fn new(
    target: EventTarget,
    name: &'static str,
    capture: bool,
    handler: impl Fn(&web_sys::Event) + 'static,
  ) -> Self {
    let handler = Rc::new(handler);
    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
      if Scene::<Web>::is_borrowed() || super::is_dom_borrowed() {
        let handler = Rc::clone(&handler);

        wasm_bindgen_futures::spawn_local(async move {
          handler(&event);
          end_turn();
        });

        return;
      }

      handler(&event);
      end_turn();
    }) as Box<dyn Fn(web_sys::Event)>);

    // Not passive, so a wheel's verdict can keep the scroll from happening.
    let options = AddEventListenerOptions::new();

    options.set_capture(capture);
    options.set_passive(false);

    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(
      name,
      closure.as_ref().unchecked_ref(),
      &options,
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
  /// The element of the node a drag in progress started from.
  dragged: Option<Element>,
  /// The element of the node taking the drag in progress.
  targeted: Option<Element>,
}

impl Events {
  pub(super) fn new(root: Element) -> Self {
    Self {
      root,
      listeners: FxHashMap::default(),
      last_entry: None,
      dragged: None,
      targeted: None,
    }
  }

  /// Installs the root listeners serving `kind`, once each.
  pub(super) fn ensure(&mut self, kind: EventKind) {
    let names: &[&'static str] = match kind {
      EventKind::Click => &["click"],
      EventKind::DoubleClick => &["dblclick"],
      EventKind::KeyDown => &["keydown"],
      EventKind::KeyUp => &["keyup"],
      EventKind::PointerDown => &["pointerdown"],
      EventKind::PointerUp => &["pointerup"],
      EventKind::PointerMove => &["pointermove"],
      EventKind::PointerCancel => &["pointercancel"],
      EventKind::PointerEnter | EventKind::PointerLeave => &["pointerover", "pointerout"],
      EventKind::Focus => &["focusin"],
      EventKind::Blur => &["focusout"],
      EventKind::Wheel => &["wheel"],
      EventKind::Scroll => &["scroll"],
      EventKind::Change => &["input"],
      EventKind::SelectionChange => &["selectionchange"],
      EventKind::Edit => &["input", "compositionend"],
      EventKind::Toggle => &["change"],
      EventKind::Dismiss => &["click", "keydown", "pointerdown"],
      EventKind::ContextMenu => &["contextmenu"],
      EventKind::DragStart => &["dragstart", "dragend"],
      EventKind::Drop => &["dragenter", "dragover", "dragleave", "drop"],
    };

    for name in names {
      if self.listeners.contains_key(name) {
        continue;
      }

      let handler: fn(&web_sys::Event) = match *name {
        "click" => on_click,
        "dblclick" => on_double_click,
        "keydown" => on_key_down,
        "keyup" => on_key_up,
        "pointerdown" => on_pointer_down,
        "pointerup" => on_pointer_up,
        "pointermove" => on_pointer_move,
        "pointercancel" => on_pointer_cancel,
        "pointerover" => on_pointer_over,
        "pointerout" => on_pointer_out,
        "focusin" => on_focus_in,
        "focusout" => on_focus_out,
        "wheel" => on_wheel,
        "scroll" => on_scroll,
        "input" => on_input,
        "compositionend" => on_composition_end,
        "selectionchange" => on_selection_change,
        "contextmenu" => on_context_menu,
        "dragstart" => on_drag_start,
        "dragend" => on_drag_end,
        "dragenter" | "dragover" => on_drag_over,
        "dragleave" => on_drag_leave,
        "drop" => on_drop,
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

/// Stamps `element` with its node.
pub(super) fn stamp(element: &Element, node: NodeId) {
  for (property, value) in [
    (INDEX_PROPERTY, node.index()),
    (GENERATION_PROPERTY, node.generation()),
  ] {
    let _ = js_sys::Reflect::set(
      element,
      &JsValue::from_str(property),
      &JsValue::from_f64(f64::from(value)),
    );
  }
}

/// The live node stamped on `element`, if the platform holds its record.
pub(super) fn node_of(element: &Element) -> Option<NodeId> {
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "an index and a generation are stamped as whole, non-negative u32s"
  )]
  let read = |property: &str| {
    js_sys::Reflect::get(element, &JsValue::from_str(property))
      .ok()
      .and_then(|value| value.as_f64())
      .map(|value| value as u32)
  };
  let (index, generation) = (read(INDEX_PROPERTY)?, read(GENERATION_PROPERTY)?);

  with_dom(|dom| dom.realizer.records.node_at(index)).filter(|node| node.generation() == generation)
}

/// The nearest element at or above `target` the scene knows, with its node.
fn resolve(target: Option<EventTarget>) -> Option<(Element, NodeId)> {
  let mut current = target.and_then(|target| target.dyn_into::<Element>().ok());

  while let Some(element) = current {
    if let Some(node) = node_of(&element) {
      return Some((element, node));
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
  let control = Scene::<Web>::with(|scene| scene.enclosing_control(node));

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
  let modal = Some(node).and_then(|sink| {
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
      let _ = dispatch_click_observed::<Web>(node, &click_event(event));
    }

    return;
  }

  apply(dispatch_click::<Web>(node, &click_event(event)), event);
}

/// The second click of a double click, dispatched after the click itself.
fn on_double_click(event: &web_sys::Event) {
  let Some((_, node)) = resolve(event.target())
  else {
    return;
  };

  apply(
    dispatch_double_click::<Web>(node, &click_event(event)),
    event,
  );
}

/// The portable click of a DOM click: its modifiers and its place in a run of quick
/// clicks, none for a click no pointer made.
fn click_event(event: &web_sys::Event) -> ClickEvent {
  event
    .dyn_ref::<web_sys::MouseEvent>()
    .map_or(ClickEvent::new(0), |mouse| ClickEvent {
      modifiers: mouse_modifiers(mouse),
      count: u8::try_from(mouse.detail()).unwrap_or(u8::MAX),
    })
}

/// The modifier state a DOM mouse event reports.
fn mouse_modifiers(mouse: &web_sys::MouseEvent) -> Modifiers {
  Modifiers {
    ctrl: mouse.ctrl_key(),
    shift: mouse.shift_key(),
    alt: mouse.alt_key(),
    meta: mouse.meta_key(),
  }
}

fn on_key_down(event: &web_sys::Event) {
  let Some(keyboard) = event.dyn_ref::<web_sys::KeyboardEvent>()
  else {
    return;
  };

  let key = key_event(keyboard);
  let prevented = node_at(event.target())
    .map(|node| dispatch_key_down::<Web>(node, &key))
    .unwrap_or_default();

  apply(prevented, event);

  if prevented.platform {
    return;
  }

  match Scene::<Web>::read(|scene| scene.key_default(&key)) {
    // A modal the scene dismisses on `Escape` is the dialog's own to cancel.
    Some(KeyDefault::Dismiss(overlay)) if !Scene::<Web>::read(|scene| scene.is_modal(overlay)) => {
      event.prevent_default();
      notify_dismiss::<Web>(overlay);
    }
    // A menu the scene serves takes the browser's place; where none does, the browser
    // raises its own.
    Some(KeyDefault::ContextMenu) => {
      let Some((element, node)) = resolve(event.target())
      else {
        return;
      };
      let served = dispatch_context_menu::<Web>(node, &keyboard_request(&element, key.modifiers));

      if served.platform {
        event.prevent_default();
        SERVED_MENU_KEY.set(Some(key.code));
      }
    }
    _ => {}
  }
}

/// A request for the context menu from the keyboard where `element` holds focus: no
/// pointer's, placed at the element's bottom left.
fn keyboard_request(element: &Element, modifiers: Modifiers) -> PointerEvent {
  let rect = element.get_bounding_client_rect();
  #[expect(
    clippy::cast_possible_truncation,
    reason = "a position in CSS pixels is precise enough in f32"
  )]
  let (x, y) = (rect.left() as f32, rect.bottom() as f32);
  let event = PointerEvent {
    modifiers,
    pointer: PointerInfo {
      pointer_id: None,
      persistent_device_id: None,
      pointer_type: PointerType::Unknown,
    },
    ..PointerEvent::mouse(x, y)
  };

  localized(&event, element)
}

fn on_key_up(event: &web_sys::Event) {
  let Some(keyboard) = event.dyn_ref::<web_sys::KeyboardEvent>()
  else {
    return;
  };
  let key = key_event(keyboard);

  if SERVED_MENU_KEY.get() == Some(key.code) {
    SERVED_MENU_KEY.set(None);
    event.prevent_default();
  }

  let Some(node) = node_at(event.target())
  else {
    return;
  };

  apply(dispatch_key_up::<Web>(node, &key), event);
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

/// The buttons by their number in `MouseEvent.button`.
const BUTTON_NUMBERS: [PointerButton; 6] = [
  PointerButton::Primary,
  PointerButton::Auxiliary,
  PointerButton::Secondary,
  PointerButton::X1,
  PointerButton::X2,
  PointerButton::PenEraser,
];

/// The buttons by their bit in `MouseEvent.buttons`.
const BUTTON_BITS: [PointerButton; 6] = [
  PointerButton::Primary,
  PointerButton::Secondary,
  PointerButton::Auxiliary,
  PointerButton::X1,
  PointerButton::X2,
  PointerButton::PenEraser,
];

/// The portable payload of a DOM mouse or pointer event, local to `element`.
///
/// A pointer event names its pointer, the primary one as [`PointerId::PRIMARY`] and
/// every other past it, and one no pointing device raised names none; a mouse event is
/// the primary mouse's.
fn pointer_payload(mouse: &web_sys::MouseEvent, element: &Element) -> PointerEvent {
  #[expect(
    clippy::cast_precision_loss,
    reason = "a pointer's position in CSS pixels is precise enough in f32"
  )]
  let (x, y) = (mouse.client_x() as f32, mouse.client_y() as f32);
  let mut buttons = PointerButtons::new();

  for (bit, button) in BUTTON_BITS.into_iter().enumerate() {
    if mouse.buttons() & (1 << bit) != 0 {
      buttons.insert(button);
    }
  }

  let mut event = PointerEvent {
    button: usize::try_from(mouse.button())
      .ok()
      .and_then(|number| BUTTON_NUMBERS.get(number).copied()),
    buttons,
    modifiers: mouse_modifiers(mouse),
    pressure: if buttons.is_empty() { 0. } else { 0.5 },
    ..PointerEvent::mouse(x, y)
  };

  if let Some(pointer) = mouse.dyn_ref::<web_sys::PointerEvent>() {
    event.pointer = PointerInfo {
      pointer_id: if pointer.is_primary() {
        Some(PointerId::PRIMARY)
      }
      else {
        u64::try_from(pointer.pointer_id())
          .ok()
          .and_then(|id| PointerId::new(id + 2))
      },
      persistent_device_id: None,
      pointer_type: match pointer.pointer_type().as_str() {
        "mouse" => PointerType::Mouse,
        "pen" => PointerType::Pen,
        "touch" => PointerType::Touch,
        _ => PointerType::Unknown,
      },
    };
    event.pressure = pointer.pressure();
  }

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

fn on_pointer_cancel(event: &web_sys::Event) {
  dispatch_pointer(event, dispatch_pointer_cancel::<Web>);
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
  let payload = PointerEvent {
    // A request from the keyboard carries no button.
    button: (mouse.button() == 2).then_some(PointerButton::Secondary),
    ..pointer_payload(mouse, &element)
  };

  apply(dispatch_context_menu::<Web>(node, &payload), event);
}

/// A wheel's turn: dispatched at the node under the pointer before the browser scrolls
/// by it, which a handler's verdict prevents.
fn on_wheel(event: &web_sys::Event) {
  let Some(wheel) = event.dyn_ref::<web_sys::WheelEvent>()
  else {
    return;
  };
  let Some((element, node)) = resolve(event.target())
  else {
    return;
  };
  #[expect(
    clippy::cast_possible_truncation,
    reason = "a wheel's delta is precise enough in f32"
  )]
  let delta = Axes::new(wheel.delta_x() as f32, wheel.delta_y() as f32);
  let unit = match wheel.delta_mode() {
    web_sys::WheelEvent::DOM_DELTA_LINE => WheelUnit::Lines,
    web_sys::WheelEvent::DOM_DELTA_PAGE => WheelUnit::Pages,
    _ => WheelUnit::Lengths,
  };
  let payload = pointer_payload(wheel, &element).wheel(delta, unit);

  apply(dispatch_wheel::<Web>(node, &payload), event);
}

/// A drag starting from an element: the nearest node offering drag data builds what it
/// carries, and the browser is handed its text, or a marker where it has none.
fn on_drag_start(event: &web_sys::Event) {
  let Some(drag) = event.dyn_ref::<web_sys::DragEvent>()
  else {
    return;
  };
  let Some((source, data)) = node_at(event.target()).and_then(start_drag::<Web>)
  else {
    return;
  };

  if let Some(transfer) = drag.data_transfer() {
    let _ = match data.text() {
      Some(text) => transfer.set_data("text/plain", text),
      None => transfer.set_data(DRAG_MARKER, ""),
    };
  }

  mark_drag(
    |events| &mut events.dragged,
    Some(source),
    DRAGGED_ATTRIBUTE,
  );
}

/// A drag over an element: the nearest node taking what it carries becomes its target,
/// and the browser lets it drop there.
fn on_drag_over(event: &web_sys::Event) {
  let target = move_drag::<Web>(node_at(event.target()));

  mark_drag(|events| &mut events.targeted, target, TARGETED_ATTRIBUTE);

  if target.is_some() {
    event.prevent_default();
  }
}

/// A drag leaving the page, which leaves it no target.
fn on_drag_leave(event: &web_sys::Event) {
  if related(event).is_none() {
    let _ = move_drag::<Web>(None);

    mark_drag(|events| &mut events.targeted, None, TARGETED_ATTRIBUTE);
  }
}

/// A drop: the target takes the drag, and the browser's own response is prevented.
fn on_drop(event: &web_sys::Event) {
  event.prevent_default();

  let _ = drop_drag::<Web>();

  clear_drag_marks();
}

/// A drag ending, dropped or not.
fn on_drag_end(_: &web_sys::Event) {
  end_drag::<Web>();
  clear_drag_marks();
}

/// Moves `attribute` from the element `slot` holds to the element of `node`.
fn mark_drag(
  slot: impl FnOnce(&mut Events) -> &mut Option<Element>,
  node: Option<NodeId>,
  attribute: &str,
) {
  let sink = node;

  with_dom(|dom| {
    let element = sink.and_then(|sink| dom.node(sink).and_then(Node::dom_element).cloned());
    let held = slot(&mut dom.events);

    if *held == element {
      return;
    }

    if let Some(previous) = held.take() {
      let _ = previous.remove_attribute(attribute);
    }

    if let Some(element) = &element {
      let _ = element.set_attribute(attribute, "");
    }

    *held = element;
  });
}

/// Takes the drag marks off their elements.
fn clear_drag_marks() {
  mark_drag(|events| &mut events.dragged, None, DRAGGED_ATTRIBUTE);
  mark_drag(|events| &mut events.targeted, None, TARGETED_ATTRIBUTE);
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

/// A text entry's edit reported at its node as one its toolkit applied, and repaired to
/// the truth unless an input method is composing; any other control's new value reported
/// as a change.
fn on_input(event: &web_sys::Event) {
  if toggle_input(event).is_some() {
    return;
  }

  let Some(target) = event.target()
  else {
    return;
  };

  if let Some(entry) = Entry::of(&target) {
    let composing = event
      .dyn_ref::<InputEvent>()
      .is_some_and(InputEvent::is_composing);

    with_dom(|dom| {
      dom.realizer.sink.composing = composing.then(|| entry.node());
    });

    if let Some(node) = node_at(Some(target)) {
      entry.report(node, composing);
    }

    return;
  }

  let value = if let Some(input) = target.dyn_ref::<HtmlInputElement>() {
    input.value()
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

/// The text a composition leaves in its entry, reported at its node as an edit its
/// toolkit applied and repaired to the truth.
fn on_composition_end(event: &web_sys::Event) {
  let Some(target) = event.target()
  else {
    return;
  };
  let Some(entry) = Entry::of(&target)
  else {
    return;
  };

  with_dom(|dom| dom.realizer.sink.composing = None);

  if let Some(node) = node_at(Some(target)) {
    entry.report(node, false);
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
  let Some(selection) = Entry::of(&element).and_then(|entry| entry.selection())
  else {
    return;
  };

  if let Some(node) = node_at(Some(element.into())) {
    notify_selection_change::<Web>(node, selection);
  }
}

/// Selects `selection`, in grapheme clusters of the control's value, inside a text
/// control; any other element is left alone.
pub(super) fn select(element: &Element, selection: TextSelection) {
  if let Some(entry) = Entry::of(element) {
    entry.select(selection);
  }
}

/// A text entry's own element: a text `<input>` or a `<textarea>`.
pub(super) enum Entry {
  /// A single-line field's `<input>`.
  Field(HtmlInputElement),
  /// An editor's `<textarea>`.
  Editor(HtmlTextAreaElement),
}

impl Entry {
  /// `element` as a text entry; `None` for any other element.
  pub(super) fn of(element: &JsValue) -> Option<Self> {
    if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
      return (input.type_() == "text").then(|| Self::Field(input.clone()));
    }

    element
      .dyn_ref::<HtmlTextAreaElement>()
      .cloned()
      .map(Self::Editor)
  }

  /// The element, as a node.
  fn node(&self) -> web_sys::Node {
    match self {
      Self::Field(input) => input.clone().into(),
      Self::Editor(area) => area.clone().into(),
    }
  }

  fn value(&self) -> String {
    match self {
      Self::Field(input) => input.value(),
      Self::Editor(area) => area.value(),
    }
  }

  /// The selection's start and end in UTF-16 code units of the value, and whether it was
  /// made backward.
  fn range(&self) -> Option<(u32, u32, bool)> {
    let (start, end, direction) = match self {
      Self::Field(input) => (
        input.selection_start().ok()??,
        input.selection_end().ok()??,
        input.selection_direction().ok()?,
      ),
      Self::Editor(area) => (
        area.selection_start().ok()??,
        area.selection_end().ok()??,
        area.selection_direction().ok()?,
      ),
    };

    Some((start, end, direction.as_deref() == Some("backward")))
  }

  /// Selects from `start` to `end`, in UTF-16 code units of the value, made backward
  /// where `backward` says.
  fn select_range(&self, start: u32, end: u32, backward: bool) {
    let direction = if backward { "backward" } else { "forward" };

    let _ = match self {
      Self::Field(input) => input.set_selection_range_with_direction(start, end, direction),
      Self::Editor(area) => area.set_selection_range_with_direction(start, end, direction),
    };
  }

  /// The selection in grapheme clusters of the value.
  fn selection(&self) -> Option<TextSelection> {
    let (start, end, backward) = self.range()?;
    let index = TextIndex::new(&self.value());
    let cluster = |unit: u32| index.grapheme_of(Unit::Utf16, unit as usize, Round::Down);
    let (start, end) = (cluster(start), cluster(end));

    Some(if backward {
      TextSelection::new(end, start)
    }
    else {
      TextSelection::new(start, end)
    })
  }

  /// Selects `selection`, in grapheme clusters of the value.
  fn select(&self, selection: TextSelection) {
    let index = TextIndex::new(&self.value());
    let unit = |cluster: usize| Self::narrow(index.offset_of_grapheme(Unit::Utf16, cluster));

    self.select_range(
      unit(selection.min()),
      unit(selection.max()),
      selection.is_reversed(),
    );
  }

  /// Reports what the entry shows at `node`, its value and its selection, as an edit its
  /// toolkit applied, and repairs it to the truth the verdict answers with unless
  /// `composing`, where the repair waits for the composition's end.
  pub(super) fn report(&self, node: NodeId, composing: bool) {
    let shown = Edit::new(self.value());
    let shown = match self.selection() {
      Some(selection) => shown.with_selection(selection),
      None => shown,
    };

    if let Verdict::Replace(truth) = report_edit::<Web>(node, &shown, true)
      && !composing
    {
      self.repair(&truth);
    }
  }

  /// Repairs the entry to `truth` by the smallest edit: the range of its value that
  /// differs is replaced in place, and the selection is the truth's where it gives one,
  /// else the entry's own mapped through the edit ([`TextIndex::map_through`]).
  pub(super) fn repair(&self, truth: &Edit) {
    let shown = self.value();
    let (old, new) = (TextIndex::new(&shown), TextIndex::new(&truth.text));
    let (replaced, inserted) = truth.splice_bytes(&shown);
    let units = old.unit_range(Unit::Utf16, replaced.clone());
    let range = self.range();

    if !replaced.is_empty() || !inserted.is_empty() {
      let (start, end) = (Self::narrow(units.start), Self::narrow(units.end));

      let _ = match self {
        Self::Field(input) => input.set_range_text_with_start_and_end(inserted, start, end),
        Self::Editor(area) => area.set_range_text_with_start_and_end(inserted, start, end),
      };
    }

    if let Some(selection) = truth.selection {
      self.select(selection);
    }
    else if let Some((start, end, backward)) = range {
      let map = |unit: u32| {
        let byte = old.byte(Unit::Utf16, unit as usize, Round::Down);
        let mapped = old.map_through(&new, replaced.clone(), inserted.len(), byte);

        Self::narrow(new.offset(Unit::Utf16, mapped))
      };

      self.select_range(map(start), map(end), backward);
    }
  }

  /// A count of UTF-16 code units as the DOM takes it.
  fn narrow(units: usize) -> u32 {
    u32::try_from(units).unwrap_or(u32::MAX)
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
