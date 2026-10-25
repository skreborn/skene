//! The behavior of a picker the platform draws and lists itself.

use std::rc::Rc;

use super::{Behavior, ControlHandle};
use crate::{Choices, Controller, EventKind, NativeElement, Role, contents};

/// A picker the platform draws and lists itself: the scriptless form of a
/// [`BasicPicker`](crate::BasicPicker), realized as the platform's own choice control
/// where it has one.
///
/// Controlled: the options and the chosen key it shows are what
/// [`derive_choices`](Self::derive_choices) reads, written to the platform as one
/// [`Choices`] content a [`Controller`] holds, and the platform's choice only calls
/// [`handle_choose`](Self::handle_choose) with the key it asks for, a choice the truth
/// then refuses being undone. A picker deriving no choices controls nothing.
#[must_use]
pub struct NativePicker {
  choices: Option<Rc<dyn Fn() -> Choices>>,
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
  /// A picker with no choices and no handler.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      choices: None,
      on_change: None,
    }
  }

  /// Derives the options and the chosen key from the signals `choices` reads.
  pub fn derive_choices(mut self, choices: impl Fn() -> Choices + 'static) -> Self {
    self.choices = Some(Rc::new(choices));

    self
  }

  /// Runs `action` with the key the platform's choice asks for.
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

impl Behavior for NativePicker {
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
    &[]
  }

  fn attach(&self, control: &ControlHandle) {
    let Some(choices) = self.choices.clone()
    else {
      return;
    };
    let on_change = self.on_change.clone();

    control.register(Controller::new(
      contents::CHOICE,
      move || choices(),
      move |proposed: &Choices| {
        if let (Some(action), Some(chosen)) = (&on_change, &proposed.chosen) {
          action(chosen.to_string());
        }
      },
    ));
  }
}
