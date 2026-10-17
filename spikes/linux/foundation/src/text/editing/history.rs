//! An entry's undo and redo steps, typing and removing merged into steps.

use crate::text::Position;

/// How many undo steps an entry keeps.
const LIMIT: usize = 100;

/// One state of an entry an undo or redo returns to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Snapshot {
  /// The text.
  pub text: String,
  /// The selection's fixed end.
  pub anchor: Position,
  /// The selection's moving end.
  pub focus: Position,
}

/// A kind of edit, the kinds whose consecutive occurrences make one undo step among them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EditKind {
  /// One cluster typed at a collapsed caret.
  Typed,
  /// One cluster removed before a collapsed caret.
  RemovedBackward,
  /// One cluster removed after a collapsed caret.
  RemovedForward,
  /// An input method's deletion of text around the caret: one step with the edit right
  /// after it (its commit, a conversion of what it deleted).
  Surrounding,
  /// Any other edit: a step of its own.
  Other,
}

impl EditKind {
  const fn merges(self) -> bool {
    !matches!(self, Self::Other)
  }
}

/// The undo and redo steps.
#[derive(Clone, Debug, Default)]
pub struct History {
  undo: Vec<Snapshot>,
  redo: Vec<Snapshot>,
  run: Option<EditKind>,
  off: bool,
}

impl History {
  /// Keeps steps where `enabled`; otherwise drops every step it kept and records none.
  pub fn set_enabled(&mut self, enabled: bool) {
    if !enabled {
      self.undo.clear();
      self.redo.clear();
      self.run = None;
    }

    self.off = !enabled;
  }

  /// Records `before`, the state before an edit of `kind`, as the next undo step unless the
  /// edit joins the run of the last one; either way the redo steps are dropped. Nothing is
  /// recorded while it is not enabled.
  pub fn record(&mut self, before: Snapshot, kind: EditKind) {
    if self.off {
      return;
    }

    self.redo.clear();

    let joins = match self.run {
      Some(EditKind::Surrounding) => true,
      Some(run) => kind.merges() && run == kind,
      None => false,
    };

    if !joins {
      if self.undo.len() == LIMIT {
        self.undo.remove(0);
      }

      self.undo.push(before);
    }

    self.run = match kind {
      EditKind::Surrounding => Some(EditKind::Surrounding),
      _ if self.run == Some(EditKind::Surrounding) => None,
      kind => kind.merges().then_some(kind),
    };
  }

  /// Ends the run of merged edits: the caret moved or the selection changed.
  pub const fn break_run(&mut self) {
    self.run = None;
  }

  /// Whether nothing was recorded.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.undo.is_empty() && self.redo.is_empty()
  }

  /// The state the last undo step returns to, taken off the undo steps.
  pub fn take_undo(&mut self) -> Option<Snapshot> {
    self.run = None;
    self.undo.pop()
  }

  /// The state the last redo step returns to, taken off the redo steps.
  pub fn take_redo(&mut self) -> Option<Snapshot> {
    self.run = None;
    self.redo.pop()
  }

  /// Keeps `state` as the next undo step, after a redo or a refused undo.
  pub fn push_undo(&mut self, state: Snapshot) {
    self.undo.push(state);
  }

  /// Keeps `state` as the next redo step, after an undo or a refused redo.
  pub fn push_redo(&mut self, state: Snapshot) {
    self.redo.push(state);
  }
}
