//! A button's behavior.

use std::cell::Cell;

use skene_core::{ControlKind, EventKind, Key, KeyEvent, NamedKey, Platform, Prevent, Role};

use super::{Behavior, ControlHandle};

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
  /// A button.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      armed: Cell::new(false),
    }
  }
}

impl<P: Platform> Behavior<P> for Activate {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::Button
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::Button)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[
      EventKind::Click,
      EventKind::KeyDown,
      EventKind::KeyUp,
      EventKind::Blur,
    ]
  }

  #[inline(always)]
  fn on_click(&self, _control: &ControlHandle<P>) -> Prevent {
    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, control: &ControlHandle<P>) -> Prevent {
    match &event.key {
      Key::Named(NamedKey::Enter) => {
        let _clicked = control.click();

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

    let _clicked = control.click();

    Prevent::All
  }

  #[inline(always)]
  fn on_blur(&self, _control: &ControlHandle<P>) {
    self.armed.set(false);
  }
}
