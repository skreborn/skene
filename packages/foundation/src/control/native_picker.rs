//! The behavior of a picker the platform draws and lists itself.

use std::rc::Rc;

use super::{Behavior, ControlHandle};
use crate::{EventKind, NativeElement, Realization, Role};

/// A picker the platform draws and lists itself: the scriptless form of a
/// [`BasicPicker`](crate::BasicPicker), realized as the platform's own choice control
/// where it has one.
///
/// The options are written to the platform by whoever builds the control, from the
/// [`Roster`](super::Roster) the option controls join; the platform's choice arrives
/// as a change and only calls [`handle_choose`](Self::handle_choose), so the bound
/// value stays the truth.
#[must_use]
pub struct NativePicker {
  on_change: Option<Rc<dyn Fn(String)>>,
}

impl std::fmt::Debug for NativePicker {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("NativePicker")
      .finish_non_exhaustive()
  }
}

impl NativePicker {
  /// A picker with no handler.
  #[inline(always)]
  pub const fn new() -> Self {
    Self { on_change: None }
  }

  /// Runs `action` with the value the platform's choice asks for.
  pub fn handle_choose(mut self, action: impl Fn(String) + 'static) -> Self {
    self.on_change = Some(Rc::new(action));

    self
  }
}

impl Default for NativePicker {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Behavior<P> for NativePicker {
  #[inline(always)]
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::Picker)
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    None
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::Change]
  }

  fn on_change(&self, value: &str, _control: &ControlHandle<P>) {
    if let Some(action) = &self.on_change {
      action(value.to_owned());
    }
  }
}
