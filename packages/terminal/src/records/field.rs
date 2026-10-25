//! A text entry's presentation: the window the entry shows its value through, over the
//! editing state the scene keeps for it, the shared editing core over the cell engine,
//! masked where the entry is secure, which runs with no records borrowed.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{
  NodeId,
  Scene,
  text::{
    Laid as _,
    MaskState,
    Masked,
    Position,
    TextIndex,
    TextWrap,
    editing::{self, Editor},
  },
};

use super::Records;
use crate::{
  Terminal,
  engine::{CellEngine, Drawn, drawn, engine, paragraph_style, whole, wrap_spans},
  field::{Area, settled_window},
  layout::{cell_point, cells_of},
  text::{self, CellScale},
};

/// An entry's editing core, over the cell engine masked as its [`MaskState`] says.
pub(crate) type EntryEditor = Editor<Masked<CellEngine>>;

/// An entry's editing state, which the scene keeps for its node.
pub(crate) type EntryState = editing::Entry<CellEngine>;

/// What a secure entry draws each cluster of its value as.
const MARK: char = '\u{2022}';

/// One cluster of an entry's visible value: the cell it starts in and the row it is on
/// within the entry's area, its text and the cells it advances, and whether it is
/// selected.
pub(crate) struct VisibleCluster {
  pub(crate) column: usize,
  pub(crate) row: usize,
  pub(crate) text: String,
  pub(crate) advance: usize,
  pub(crate) selected: bool,
}

/// What an entry shows in its area: the clusters of its value or, while the value is
/// empty, of its placeholder, and the caret's row and column, `None` while the window has
/// scrolled it out of view; an editor's caret at the end of a full row stands at the
/// area's edge, past its last cell.
pub(crate) struct Visible {
  pub(crate) clusters: Vec<VisibleCluster>,
  pub(crate) placeholder: bool,
  pub(crate) caret: Option<(usize, usize)>,
}

/// A text entry's presentation: the window over its value, which the editing state the
/// scene keeps for the entry ([`EntryState`]) holds.
pub(crate) struct Field {
  /// The first visible cell of a field, or the first visible row of an editor; moves only
  /// to keep the caret in view, under an autoscroll, or under the wheel.
  pub(crate) window: usize,
  /// The area the window was last settled or scrolled in, `None` before the first; in any
  /// other the window settles around the caret before it shows.
  pub(crate) window_area: Option<Area>,
  /// The text shown while the value is empty, as its last placeholder content said.
  pub(crate) placeholder: Option<String>,
  /// The rows the entry asks the layout for; more than one makes it an editor.
  pub(crate) lines: u16,
  /// The cells the value's widest line spans, as last read.
  widest: Cell<usize>,
}

impl Field {
  /// A fresh entry showing `lines` rows, with no placeholder: an editor where they are
  /// more than one, else a field of one line.
  pub(crate) fn new(lines: u16) -> Self {
    Self {
      window: 0,
      window_area: None,
      placeholder: None,
      lines: lines.max(1),
      widest: Cell::new(0),
    }
  }

  /// The editing state of the fresh, empty entry `node`: an editor wrapping its lines at
  /// its width where `multiline`, else a field of one unwrapped line, its value masked by
  /// [`MARK`] once it is secure.
  pub(crate) fn entry(node: NodeId, multiline: bool) -> EntryState {
    EntryState::new::<Terminal>(
      node,
      Self::editor_of(multiline, Rc::new(MaskState::new(MARK))),
    )
  }

  /// An editing core over the cell engine masked as `mask` says, at one cell's scale: of
  /// several lines wrapping at its width where `multiline`, else of one unwrapped line.
  pub(crate) fn editor_of(multiline: bool, mask: Rc<MaskState>) -> EntryEditor {
    let style = if multiline {
      paragraph_style()
    }
    else {
      paragraph_style().with_wrap(TextWrap::None)
    };
    let mut editor = Editor::new(Masked::new(engine(), mask), CellScale::ONE.style(), style);

    editor.set_multiline(multiline);

    editor
  }

  /// Whether the entry is an editor, of several rows.
  pub(crate) const fn is_editor(&self) -> bool {
    self.lines > 1
  }

  /// The cells the value `entry` holds spans at its widest line; while an operation
  /// holds the editor, or where the entry cannot be read, as last read.
  pub(crate) fn widest(&self, entry: Option<&EntryState>) -> usize {
    if let Some(entry) = entry
      && let Ok(editor) = entry.editor().try_borrow()
    {
      self
        .widest
        .set(Self::widest_of(&entry.mask().shown(editor.text())));
    }

    self.widest.get()
  }

