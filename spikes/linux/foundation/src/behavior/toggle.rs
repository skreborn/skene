//! An on/off control: a switch, a checkbox; and the key table a radio shares with them.

use std::rc::Rc;

use crate::{
  Signal,
  behavior::{Behavior, ControlHandle, Controlled, Response, plain},
  seam::{
    access::{AccessFacts, Checked, Role},
    control::{AccessRequest, ControlState, LabelPlacement, Prevent},
    report::KeyEvent,
  },
};

/// Which on/off control.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ToggleKind {
  /// A switch: flips.
  Switch,
  /// A checkbox: flips, a mixed one turning on.
  Checkbox,
  /// A radio: turns on, never off (choosing another member turns it off).
  Radio,
}

/// An on/off control's key table: `Space` shows it pressed and flips it on its release;
/// `Enter` is left to the ancestors (a form's default action), as a browser's checkbox leaves
/// it. Its act is the state the reader flipped it to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub struct ToggleKeys {
  /// Which control.
  pub kind: ToggleKind,
  /// Whether it is on.
  pub on: bool,
  /// Whether it is mixed (a checkbox's third state).
  pub mixed: bool,
  /// Whether `Space` holds it pressed now.
  pub pressed: bool,
}

impl ToggleKeys {
  /// A control of `kind`, on where `on`, mixed where `mixed`, held pressed by `Space` where
  /// `pressed`.
  #[inline(always)]
  pub const fn new(kind: ToggleKind, on: bool, mixed: bool, pressed: bool) -> Self {
    Self {
      kind,
      on,
      mixed,
      pressed,
    }
  }

  /// The state a flip proposes: on from off or mixed, off from on; a radio only turns on.
  #[must_use]
  pub const fn flipped(&self) -> Option<bool> {
    match self.kind {
      ToggleKind::Radio if self.on => None,
      ToggleKind::Radio => Some(true),
      ToggleKind::Switch | ToggleKind::Checkbox => Some(!self.on || self.mixed),
    }
  }

  /// A key pressed or released while the control holds focus.
  pub fn key(&self, event: &KeyEvent) -> Response<bool> {
    if !plain(event) {
      return Response::IGNORED;
    }

    match (event.key.as_str(), event.down) {
      (" ", true) if event.repeat || self.pressed => Response::CONSUMED,
      (" ", true) => Response::CONSUMED.with_pressed(true),
      (" ", false) if self.pressed => Response::maybe(self.flipped()).with_pressed(false),
      ("Escape", true) if self.pressed => Response::CONSUMED.with_pressed(false),
      _ => Response::IGNORED,
    }
  }

  /// What the control tells a reader of its check.
  #[must_use]
  pub const fn checked(&self) -> Checked {
    if self.mixed {
      Checked::Mixed
    } else if self.on {
      Checked::On
    } else {
      Checked::Off
    }
  }
}

/// An on/off control's behavior, a switch's or a checkbox's: [`ToggleKeys`], a click and a
/// reader's activation flip it, asking its handler for the state it flipped to.
#[must_use]
pub struct Toggle {
  kind: ToggleKind,
  checked: Controlled<bool>,
  mixed: Option<Rc<dyn Fn() -> bool>>,
  pressed: Signal<bool>,
}

impl Toggle {
  fn of(kind: ToggleKind) -> Self {
    Self {
      kind,
      checked: Controlled::new(),
      mixed: None,
      pressed: Signal::new(false),
    }
  }

  /// A switch, off: its label leads.
  pub fn switch() -> Self {
    Self::of(ToggleKind::Switch)
  }

  /// A checkbox, unchecked: its label trails.
  pub fn checkbox() -> Self {
    Self::of(ToggleKind::Checkbox)
  }

  /// Which control.
  #[must_use]
  pub const fn kind(&self) -> ToggleKind {
    self.kind
  }

  /// This control, on where `checked`.
  pub fn set_checked(mut self, checked: bool) -> Self {
    self.checked.set(checked);

    self
  }

  /// This control, on while `checked` returns true, kept current.
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.checked.derive(checked);

    self
  }

  /// This control, mixed while `mixed` returns true, kept current.
  pub fn derive_mixed(mut self, mixed: impl Fn() -> bool + 'static) -> Self {
    self.mixed = Some(Rc::new(mixed));

    self
  }

  /// This control, asking `change` for each flip.
  pub fn handle_change(mut self, change: impl Fn(bool) + 'static) -> Self {
    self.checked.handle(change);

    self
  }

  /// This control, showing `checked` and writing each flip into it.
  pub fn bind_checked(mut self, checked: Signal<bool>) -> Self {
    self.checked.bind_signal(checked);

    self
  }

  fn keys(&self) -> ToggleKeys {
    ToggleKeys::new(
      self.kind,
      self.checked.read().unwrap_or(false),
      self.mixed.as_ref().is_some_and(|mixed| mixed()),
      self.pressed.get_untracked(),
    )
  }

  fn flip(&self) {
    if let Some(on) = crate::untrack(|| self.keys().flipped()) {
      self.checked.ask(on);
    }
  }
}

impl Behavior for Toggle {
  fn role(&self) -> Role {
    match self.kind {
      ToggleKind::Switch => Role::Switch,
      ToggleKind::Checkbox | ToggleKind::Radio => Role::Checkbox,
    }
  }

  fn label_placement(&self) -> LabelPlacement {
    match self.kind {
      ToggleKind::Switch => LabelPlacement::Leading,
      ToggleKind::Checkbox | ToggleKind::Radio => LabelPlacement::Trailing,
    }
  }

  fn on_click(&self, _: &ControlHandle) -> Prevent {
    self.flip();

    Prevent::All
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
    let response = crate::untrack(|| self.keys()).key(event);

    if let Some(pressed) = response.pressed {
      self.pressed.set(pressed);
    }

    if let Some(on) = response.act {
      self.checked.ask(on);
    }

    response.verdict()
  }

  fn on_blur(&self, _: &ControlHandle) {
    if self.pressed.get_untracked() {
      self.pressed.set(false);
    }
  }

  fn on_access(&self, request: AccessRequest, _: &ControlHandle) -> bool {
    match request {
      AccessRequest::Activate | AccessRequest::Toggle => {
        self.flip();

        true
      }
      _ => false,
    }
  }

  fn access(&self) -> AccessFacts {
    AccessFacts {
      checked: Some(self.keys().checked()),
      actions: vec![AccessRequest::Activate],
      ..AccessFacts::default()
    }
  }

  fn state(&self) -> ControlState {
    let keys = self.keys();

    ControlState {
      checked: keys.checked(),
      pressed: self.pressed.get(),
      ..ControlState::DEFAULT
    }
  }
}
