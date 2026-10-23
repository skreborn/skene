//! A button's behavior.

use std::cell::Cell;

use super::{Behavior, ControlHandle};
use crate::{
  ClickEvent,
  EventKind,
  Key,
  KeyEvent,
  NamedKey,
  NativeElement,
  Prevent,
  Realization,
  Role,
};

/// A button: activated by a click, `Enter` or `Space`.
///
/// `Enter` activates as it is pressed, each repeat again; `Space` activates once, as it
/// is released, when the button has held focus since the press. A key activation is
/// dispatched as a click at the control, so the control's click handlers see it. A
/// click is the button's alone and never reaches the ancestors.
#[derive(Debug, Default)]
#[must_use]
pub struct Activate {
  /// Whether a `Space` press the button saw awaits its release.
  armed: Cell<bool>,
}

impl Activate {
  /// The events a button acts on: a click, the keys that activate it, and the blur that
  /// disarms a `Space` press.
  const EVENTS: &'static [EventKind] = &[
    EventKind::Click,
    EventKind::KeyDown,
    EventKind::KeyUp,
    EventKind::Blur,
  ];

  /// A button.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      armed: Cell::new(false),
    }
  }
}

impl<P: Realization> Behavior<P> for Activate {
  #[inline(always)]
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::Button)
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::Button)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    Self::EVENTS
  }

  #[inline(always)]
  fn on_click(&self, _control: &ControlHandle<P>) -> Prevent {
    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, control: &ControlHandle<P>) -> Prevent {
    match &event.key {
      Key::Named(NamedKey::Enter) => {
        let _clicked = control.click(&ClickEvent {
          modifiers: event.modifiers,
          count: 0,
        });

        Prevent::All
      }
      Key::Character(text) if text == " " => {
        if !event.repeat {
          self.armed.set(true);
        }

        Prevent::All
      }
      _ => Prevent::None,
    }
  }

  fn on_key_up(&self, event: &KeyEvent, control: &ControlHandle<P>) -> Prevent {
    if !event.types(" ") || !self.armed.replace(false) {
      return Prevent::None;
    }

    let _clicked = control.click(&ClickEvent {
      modifiers: event.modifiers,
      count: 0,
    });

    Prevent::All
  }

  #[inline(always)]
  fn on_blur(&self, _control: &ControlHandle<P>) {
    self.armed.set(false);
  }
}

#[cfg(test)]
mod tests {
  use super::Activate;
  use crate::EventKind;

  /// A button listens to the click, the keys and the blur it acts on, and nothing else.
  #[test]
  fn a_button_listens_to_what_activates_it() {
    assert_eq!(
      Activate::EVENTS,
      [
        EventKind::Click,
        EventKind::KeyDown,
        EventKind::KeyUp,
        EventKind::Blur,
      ]
    );
  }
}