  /// The cells `text`'s widest line spans.
  pub(crate) fn widest_of(text: &str) -> usize {
    text.split('\n').map(text::cells).max().unwrap_or(0)
  }

  /// Lays an editor's value out at the width of `area`; a field's lays out unwrapped.
  pub(crate) fn fit(editor: &mut EntryEditor, lines: u16, area: Area) {
    if lines > 1 {
      editor.set_width(Some(cells_of(area.columns.max(1))));
    }
  }

  /// The window's geometry in `area`: the caret's position along it, the extent it covers
  /// and the span it shows, in cells of a field and in rows of an editor.
  fn window_geometry(&self, editor: &EntryEditor, area: Area) -> (usize, usize, usize) {
    let laid = editor.laid();
    let caret = editor.caret_rect();

    if self.is_editor() {
      return (whole(caret.origin.y), laid.line_count(), area.rows.max(1));
    }

    let end = laid
      .caret(Position::new(editor.text().len()))
      .primary
      .x
      .max(laid.size().width);

    (
      whole(caret.origin.x),
      whole(end.ceil()) + 1,
      area.columns.max(1),
    )
  }

  /// Every cluster the editor's value draws, in bytes of the value, with what it shows:
  /// the cluster itself, or a secure entry's mark.
  fn drawn_value(editor: &EntryEditor) -> Vec<(Drawn, String)> {
    let laid = editor.laid();
    let value = editor.text();

    if !laid.is_substituted() {
      return drawn(laid.inner(), value, editor.index())
        .into_iter()
        .map(|cluster| (cluster, value[cluster.at..cluster.end].to_owned()))
        .collect();
    }

    let shown = editor.engine().substitute().shown(value);

    drawn(laid.inner(), &shown, &TextIndex::new(&shown))
      .into_iter()
      .map(|cluster| {
        let real = Drawn {
          at: laid.real_index(cluster.at),
          end: laid.real_index(cluster.end),
          ..cluster
        };

        (real, shown[cluster.at..cluster.end].to_owned())
      })
      .collect()
  }

  /// `window` moved past the wide cluster of a field's value it starts inside, if any.
  fn past_split(&self, editor: &EntryEditor, window: usize) -> usize {
    if self.is_editor() {
      return window;
    }

    Self::drawn_value(editor)
      .into_iter()
      .find(|(cluster, _)| cluster.column < window && window < cluster.column + cluster.advance)
      .map_or(window, |(cluster, _)| cluster.column + cluster.advance)
  }

  /// How far the window can scroll from its start in `area`.
  fn window_limit(&self, editor: &EntryEditor, area: Area) -> usize {
    let (_, extent, span) = self.window_geometry(editor, area);

    self.past_split(editor, extent.saturating_sub(span))
  }

  /// The window settled around the caret in `area`: held where the caret stays in view,
  /// else moved the least that shows it.
  fn settled_window(&self, editor: &EntryEditor, area: Area) -> usize {
    let (at, extent, span) = self.window_geometry(editor, area);

    self.past_split(editor, settled_window(self.window, at, extent, span))
  }

  /// The window as the entry shows it in `area`: held where it was last settled or
  /// scrolled in that area, clamped to the extent, and settled around the caret in any
  /// other.
  fn shown_window(&self, editor: &EntryEditor, area: Area) -> usize {
    if self.window_area == Some(area) {
      self.window.min(self.window_limit(editor, area))
    }
    else {
      self.settled_window(editor, area)
    }
  }

  /// Settles the window over the value `entry` holds around the caret in `area`: held
  /// where the caret stays in view, else moved the least that shows it.
  pub(crate) fn settle(&mut self, entry: &EntryState, area: Area) {
    let Ok(editor) = entry.editor().try_borrow()
    else {
      return;
    };
    let window = self.settled_window(&editor, area);

    drop(editor);
    self.window = window;
    self.window_area = Some(area);
  }

  /// How far the window over the value `entry` holds can scroll from its start in `area`;
  /// nothing while an operation holds the editor.
  pub(crate) fn scroll_limit(&self, entry: &EntryState, area: Area) -> usize {
    entry
      .editor()
      .try_borrow()
      .map_or(0, |editor| self.window_limit(&editor, area))
  }

  /// The window as the entry shows the value `entry` holds in `area`.
  pub(crate) fn visible_window(&self, entry: &EntryState, area: Area) -> usize {
    entry
      .editor()
      .try_borrow()
      .map_or(self.window, |editor| self.shown_window(&editor, area))
  }

