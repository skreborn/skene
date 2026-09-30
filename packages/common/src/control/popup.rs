//! A popup face's behavior.

use std::rc::Rc;

use skene_core::{
  AccessProp,
  ControlState,
  EventKind,
  Key,
  KeyEvent,
  NamedKey,
  NativeElement,
  NodeId,
  Platform,
  PopupKind,
  Prevent,
  Role,
};
use skene_reactive::Signal;

use super::{Behavior, ControlHandle, Controlled, LabelPlacement};

/// A handler of a key the closed face does not act on, answering whether it took it.
type Typed = dyn Fn(&KeyEvent) -> bool;

/// The face of a popup, which opens and closes it.
///
/// A face opening a list of options is a combobox, and one opening a menu or a dialog a
/// button. A click, `Enter` or `Space` toggles it, `ArrowDown` and `ArrowUp` open it,
/// `Escape` closes it, and any other key pressed while it is closed goes to
/// [`set_key_fallback`](Self::set_key_fallback), which `Space` reaches first, toggling only
/// where the handler does not take it.
///
/// Controlled: the open state it reports is what [`derive_open`](Self::derive_open)
/// reads, and a toggle only calls [`handle_open`](Self::handle_open) with the state it
/// asks for; [`bind_open`](Self::bind_open) wires both to one signal. Reports
/// [`AccessProp::Expanded`], and the control-scoped `Open` state inside it follows.
#[derive(Default)]
#[must_use]
pub struct Popup {
  open: Controlled<bool>,
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
    self.open.set(open);

    self
  }

  /// Derives whether the popup is open from the signals `open` reads.
  #[inline(always)]
  pub fn derive_open(mut self, open: impl Fn() -> bool + 'static) -> Self {
    self.open.derive(open);

    self
  }

  /// Runs `action` with the open state the face asks for.
  #[inline(always)]
  pub fn handle_open(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.open.handle(action);

    self
  }

  /// Binds the open state to `signal` in both directions.
  #[inline(always)]
  pub fn bind_open(mut self, signal: Signal<bool>) -> Self {
    self.open.bind(signal);

    self
  }

  /// Runs `action` with a key the closed face does not act on itself; `true` from it
  /// consumes the key.
  #[inline(always)]
  pub fn set_key_fallback(mut self, action: impl Fn(&KeyEvent) -> bool + 'static) -> Self {
    self.on_typed = Some(Rc::new(action));

    self
  }

  /// The open state currently reported.
  #[inline(always)]
  fn read(&self) -> bool {
    self.open.read().unwrap_or(false)
  }

  /// Asks for `open`.
  #[inline(always)]
  fn ask(&self, open: bool) {
    self.open.ask(open);
  }
}

impl<P: Platform> Behavior<P> for Popup {
  #[inline(always)]
  fn native(&self) -> Option<NativeElement> {
    match self.kind {
      PopupKind::Listbox => None,
      PopupKind::Menu | PopupKind::Dialog => Some(NativeElement::Button),
    }
  }

  #[inline(always)]
  fn label_placement(&self) -> LabelPlacement {
    match self.kind {
      PopupKind::Listbox => LabelPlacement::Beside,
      PopupKind::Menu | PopupKind::Dialog => LabelPlacement::Trailing,
    }
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(match self.kind {
      PopupKind::Listbox => Role::ComboBox,
      PopupKind::Menu | PopupKind::Dialog => Role::Button,
    })
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
    let typed = || !self.read() && self.on_typed.as_ref().is_some_and(|typed| typed(event));

    match &event.key {
      Key::Character(text) if text == " " && typed() => {}
      Key::Named(NamedKey::Enter) => self.ask(!self.read()),
      Key::Character(text) if text == " " => self.ask(!self.read()),
      Key::Named(NamedKey::ArrowDown | NamedKey::ArrowUp) => self.ask(true),
      Key::Named(NamedKey::Escape) if self.read() => self.ask(false),
      _ if typed() => {}
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
