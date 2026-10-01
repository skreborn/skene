//! A text entry's editing state: its value, caret, selection and undo history.

use skene_foundation::{NodeId, TextSelection};

use super::Records;
use crate::{
  field::Area,
  text::{self, Clusters},
};

/// How many undo steps an entry keeps.
const HISTORY_LIMIT: usize = 100;

/// One state of an entry an undo or redo returns to.
struct Snapshot {
  value: String,
  caret: usize,
  anchor: Option<usize>,
}

/// A kind of edit whose consecutive occurrences make one undo step.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EditRun {
  /// A cluster typed at the caret.
  Typed,
  /// A cluster removed beside the caret.
  Removed,
}

/// A text entry's editing state; the caret and the anchor are cluster positions in
/// the value, `0..=count`.
pub(crate) struct Field {
  /// The value being edited.
  pub(crate) value: String,
  /// The value's clusters, kept with it.
  pub(crate) clusters: Clusters,
  /// The selection's moving end, and the insertion point.
  pub(crate) caret: usize,
  /// The selection's fixed end, `None` while the selection is collapsed.
  pub(crate) anchor: Option<usize>,
  /// The selection last reported or written from outside.
  reported: TextSelection,
  /// The states an undo returns to, oldest first, at most [`HISTORY_LIMIT`].
  undo: Vec<Snapshot>,
  /// The states a redo returns to, oldest first; emptied by an edit.
  redo: Vec<Snapshot>,
  /// The kind of the last edit, which the next edit of the same kind joins as one undo
  /// step; `None` once the caret has moved or the selection changed since.
  run: Option<EditRun>,
  /// The first visible column of a field, or the first visible wrapped row of an
  /// editor; moves only to keep the caret in view, under an autoscroll, or under an
  /// editor's wheel.
  pub(crate) window: usize,
  /// The area the window was last settled or scrolled in, `None` before the first; in
  /// any other the window settles around the caret before it shows.
  pub(crate) window_area: Option<Area>,
  /// The cell column vertical motion aims for, held across consecutive vertical moves.
  pub(crate) goal: Option<usize>,
  /// The text shown while the value is empty.
  pub(crate) placeholder: Option<String>,
  /// The rows the entry asks the layout for; more than one makes it an editor.
  pub(crate) lines: u16,
}

impl Field {
  /// A fresh, empty entry showing `lines` rows.
  pub(crate) const fn new(placeholder: Option<String>, lines: u16) -> Self {
    Self {
      value: String::new(),
      clusters: Clusters::EMPTY,
      caret: 0,
      anchor: None,
      reported: TextSelection::collapsed(0),
      undo: Vec::new(),
      redo: Vec::new(),
      run: None,
      window: 0,
      window_area: None,
      goal: None,
      placeholder,
      lines: if lines == 0 { 1 } else { lines },
    }
  }

  /// The value's length in clusters.
  pub(crate) const fn count(&self) -> usize {
    self.clusters.count()
  }

  /// Measures the value's clusters again, after the value or the terminal's
  /// clustering changed.
  pub(crate) fn reflow(&mut self) {
    self.clusters = Clusters::of(&self.value);
  }

  /// The selected cluster range in document order, `None` while collapsed.
  pub(crate) fn selection(&self) -> Option<(usize, usize)> {
    let anchor = self.anchor?;

    (anchor != self.caret).then(|| (anchor.min(self.caret), anchor.max(self.caret)))
  }

  /// The selected text, `None` while collapsed.
  pub(crate) fn selected_text(&self) -> Option<String> {
    let (from, to) = self.selection()?;

    Some(self.value[self.byte_of(from)..self.byte_of(to)].to_owned())
  }

  /// The selection as the app sees it: from the anchor, or the caret while collapsed,
  /// to the caret.
  pub(crate) fn selection_offsets(&self) -> TextSelection {
    TextSelection::new(self.anchor.unwrap_or(self.caret), self.caret)
  }

  /// The selection, when it differs from the one last reported or written from
  /// outside, which it becomes.
  pub(crate) fn selection_report(&mut self) -> Option<TextSelection> {
    let selection = self.selection_offsets();

    if selection == self.reported {
      return None;
    }

    self.reported = selection;

    Some(selection)
  }

  /// Places the selection from outside the entry's editing, each index clamped to the
  /// value; `false` when it is the selection already.
  pub(crate) fn set_selection(&mut self, selection: TextSelection) -> bool {
    let selection = selection.clamped(self.count());
    let caret = selection.end;
    let anchor = (selection.start != caret).then_some(selection.start);

    if (caret, anchor) == (self.caret, self.anchor) {
      self.reported = self.selection_offsets();

      return false;
    }

    self.caret = caret;
    self.anchor = anchor;
    self.run = None;
    self.goal = None;
    self.reported = self.selection_offsets();

    true
  }