  /// Scrolls the window over the value `entry` holds by `delta` (rows of an editor, cells
  /// of a field) in `area`, clamped to its extent and leaving the caret where it is;
  /// `false` when it cannot move that way.
  pub(crate) fn scroll_window(&mut self, entry: &EntryState, area: Area, delta: i64) -> bool {
    let Ok(editor) = entry.editor().try_borrow()
    else {
      return false;
    };
    let limit = i64::try_from(self.window_limit(&editor, area)).unwrap_or(i64::MAX);
    let window = i64::try_from(self.shown_window(&editor, area))
      .unwrap_or(i64::MAX)
      .min(limit);
    let next = window.saturating_add(delta).clamp(0, limit);

    if next == window {
      return false;
    }

    let next = self.past_split(&editor, usize::try_from(next).unwrap_or(0));

    drop(editor);
    self.window = next;
    self.window_area = Some(area);

    true
  }

  /// The layout position standing for the cell (`column`, `row`) of `area`, the value
  /// shown from `window`: clamped into the area, and a hair into the cell, so a press on
  /// a cluster's first cell lands before it and one on a wide cluster's second cell after
  /// it.
  pub(crate) fn point_at(
    &self,
    window: usize,
    area: Area,
    column: usize,
    row: usize,
  ) -> (f32, f32) {
    let column = column.min(area.columns.max(1) - 1);
    let row = row.min(area.rows.max(1) - 1);
    let point = if self.is_editor() {
      cell_point(column, row + window)
    }
    else {
      cell_point(column + window, row)
    };

    (point.x, point.y)
  }

  /// What the entry shows in `area`: the clusters of the value `entry` holds in the
  /// window, or its placeholder's while the value is empty, the selection marked where
  /// `focused`, and the caret's cell. Nothing shows while an operation holds the editor.
  pub(crate) fn visible(&self, entry: &EntryState, area: Area, focused: bool) -> Visible {
    let inner = area.columns.max(1);
    let rows = if self.is_editor() {
      area.rows.max(1)
    }
    else {
      1
    };
    let Ok(editor) = entry.editor().try_borrow()
    else {
      return Visible {
        clusters: Vec::new(),
        placeholder: false,
        caret: None,
      };
    };
    let value = editor.text();
    let placeholder = value.is_empty();
    let window = if placeholder {
      0
    }
    else {
      self.shown_window(&editor, area)
    };
    let (shift_x, shift_y) = if self.is_editor() {
      (0, window)
    }
    else {
      (window, 0)
    };
    let mut clusters = Vec::new();

    if placeholder {
      let shown = self.placeholder.as_deref().unwrap_or("");
      let lines = if self.is_editor() {
        wrap_spans(shown, inner)
      }
      else {
        vec![(0, text::fit(shown, inner).to_owned())]
      };

      for (row, (_, line)) in lines.into_iter().take(rows).enumerate() {
        let mut column = 0;

        for (_, cluster) in text::clusters(&line) {
          let advance = text::width(cluster);

          clusters.push(VisibleCluster {
            column,
            row,
            text: cluster.to_owned(),
            advance,
            selected: false,
          });
          column += advance;
        }
      }
    }
    else {
      let selected = editor.selected();

      for (cluster, shown) in Self::drawn_value(&editor) {
        let (Some(column), Some(row)) = (
          cluster.column.checked_sub(shift_x),
          cluster.top.checked_sub(shift_y),
        )
        else {
          continue;
        };

        if column + cluster.advance > inner || row >= rows {
          continue;
        }

        clusters.push(VisibleCluster {
          column,
          row,
          text: shown,
          advance: cluster.advance,
          selected: focused && selected.contains(&cluster.at),
        });
      }
    }

    let last = if self.is_editor() { inner } else { inner - 1 };
    let caret = editor.caret_rect();
    let caret = whole(caret.origin.x)
      .checked_sub(shift_x)
      .zip(whole(caret.origin.y).checked_sub(shift_y))
      .filter(|(column, row)| *column <= last && *row < rows)
      .map(|(column, row)| (row, column));

    Visible {
      clusters,
      placeholder,
      caret,
    }
  }
}

impl Records {
  /// The text entry at `handle`, if it is one.
  pub(crate) fn field(&self, handle: super::Handle) -> Option<&Field> {
    self.record(handle)?.kind.field()
  }

  /// The text entry at `handle`, if it is one, to change.
  pub(crate) fn field_mut(&mut self, handle: super::Handle) -> Option<&mut Field> {
    self.record_mut(handle)?.kind.field_mut()
  }

