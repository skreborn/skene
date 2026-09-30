//! A checkbox's behavior.

use std::{cell::Cell, rc::Rc};

use skene_core::{
  AccessProp,
  ControlState,
  EventKind,
  KeyEvent,
  NativeElement,
  Platform,
  Prevent,
  Role,
};
use skene_reactive::Signal;

use super::{Behavior, ControlHandle, Controlled};

/// A checkbox: a click, `Space` or the platform's own toggle flips it.
///
/// `Space` flips it once, as it is released, when the checkbox has held focus since the
/// press.
///
/// Controlled: the checked state it reports is what
/// [`derive_checked`](Self::derive_checked) reads, and a flip only calls
/// [`handle_toggle`](Self::handle_toggle) with the state it asks for. With no handler
/// nothing changes; [`bind_checked`](Self::bind_checked) wires both to one signal.
#[derive(Default)]
#[must_use]
pub struct Toggle {
  checked: Controlled<bool>,
  armed: Cell<bool>,
}

impl std::fmt::Debug for Toggle {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Toggle").finish_non_exhaustive()
  }
}

impl Toggle {
  /// An unchecked checkbox with no handler.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// The closure the checked state is read through; `false` with none set.
  #[inline(always)]
  #[must_use]
  pub fn reads_checked(&self) -> Rc<dyn Fn() -> bool> {
    self.checked.reader().unwrap_or_else(|| Rc::new(|| false))
  }

  /// Sets the checked state.
  #[inline(always)]
  pub fn set_checked(mut self, checked: bool) -> Self {
    self.checked.set(checked);

    self
  }

  /// Derives the checked state from the signals `checked` reads.
  #[inline(always)]
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.checked.derive(checked);

    self
  }

  /// Runs `action` with the state a flip asks for.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.checked.handle(action);

    self
  }

  /// Binds the checked state to `signal` in both directions.
  #[inline(always)]
  pub fn bind_checked(mut self, signal: Signal<bool>) -> Self {
    self.checked.bind(signal);

    self
  }

  /// The checked state currently reported.
  #[inline(always)]
  fn read(&self) -> bool {
    self.checked.read().unwrap_or(false)
  }

  /// Asks for `checked`.
  #[inline(always)]
  fn ask(&self, checked: bool) {
    self.checked.ask(checked);
  }

  /// Asks for the opposite state.
  #[inline(always)]
  fn flip(&self) {
    self.ask(!self.read());
  }
}

impl<P: Platform> Behavior<P> for Toggle {
  #[inline(always)]
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::Checkbox)
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::CheckBox)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[
      EventKind::Click,
      EventKind::KeyDown,
      EventKind::KeyUp,
      EventKind::Blur,
      EventKind::Toggle,
    ]
  }

  fn on_click(&self, _control: &ControlHandle<P>) -> Prevent {
    self.flip();

    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, _control: &ControlHandle<P>) -> Prevent {
    if !event.types(" ") {
      return Prevent::None;
    }

    if !event.repeat {
      self.armed.set(true);
    }

    Prevent::All
  }

  fn on_key_up(&self, event: &KeyEvent, _control: &ControlHandle<P>) -> Prevent {
    if !event.types(" ") || !self.armed.replace(false) {
      return Prevent::None;
    }

    self.flip();

    Prevent::All
  }

  #[inline(always)]
  fn on_blur(&self, _control: &ControlHandle<P>) {
    self.armed.set(false);
  }

  fn on_toggle(&self, checked: bool, _control: &ControlHandle<P>) {
    self.ask(checked);
  }

  fn access(&self) -> Vec<AccessProp> {
    vec![AccessProp::Checked(self.read().into())]
  }

  fn state(&self) -> ControlState {
    ControlState {
      checked: self.read().into(),
      ..ControlState::DEFAULT
    }
  }
}
