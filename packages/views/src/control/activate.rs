//! A button's behavior.

use skene_core::{
  KeyEvent,
  Prevent,
  Role,
  next::{ControlKind, Driver, EventKind},
};

use super::{Behavior, ControlHandle};

/// A button: activated by a click, `Enter` or `Space`.
///
/// A key activation is dispatched as a click at the control, so the control's click
/// handlers see it. A click is the button's alone and never reaches the ancestors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub struct Activate;

impl Activate {
  /// A button.
  #[inline(always)]
  pub const fn new() -> Self {
    Self
  }
}

impl<D: Driver> Behavior<D> for Activate {
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
    &[EventKind::Click, EventKind::KeyDown]
  }

  #[inline(always)]
  fn on_click(&self, _control: &ControlHandle<D>) -> Prevent {
    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, control: &ControlHandle<D>) -> Prevent {
    match event.key.as_str() {
      "Enter" | " " => {
        let _clicked = control.click();

        Prevent::All
      }
      _ => Prevent::None,
    }
  }
}
