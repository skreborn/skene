//! A command: a button.

use std::rc::Rc;

use crate::{
  Signal,
  behavior::{Behavior, ControlHandle, Response, plain},
  seam::{
    access::{AccessFacts, Role},
    control::{AccessRequest, ControlState, Prevent},
    report::KeyEvent,
  },
};

/// A command's key table: `Enter` activates on every press (a held key repeats), `Space`
/// shows the control pressed and activates on its release, `Escape` while it is held lets it
/// go without activating.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[must_use]
pub struct ActivateKeys {
  /// Whether `Space` holds the control pressed now.
  pub pressed: bool,
}

impl ActivateKeys {
  /// A command, held pressed by `Space` where `pressed`.
  #[inline(always)]
  pub const fn new(pressed: bool) -> Self {
    Self { pressed }
  }

  /// A key pressed or released while the command holds focus: its act is an activation.
  pub fn key(&self, event: &KeyEvent) -> Response<()> {
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
}

/// A command's behavior: [`ActivateKeys`], a click and a reader's activation run its handler.
#[must_use]
pub struct Activate {
  activate: Option<Rc<dyn Fn()>>,
  pressed: Signal<bool>,
}

impl Default for Activate {
  fn default() -> Self {
    Self::new()
  }
}

impl Activate {
  /// A command that does nothing until it is given a handler.
  pub fn new() -> Self {
    Self {
      activate: None,
      pressed: Signal::new(false),
    }
  }

  /// This command, running `activate` when it is activated: clicked, pressed from the
  /// keyboard, or activated by a reader.
  pub fn handle_activate(mut self, activate: impl Fn() + 'static) -> Self {
    self.activate = Some(Rc::new(activate));

    self
  }

  fn run(&self) {
    if let Some(activate) = &self.activate {
      activate();
    }
  }
}

impl Behavior for Activate {
  fn role(&self) -> Role {
    Role::Button
  }

  fn on_click(&self, _: &ControlHandle) -> Prevent {
    self.run();

    Prevent::All
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
    let response = ActivateKeys::new(self.pressed.get_untracked()).key(event);

    if let Some(pressed) = response.pressed {
      self.pressed.set(pressed);
    }

    if response.act.is_some() {
      self.run();
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
      AccessRequest::Activate => {
        self.run();

        true
      }
      _ => false,
    }
  }

  fn access(&self) -> AccessFacts {
    AccessFacts {
      actions: vec![AccessRequest::Activate],
      ..AccessFacts::default()
    }
  }

  fn state(&self) -> ControlState {
    ControlState {
      pressed: self.pressed.get(),
      ..ControlState::DEFAULT
    }
  }
}
