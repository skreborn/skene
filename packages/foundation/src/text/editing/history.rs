//! An entry's undo and redo steps, typing and removing merged into steps.

use std::collections::VecDeque;

use crate::text::Position;

/// How many undo steps an entry keeps.
const LIMIT: usize = 100;

/// One state of an entry an undo or redo returns to.
#[derive(Clone, PartialEq, Eq, Debug)]
#[must_use]
pub struct Step {
  /// The text.
  pub text: String,
  /// The selection's fixed end.
  pub anchor: Position,
  /// The selection's moving end.
  pub focus: Position,
}

/// A kind of edit, the kinds whose consecutive occurrences make one undo step among them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
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
#[must_use]
pub struct History {
  undo: VecDeque<Step>,
  redo: Vec<Step>,
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
  pub fn record(&mut self, before: Step, kind: EditKind) {
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
        self.undo.pop_front();
      }

      self.undo.push_back(before);
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
  pub fn is_empty(&self) -> bool {
    self.undo.is_empty() && self.redo.is_empty()
  }

  /// The state the last undo step returns to, taken off the undo steps.
  pub fn take_undo(&mut self) -> Option<Step> {
    self.run = None;
    self.undo.pop_back()
  }

  /// The state the last redo step returns to, taken off the redo steps.
  pub fn take_redo(&mut self) -> Option<Step> {
    self.run = None;
    self.redo.pop()
  }

  /// Keeps `state` as the next undo step, after a redo or a refused undo.
  pub fn push_undo(&mut self, state: Step) {
    self.undo.push_back(state);
  }

  /// Keeps `state` as the next redo step, after an undo or a refused redo.
  pub fn push_redo(&mut self, state: Step) {
    self.redo.push(state);
  }
}

#[cfg(test)]
mod tests {
  use super::{EditKind, History, LIMIT, Step};
  use crate::text::Position;

  fn step(text: &str) -> Step {
    Step {
      text: text.to_owned(),
      anchor: Position::new(text.len()),
      focus: Position::new(text.len()),
    }
  }

  #[test]
  fn edits_of_one_kind_merge_until_the_run_breaks() {
    let mut history = History::default();

    history.record(step(""), EditKind::Typed);
    history.record(step("a"), EditKind::Typed);
    history.record(step("ab"), EditKind::RemovedBackward);
    history.break_run();
    history.record(step("a"), EditKind::RemovedBackward);
    history.record(step(""), EditKind::Other);
    history.record(step("x"), EditKind::Other);

    let undone: Vec<String> = std::iter::from_fn(|| history.take_undo())
      .map(|step| step.text)
      .collect();

    assert_eq!(undone, ["x", "", "a", "ab", ""]);
  }

  #[test]
  fn an_input_methods_deletion_joins_the_edit_after_it() {
    let mut history = History::default();

    history.record(step("abc"), EditKind::Surrounding);
    history.record(step("a"), EditKind::Other);
    history.record(step("a\u{5951}"), EditKind::Other);

    assert_eq!(
      history.take_undo().map(|step| step.text).as_deref(),
      Some("a\u{5951}")
    );
    assert_eq!(
      history.take_undo().map(|step| step.text).as_deref(),
      Some("abc")
    );
    assert_eq!(history.take_undo(), None);
  }

  #[test]
  fn the_oldest_step_goes_past_the_limit() {
    let mut history = History::default();

    for count in 0..=LIMIT {
      history.record(step(&count.to_string()), EditKind::Other);
    }

    let mut kept = 0;
    let mut last = None;

    while let Some(step) = history.take_undo() {
      kept += 1;
      last = Some(step.text);
    }

    assert_eq!(kept, LIMIT);
    assert_eq!(last.as_deref(), Some("1"), "the first step was dropped");
  }

  #[test]
  fn a_disabled_history_records_nothing_and_drops_what_it_kept() {
    let mut history = History::default();

    history.record(step(""), EditKind::Other);
    history.set_enabled(false);
    assert!(history.is_empty());

    history.record(step("a"), EditKind::Other);
    assert!(history.is_empty());
  }
}
