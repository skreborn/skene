//! Abstract input events.
//!
//! These are backend-agnostic. The DOM backend constructs them from DOM events;
//! a native backend would construct them from winit/OS events; a TUI backend
//! from terminal input. The core only ever sees these abstract events.

/// The category of an event a node can subscribe to.
///
/// Used both when registering interest ([`Backend::listen`](crate::Backend::listen))
/// and to tag a delivered [`Event`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EventKind {
  Click,
  PointerDown,
  PointerUp,
  PointerMove,
  KeyDown,
  KeyUp,
  /// Text input / value change on an editable element.
  Input,
}

/// Mouse/pointer buttons.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PointerButton {
  Primary,
  Secondary,
  Middle,
  Other(u16),
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Modifiers {
  pub shift: bool,
  pub ctrl: bool,
  pub alt: bool,
  pub meta: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerEvent {
  /// Position relative to the target node, in logical pixels.
  pub x: f64,
  pub y: f64,
  pub button: PointerButton,
  pub modifiers: Modifiers,
}

#[derive(Clone, PartialEq, Debug)]
pub struct KeyEvent {
  /// The logical key value (e.g. "Enter", "a", "ArrowUp").
  pub key: String,
  /// The physical key code (e.g. "Enter", "KeyA").
  pub code: String,
  pub modifiers: Modifiers,
}

/// A delivered event, with whatever payload the kind carries.
#[derive(Clone, PartialEq, Debug)]
pub enum Event {
  Click(PointerEvent),
  PointerDown(PointerEvent),
  PointerUp(PointerEvent),
  PointerMove(PointerEvent),
  KeyDown(KeyEvent),
  KeyUp(KeyEvent),
  /// Current text value of an editable element.
  Input(String),
}

impl Event {
  pub fn kind(&self) -> EventKind {
    match self {
      Event::Click(_) => EventKind::Click,
      Event::PointerDown(_) => EventKind::PointerDown,
      Event::PointerUp(_) => EventKind::PointerUp,
      Event::PointerMove(_) => EventKind::PointerMove,
      Event::KeyDown(_) => EventKind::KeyDown,
      Event::KeyUp(_) => EventKind::KeyUp,
      Event::Input(_) => EventKind::Input,
    }
  }
}
