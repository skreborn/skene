//! A text entry.

use crate::{
  behavior::{Behavior, Response},
  seam::{
    access::{Action, Role},
    report::KeyEvent,
  },
  text::editing::{Binding, Command},
};

/// What a text entry reports.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntryAct {
  /// A key bound to an editing command, or a paste.
  Edit(Binding),
  /// `Enter` in an entry of one line: the entry is submitted.
  Activate,
}

/// A text entry's behavior: the editing core's key table ([`Binding::for_key`]), `Enter`
/// submitting an entry of one line, and a read-only entry keeping its moves, its selection
/// and its copy but no edit.
///
/// A secure entry (a password) offers neither copy nor cut.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub struct TextEntry {
  /// Whether it holds several lines.
  pub multiline: bool,
  /// Whether its text can be selected but not changed.
  pub read_only: bool,
  /// Whether it hides what is typed.
  pub secure: bool,
}

impl TextEntry {
  /// An entry of several lines where `multiline`.
  #[inline(always)]
  pub const fn new(multiline: bool) -> Self {
    Self {
      multiline,
      read_only: false,
      secure: false,
    }
  }

  /// This entry, read only where `read_only`.
  #[inline(always)]
  pub const fn with_read_only(mut self, read_only: bool) -> Self {
    self.read_only = read_only;

    self
  }

  /// This entry, hiding what is typed where `secure`.
  #[inline(always)]
  pub const fn with_secure(mut self, secure: bool) -> Self {
    self.secure = secure;

    self
  }

  /// Whether the entry runs `command`: a secure entry neither copies nor cuts.
  #[must_use]
  pub const fn allows(&self, command: &Command) -> bool {
    !(self.secure && matches!(command, Command::Copy | Command::Cut))
  }
}

impl Behavior for TextEntry {
  type Act = EntryAct;

  fn role(&self) -> Role {
    if self.multiline {
      Role::TextEditor
    } else {
      Role::TextField
    }
  }

  fn key(&self, event: &KeyEvent) -> Response<EntryAct> {
    let modifiers = event.modifiers;

    if event.down
      && event.key == "Enter"
      && !self.multiline
      && !modifiers.control
      && !modifiers.alt
      && !modifiers.meta
    {
      return Response::act(EntryAct::Activate);
    }

    match Binding::for_key(event, self.multiline) {
      Some(Binding::Command(command)) if !self.allows(&command) => Response::CONSUMED,
      Some(binding) => Response::act(EntryAct::Edit(binding)),
      None => Response::IGNORED,
    }
  }

  fn actions(&self) -> Vec<Action> {
    vec![Action::Activate]
  }
}
