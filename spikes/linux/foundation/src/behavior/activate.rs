//! A command: a button.

use crate::{
  behavior::{Behavior, Response, plain},
  seam::{
    access::{Action, Role},
    report::KeyEvent,
  },
};

/// A command's behavior: `Enter` activates on every press (a held key repeats), `Space`
/// shows the control pressed and activates on its release, a click activates.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[must_use]
pub struct Activate {
  /// Whether `Space` holds the control pressed now.
  pub pressed: bool,
}

impl Activate {
  /// A command, held pressed by `Space` where `pressed`.
  #[inline(always)]
  pub const fn new(pressed: bool) -> Self {
    Self { pressed }
  }
}

impl Behavior for Activate {
  type Act = ();

  fn role(&self) -> Role {
    Role::Button
  }

  fn key(&self, event: &KeyEvent) -> Response<()> {
    if !plain(event) {
      return Response::IGNORED;
    }

    match (event.key.as_str(), event.down) {
      ("Enter", true) => Response::act(()),
      (" ", true) if event.repeat || self.pressed => Response::CONSUMED,
      (" ", true) => Response::CONSUMED.with_pressed(true),
      (" ", false) if self.pressed => Response::act(()).with_pressed(false),
      ("Escape", true) if self.pressed => Response::CONSUMED.with_pressed(false),
      _ => Response::IGNORED,
    }
  }

  fn click(&self) -> Option<()> {
    Some(())
  }

  fn action(&self, action: Action) -> Option<()> {
    matches!(action, Action::Activate).then_some(())
  }

  fn actions(&self) -> Vec<Action> {
    vec![Action::Activate]
  }
}
