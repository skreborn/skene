//! An on/off control: a switch, a checkbox, a radio.

use crate::{
  behavior::{Behavior, Response, plain},
  seam::{
    access::{AccessFacts, Action, Checked, Role},
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

/// An on/off control's behavior: `Space` shows it pressed and flips it on its release, a
/// click flips it; `Enter` is left to the ancestors (a form's default action), as a browser's
/// checkbox leaves it.
///
/// It proposes the state the reader flipped it to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub struct Toggle {
  /// Which control.
  pub kind: ToggleKind,
  /// Whether it is on.
  pub on: bool,
  /// Whether it is mixed (a checkbox's third state).
  pub mixed: bool,
  /// Whether `Space` holds it pressed now.
  pub pressed: bool,
}

impl Toggle {
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
}

impl Behavior for Toggle {
  type Act = bool;

  fn role(&self) -> Role {
    match self.kind {
      ToggleKind::Switch => Role::Switch,
      ToggleKind::Checkbox => Role::Checkbox,
      ToggleKind::Radio => Role::Radio,
    }
  }

  fn key(&self, event: &KeyEvent) -> Response<bool> {
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

  fn click(&self) -> Option<bool> {
    self.flipped()
  }

  fn action(&self, action: Action) -> Option<bool> {
    match action {
      Action::Activate | Action::Toggle => self.flipped(),
      _ => None,
    }
  }

  fn actions(&self) -> Vec<Action> {
    vec![Action::Activate]
  }

  fn facts(&self) -> AccessFacts {
    AccessFacts {
      checked: Some(if self.mixed {
        Checked::Mixed
      } else if self.on {
        Checked::On
      } else {
        Checked::Off
      }),
      ..AccessFacts::default()
    }
  }
}