  /// Collapses the selection at the caret.
  pub(crate) const fn collapse(&mut self) {
    self.anchor = None;
    self.run = None;
    self.goal = None;
  }

  /// Selects `from..to`, the caret at `to`.
  pub(crate) fn select(&mut self, from: usize, to: usize) {
    let count = self.count();

    self.caret = to.min(count);
    self.anchor = (from != to).then_some(from.min(count));
    self.run = None;
    self.goal = None;
  }

  /// Sets the value from outside the entry's editing, sanitized as
  /// [`sanitized`](Self::sanitized) says, as one undo step, the caret clamped and the
  /// selection collapsed; a value over an empty entry with no history is the initial
  /// one and records nothing, and nothing happens when `value` is the value already.
  pub(crate) fn set_value(&mut self, value: &str) -> bool {
    let value = self.sanitized(value);

    if self.value == value {
      return false;
    }

    if !(self.value.is_empty() && self.undo.is_empty() && self.redo.is_empty()) {
      self.record(None);
    }

    self.value = value;
    self.reflow();
    self.caret = self.caret.min(self.count());
    self.anchor = None;
    self.goal = None;

    true
  }

  /// Records the state before an edit of `kind` as the next undo step, unless the
  /// edit joins the run of the last one; either way the redo states are dropped.
  fn record(&mut self, kind: Option<EditRun>) {
    self.redo.clear();

    if kind.is_none() || kind != self.run {
      if self.undo.len() == HISTORY_LIMIT {
        self.undo.remove(0);
      }

      self.undo.push(self.snapshot());
    }

    self.run = kind;
  }

  /// Returns to the state before the last undo step, the state left becoming a redo
  /// step; `false` with nothing to undo.
  pub(crate) fn undo(&mut self) -> bool {
    let Some(snapshot) = self.undo.pop()
    else {
      return false;
    };

    self.redo.push(self.snapshot());
    self.restore(snapshot);

    true
  }

  /// Returns to the state the last undo left, that state becoming an undo step again;
  /// `false` with nothing to redo.
  pub(crate) fn redo(&mut self) -> bool {
    let Some(snapshot) = self.redo.pop()
    else {
      return false;
    };

    self.undo.push(self.snapshot());
    self.restore(snapshot);

    true
  }

  fn snapshot(&self) -> Snapshot {
    Snapshot {
      value: self.value.clone(),
      caret: self.caret,
      anchor: self.anchor,
    }
  }

  fn restore(&mut self, snapshot: Snapshot) {
    self.value = snapshot.value;
    self.reflow();
    self.caret = snapshot.caret;
    self.anchor = snapshot.anchor;
    self.run = None;
    self.goal = None;
  }

  /// Selects the whole value.
  pub(crate) fn select_all(&mut self) {
    self.select(0, self.count());
  }

  /// Moves the caret to `caret`, extending the selection from the anchor—the caret's
  /// old position when none—or collapsing it.
  pub(crate) fn move_caret(&mut self, caret: usize, extend: bool) {
    let caret = caret.min(self.count());

    if extend {
      let anchor = self.anchor.unwrap_or(self.caret);

      self.anchor = (anchor != caret).then_some(anchor);
    }
    else {
      self.anchor = None;
    }

    self.caret = caret;
    self.run = None;
    self.goal = None;
  }

  /// Replaces the selection—or inserts at the caret—with `text`, leaving the caret
  /// after it. One cluster inserted at the caret joins the run of clusters typed
  /// before it as one undo step; anything else is a step of its own.
  pub(crate) fn replace_selection(&mut self, text: &str) {
    let selection = self.selection();
    let (from, to) = selection.unwrap_or((self.caret, self.caret));
    let typed = selection.is_none() && text::count(text) == 1;
    let start = self.byte_of(from);

    self.record(typed.then_some(EditRun::Typed));
    self.value.replace_range(start..self.byte_of(to), text);
    self.reflow();
    self.caret = self.clusters.index_of_byte(start + text.len());
    self.anchor = None;
    self.goal = None;
  }

  /// Removes `from..to`, leaving the caret at `from`. One cluster removed beside a
  /// collapsed caret joins the run of clusters removed before it as one undo step;
  /// anything else is a step of its own.
  pub(crate) fn remove(&mut self, from: usize, to: usize) {
    let single = self.selection().is_none() && to == from + 1;
    let start = self.byte_of(from);

    self.record(single.then_some(EditRun::Removed));
    self.value.replace_range(start..self.byte_of(to), "");
    self.reflow();
    self.caret = self.clusters.index_of_byte(start);
    self.anchor = None;
    self.goal = None;
  }

