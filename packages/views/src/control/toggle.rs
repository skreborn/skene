//! A checkbox's behavior.

use std::rc::Rc;

use skene_core::{
  AccessProp,
  ControlKind,
  ControlState,
  Driver,
  EventKind,
  KeyEvent,
  Prevent,
  Role,
};
use skene_reactive::{Get, Set, Signal};

use super::{Behavior, ControlHandle};

/// A checkbox: a click, `Space` or the platform's own toggle flips it.
///
/// Controlled: the checked state it reports is what
/// [`derive_checked`](Self::derive_checked) reads, and a flip only calls
/// [`handle_toggle`](Self::handle_toggle) with the state it asks for. With no handler
/// nothing changes; [`bind_checked`](Self::bind_checked) wires both to one signal.
#[derive(Default)]
#[must_use]
pub struct Toggle {
  checked: Option<Rc<dyn Fn() -> bool>>,
  on_toggle: Option<Rc<dyn Fn(bool)>>,
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
    self.checked.clone().unwrap_or_else(|| Rc::new(|| false))
  }

  /// Sets the checked state.
  #[inline(always)]
  pub fn set_checked(mut self, checked: bool) -> Self {
    self.checked = Some(Rc::new(move || checked));

    self
  }

  /// Derives the checked state from the signals `checked` reads.
  #[inline(always)]
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.checked = Some(Rc::new(checked));

    self
  }

  /// Runs `action` with the state a flip asks for.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.on_toggle = Some(Rc::new(action));

    self
  }

  /// Binds the checked state to `signal` in both directions.
  #[inline(always)]
  pub fn bind_checked(self, signal: Signal<bool>) -> Self {
    self
      .derive_checked(move || signal.get())
      .handle_toggle(move |checked| signal.set(checked))
  }

  /// The checked state currently reported.
  #[inline(always)]
  fn read(&self) -> bool {
    self.checked.as_ref().is_some_and(|checked| checked())
  }

  /// Asks for `checked`.
  fn ask(&self, checked: bool) {
    if let Some(action) = &self.on_toggle {
      action(checked);
    }
  }

  /// Asks for the opposite state.
  #[inline(always)]
  fn flip(&self) {
    self.ask(!self.read());
  }
}

impl<D: Driver> Behavior<D> for Toggle {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::Checkbox
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::Checkbox)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::Click, EventKind::KeyDown, EventKind::Toggle]
  }

  fn on_click(&self, _control: &ControlHandle<D>) -> Prevent {
    self.flip();

    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, _control: &ControlHandle<D>) -> Prevent {
    if event.key == " " {
      self.flip();

      return Prevent::All;
    }

    Prevent::None
  }

  fn on_toggle(&self, checked: bool, _control: &ControlHandle<D>) {
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
