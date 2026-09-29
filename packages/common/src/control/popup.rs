//! A popup face's behavior.

use std::rc::Rc;

use skene_core::{
  AccessProp,
  ControlKind,
  ControlState,
  EventKind,
  KeyEvent,
  NodeId,
  Platform,
  PopupKind,
  Prevent,
  Role,
};
use skene_reactive::{Get, Set, Signal};

use super::{Behavior, ControlHandle};

/// A handler of a key the closed face does not act on, answering whether it took it.
type Typed = dyn Fn(&KeyEvent) -> bool;

/// The face of a popup, which opens and closes it.
///
/// A click, `Enter` or `Space` toggles it, `ArrowDown` and `ArrowUp` open it, `Escape`
/// closes it, and any other key pressed while it is closed goes to
/// [`handle_typed`](Self::handle_typed).
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
  kind: PopupKind,
  controls: Option<Rc<dyn Fn() -> Option<NodeId>>>,
  on_typed: Option<Rc<Typed>>,
}

impl std::fmt::Debug for Popup {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Popup").finish_non_exhaustive()
  }
}

impl Popup {
  /// A closed face with no handler.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets what the popup is; a list of options unless told otherwise.
  #[inline(always)]
  pub const fn set_kind(mut self, kind: PopupKind) -> Self {
    self.kind = kind;

    self
  }

  /// Derives the node the popup shows, reported as the node the control operates once
  /// `controls` yields it.
  #[inline(always)]
  pub fn derive_controls(mut self, controls: impl Fn() -> Option<NodeId> + 'static) -> Self {
    self.controls = Some(Rc::new(controls));

    self
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

  /// Runs `action` with a key the closed face does not act on itself; `true` from it
  /// consumes the key.
  #[inline(always)]
  pub fn handle_typed(mut self, action: impl Fn(&KeyEvent) -> bool + 'static) -> Self {
    self.on_typed = Some(Rc::new(action));

    self
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

impl<P: Platform> Behavior<P> for Popup {
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

  fn on_click(&self, _control: &ControlHandle<P>) -> Prevent {
    self.ask(!self.read());

    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, _control: &ControlHandle<P>) -> Prevent {
    match event.key.as_str() {
      "Enter" | " " => self.ask(!self.read()),
      "ArrowDown" | "ArrowUp" => self.ask(true),
      "Escape" if self.read() => self.ask(false),
      _ if !self.read() && self.on_typed.as_ref().is_some_and(|typed| typed(event)) => {}
      _ => return Prevent::None,
    }

    Prevent::All
  }

  fn access(&self) -> Vec<AccessProp> {
    let mut access = vec![
      AccessProp::HasPopup(self.kind),
      AccessProp::Expanded(self.read()),
    ];

    if let Some(node) = self.controls.as_ref().and_then(|controls| controls()) {
      access.push(AccessProp::Controls(vec![node]));
    }

    access
  }

  fn state(&self) -> ControlState {
    ControlState {
      open: self.read(),
      ..ControlState::DEFAULT
    }
  }
}