  /// The byte offset of the cluster at `position`, or the value's end.
  pub(crate) fn byte_of(&self, position: usize) -> usize {
    self.clusters.byte_of(position)
  }
}

impl Records {
  /// The selection of every entry whose selection changed since it was last reported
  /// or written from outside, with the entry's node.
  pub(crate) fn selection_reports(&mut self) -> Vec<(NodeId, TextSelection)> {
    let mut reports = Vec::new();

    for index in 0..self.entries.len() {
      let Some(record) = self.record_mut(self.entries[index])
      else {
        continue;
      };
      let node = record.node;

      if let Some(selection) = record.kind.field_mut().and_then(Field::selection_report)
        && let Some(node) = node
      {
        reports.push((node, selection));
      }
    }

    reports
  }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod properties {
  use proptest::prelude::*;

  use super::Field;
  use crate::text::{self, Clusters};

  /// One edit a user makes to an entry.
  #[derive(Debug, Clone)]
  enum Step {
    Type(&'static str),
    Paste(&'static str),
    RemoveBefore,
    RemoveAfter,
    MoveCaret(usize, bool),
    SelectAll,
    Undo,
    Redo,
    SetValue(&'static str),
  }

  /// Edits mostly of the clusters an entry is hard on: wide ones, combining marks
  /// joining what is before them, and multi-cluster pastes.
  fn arbitrary_step() -> impl Strategy<Value = Step> {
    const CLUSTERS: &[&str] = &["a", "b", " ", "日", "e", "\u{301}", "👍"];
    const PASTES: &[&str] = &["", "xy", "日本 語", "e\u{301}e"];

    prop_oneof![
      4 => proptest::sample::select(CLUSTERS).prop_map(Step::Type),
      1 => proptest::sample::select(PASTES).prop_map(Step::Paste),
      2 => Just(Step::RemoveBefore),
      1 => Just(Step::RemoveAfter),
      2 => (0..12_usize, any::<bool>()).prop_map(|(caret, extend)| Step::MoveCaret(caret, extend)),
      1 => Just(Step::SelectAll),
      2 => Just(Step::Undo),
      1 => Just(Step::Redo),
      1 => proptest::sample::select(PASTES).prop_map(Step::SetValue),
    ]
  }

  /// Applies `step` to `field` as the entry's editing does.
  fn apply(field: &mut Field, step: &Step) {
    match step {
      Step::Type(text) | Step::Paste(text) => field.replace_selection(text),
      Step::RemoveBefore => match field.selection() {
        Some((from, to)) => field.remove(from, to),
        None if field.caret > 0 => field.remove(field.caret - 1, field.caret),
        None => {}
      },
      Step::RemoveAfter => match field.selection() {
        Some((from, to)) => field.remove(from, to),
        None if field.caret < field.count() => field.remove(field.caret, field.caret + 1),
        None => {}
      },
      Step::MoveCaret(caret, extend) => field.move_caret(*caret, *extend),
      Step::SelectAll => field.select_all(),
      Step::Undo => {
        let _ = field.undo();
      }
      Step::Redo => {
        let _ = field.redo();
      }
      Step::SetValue(value) => {
        let _ = field.set_value(value);
      }
    }
  }

  /// Where each cluster of `clusters` starts, its byte and its cell, the end last.
  fn boundaries(clusters: &Clusters) -> Vec<(usize, usize)> {
    (0..=clusters.count())
      .map(|index| (clusters.byte_of(index), clusters.column_of(index)))
      .collect()
  }

  /// The state an undo or redo returns to.
  fn state(field: &Field) -> (String, usize, Option<usize>) {
    (field.value.clone(), field.caret, field.anchor)
  }

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Any run of edits keeps the caret and the anchor on clusters of the value and the
    /// clusters true to it, and an undo a redo follows changes nothing.
    #[test]
    fn any_run_of_edits_keeps_the_entry_whole(
      steps in proptest::collection::vec(arbitrary_step(), 0..40),
    ) {
      text::reset();

      let mut field = Field::new(None, 1);

      for step in &steps {
        apply(&mut field, step);

        prop_assert!(field.caret <= field.count(), "{step:?} left the caret past the end");
        prop_assert!(
          field.anchor.is_none_or(|anchor| anchor <= field.count() && anchor != field.caret),
          "{step:?} left the anchor off the value"
        );
        prop_assert_eq!(boundaries(&field.clusters), boundaries(&Clusters::of(&field.value)));

        let before = state(&field);

        if field.undo() {
          prop_assert!(field.redo(), "a redo follows an undo");
          prop_assert_eq!(state(&field), before, "an undo a redo follows changes nothing");
        }
      }
    }
  }
}