  /// The editing state the scene keeps for the entry at `handle`, where it is one and
  /// the scene is free to read.
  #[expect(
    clippy::unused_self,
    reason = "every reader holds the records, and an entry's editing state is asked of it as its window is"
  )]
  pub(crate) fn entry(&self, handle: super::Handle) -> Option<Rc<EntryState>> {
    Scene::<Terminal>::entry_of(handle)
  }

  /// Every entry's editing state, in build order.
  pub(crate) fn entry_states(&self) -> Vec<Rc<EntryState>> {
    self
      .entries
      .iter()
      .filter_map(|entry| self.entry(*entry))
      .collect()
  }
}

#[cfg(test)]
mod properties {
  use std::rc::Rc;

  use proptest::prelude::*;
  use skene_foundation::{
    Edit,
    TextSelection,
    text::{Direction, MaskState, Round, TextBoundary, TextIndex, editing::Command},
  };

  use super::{EntryEditor, Field};
  use crate::{engine::drawn, text};

  /// One edit a user makes to an entry.
  #[derive(Debug, Clone)]
  enum Step {
    Type(&'static str),
    Paste(&'static str),
    RemoveBefore,
    RemoveAfter,
    Select(usize, usize),
    SelectAll,
    Undo,
    Redo,
    Write(&'static str),
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
      2 => (0..12_usize, 0..12_usize).prop_map(|(anchor, caret)| Step::Select(anchor, caret)),
      1 => Just(Step::SelectAll),
      2 => Just(Step::Undo),
      1 => Just(Step::Redo),
      1 => proptest::sample::select(PASTES).prop_map(Step::Write),
    ]
  }

  /// Applies `step` to `editor` as the entry's operations do.
  fn apply(editor: &mut EntryEditor, step: &Step) {
    let command = match step {
      Step::Type(text) => Command::Insert((*text).to_owned()),
      Step::Paste(text) => Command::Paste((*text).to_owned()),
      Step::RemoveBefore => Command::Delete(TextBoundary::Grapheme, Direction::Backward),
      Step::RemoveAfter => Command::Delete(TextBoundary::Grapheme, Direction::Forward),
      Step::SelectAll => Command::SelectAll,
      Step::Undo => Command::Undo,
      Step::Redo => Command::Redo,
      Step::Select(anchor, caret) => {
        let _ = editor.set_selection(TextSelection::new(*anchor, *caret));

        return;
      }
      Step::Write(text) => {
        let _ = editor.write(&Edit::new(*text));

        return;
      }
    };
    let _ = editor.perform(command);
  }

  /// The state an undo or redo returns to.
  fn state(editor: &EntryEditor) -> (String, usize, usize) {
    (
      editor.text().to_owned(),
      editor.anchor().index,
      editor.focus().index,
    )
  }

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Any run of edits keeps the selection's ends on clusters of the value, its index and
    /// its cells true to it, and an undo a redo follows changes nothing.
    #[test]
    fn any_run_of_edits_keeps_the_entry_whole(
      steps in proptest::collection::vec(arbitrary_step(), 0..40),
    ) {
      text::reset();

      let mut editor = Field::editor_of(false, Rc::new(MaskState::new('*')));

      editor.set_arbiter(Accepting);

      for step in &steps {
        apply(&mut editor, step);

        let index = TextIndex::new(editor.text());

        for end in [editor.anchor().index, editor.focus().index] {
          prop_assert!(end <= editor.text().len(), "{step:?} left an end past the value");
          prop_assert_eq!(
            index.snap_to_grapheme(end, Round::Down),
            end,
            "{:?} left an end inside a cluster",
            step
          );
        }

        prop_assert_eq!(editor.index().grapheme_count(), index.grapheme_count());

        let laid = editor.laid();
        let cells: usize = drawn(laid.inner(), editor.text(), editor.index())
          .iter()
          .map(|cluster| cluster.advance)
          .sum();

        prop_assert_eq!(cells, text::cells(editor.text()), "the value lays out in its cells");

        let selection = editor.selection();

        prop_assert!(selection.start <= index.grapheme_count() && selection.end <= index.grapheme_count());

        let before = state(&editor);

        if editor.perform(Command::Undo).handled {
          prop_assert!(editor.perform(Command::Redo).handled, "a redo follows an undo");
          prop_assert_eq!(state(&editor), before, "an undo a redo follows changes nothing");
        }
      }
    }
  }

  /// An arbiter accepting every edit, as an uncontrolled entry's node does.
  struct Accepting;

  impl skene_foundation::text::editing::Arbiter for Accepting {
    fn propose(
      &mut self,
      _proposed: &Edit,
      _applied: bool,
      _composing: bool,
    ) -> skene_foundation::Verdict {
      skene_foundation::Verdict::Accept
    }

    fn settle(&mut self) -> Option<Edit> {
      None
    }
  }
}
