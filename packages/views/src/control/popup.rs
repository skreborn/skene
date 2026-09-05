//! A popup face's behavior.

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

/// The face of a popup: a click, `Enter` or `Space` toggles it, `ArrowDown` and
/// `ArrowUp` open it, and `Escape` closes it.
///
/// Controlled: the open state it reports is what [`derive_open`](Self::derive_open)
/// reads, and a toggle only calls [`handle_open`](Self::handle_open) with the state it
/// asks for; [`bind_open`](Self::bind_open) wires both to one signal. Reports
/// [`AccessProp::Expanded`], and the control-scoped `Open` state inside it follows.
#[derive(Default)]
#[must_use]
pub struct Popup {
  open: Option<Rc<dyn Fn() -> bool>>,
  on_open: Option<Rc<dyn Fn(bool)>>,
}

impl Popup {
  /// A closed face with no handler.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets whether the popup is open.
  #[inline(always)]
  pub fn set_open(mut self, open: bool) -> Self {
    self.open = Some(Rc::new(move || open));

    self
  }

  /// Derives whether the popup is open from the signals `open` reads.
  #[inline(always)]
  pub fn derive_open(mut self, open: impl Fn() -> bool + 'static) -> Self {
    self.open = Some(Rc::new(open));

    self
  }

  /// Runs `action` with the open state the face asks for.
  #[inline(always)]
  pub fn handle_open(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.on_open = Some(Rc::new(action));

    self
  }

  /// Binds the open state to `signal` in both directions.
  #[inline(always)]
  pub fn bind_open(self, signal: Signal<bool>) -> Self {
    self
      .derive_open(move || signal.get())
      .handle_open(move |open| signal.set(open))
  }

  /// The open state currently reported.
  #[inline(always)]
  fn read(&self) -> bool {
    self.open.as_ref().is_some_and(|open| open())
  }

  /// Asks for `open`.
  fn ask(&self, open: bool) {
    if let Some(action) = &self.on_open {
      action(open);
    }
  }
}

impl<D: Driver> Behavior<D> for Popup {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::Combobox
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::Combobox)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::Click, EventKind::KeyDown]
  }

  fn on_click(&self, _control: &ControlHandle<D>) -> Prevent {
    self.ask(!self.read());

    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, _control: &ControlHandle<D>) -> Prevent {
    match event.key.as_str() {
      "Enter" | " " => self.ask(!self.read()),
      "ArrowDown" | "ArrowUp" => self.ask(true),
      "Escape" if self.read() => self.ask(false),
      _ => return Prevent::None,
    }

    Prevent::All
  }

  fn access(&self) -> Vec<AccessProp> {
    vec![AccessProp::Expanded(self.read())]
  }

  fn state(&self) -> ControlState {
    ControlState {
      open: self.read(),
      ..ControlState::DEFAULT
    }
  }
}
