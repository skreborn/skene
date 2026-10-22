//! The terminal's editing of a text entry: the caret and the selection over the value,
//! the visible window, and the keys that move, select and edit.

use skene_foundation::{
  ClipboardAction,
  Key,
  KeyEvent,
  Scene,
  notify_change,
  notify_selection_change,
};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
  Terminal,
  engine::wrap_spans,
  records::{AUTOSCROLL_INTERVAL, Drag, Field, Handle, Records},
  text,
  with_records,
};

/// The word segments of `text` as Unicode word boundaries divide it, by cluster: each
/// one's start and end, and whether it is whitespace alone.
fn word_segments(text: &str) -> Vec<(usize, usize, bool)> {
  let mut start = 0;

  text
    .split_word_bounds()
    .map(|segment| {
      let end = start + text::count(segment);
      let bounds = (start, end, segment.chars().all(char::is_whitespace));

      start = end;

      bounds
    })
    .collect()
}

/// The caret position at the start of the word before `caret`: whitespace skipped,
/// then one segment—a word, or a punctuation mark.
fn word_before(value: &str, caret: usize) -> usize {
  word_segments(value)
    .into_iter()
    .rev()
    .find(|(start, _, space)| *start < caret && !space)
    .map_or(0, |(start, ..)| start)
}

/// The caret position at the end of the word after `caret`: whitespace skipped, then
/// one segment—a word, or a punctuation mark.
fn word_after(value: &str, caret: usize) -> usize {
  word_segments(value)
    .into_iter()
    .find(|(_, end, space)| *end > caret && !space)
    .map_or_else(|| text::count(value), |(_, end, _)| end)
}

/// The bounds of the word segment around the cluster at `at`—a word, a whitespace run,
/// or a punctuation mark—`at` clamped into a non-empty `text`.
pub(crate) fn word_bounds(text: &str, at: usize) -> (usize, usize) {
  let segments = word_segments(text);
  let Some(last) = segments.last()
  else {
    return (0, 0);
  };
  let at = at.min(last.1 - 1);

  segments
    .into_iter()
    .find(|(start, end, _)| (*start..*end).contains(&at))
    .map_or((0, 0), |(start, end, _)| (start, end))
}

/// The clusters of `text`.
fn clusters(text: &str) -> Vec<&str> {
  text::clusters(text).map(|(_, cluster)| cluster).collect()
}

/// The bounds of the hard line around `at` in `text`: from after the previous line
/// break to the next one.
fn line_bounds(text: &str, at: usize) -> (usize, usize) {
  let clusters = clusters(text);
  let at = at.min(clusters.len());
  let from = clusters[..at]
    .iter()
    .rposition(|cluster| *cluster == "\n")
    .map_or(0, |index| index + 1);
  let to = clusters[at..]
    .iter()
    .position(|cluster| *cluster == "\n")
    .map_or(clusters.len(), |index| at + index);

  (from, to)
}

/// The wrapped lines of a multi-line value at `width`, and the line holding `caret`.
fn caret_line(value: &str, caret: usize, width: usize) -> (Vec<(usize, String)>, usize) {
  let lines = wrap_spans(value, width);
  let row = lines
    .iter()
    .rposition(|(start, _)| *start <= caret)
    .unwrap_or(0);

  (lines, row)
}

/// A window of `span` rows over `extent` of them, starting at `window` and settled
/// around `at`: held where `at` stays in view, moved the least that brings it in, and
/// never past the extent's end.
pub(crate) const fn settled_window(window: usize, at: usize, extent: usize, span: usize) -> usize {
  let last = extent.saturating_sub(span);
  let window = if window > last { last } else { window };

  if at < window {
    at
  }
  else if at + 1 > window + span {
    at + 1 - span
  }
  else {
    window
  }
}

/// An entry's visible rows, each with its start offset in the shown text, and the
/// caret's row and column among them, `None` while it is out of view.
pub(crate) type VisibleRows = (Vec<(usize, String)>, Option<(usize, usize)>);

/// The cells an entry shows its value in: inside its border and padding.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Area {
  pub(crate) columns: usize,
  pub(crate) rows: usize,
}

impl Field {
  /// The window's geometry in `area`: the caret's position along it, the extent it
  /// covers and the span it shows—the clusters of the value for a field, wrapped rows
  /// for an editor, the span in cells or rows.
  fn window_geometry(&self, area: Area) -> (usize, usize, usize) {
    if self.lines <= 1 {
      return (self.caret, self.count() + 1, area.columns.max(1));
    }

    let (wrapped, row) = caret_line(&self.value, self.caret, area.columns.max(1));

    (row, wrapped.len(), area.rows.max(1))
  }

  /// The window settled around the caret in `area`.
  fn settled_window(&self, area: Area) -> usize {
    if self.lines <= 1 {
      return self
        .clusters
        .settled_window(self.window, self.caret, area.columns.max(1));
    }

    let (at, extent, span) = self.window_geometry(area);

    settled_window(self.window, at, extent, span)
  }

  /// How far the window can scroll from its start in `area`.
  pub(crate) fn window_limit(&self, area: Area) -> usize {
    if self.lines <= 1 {
      return self.clusters.last_window(area.columns.max(1));
    }

    let (_, extent, span) = self.window_geometry(area);

    extent.saturating_sub(span)
  }

  /// Settles the window around the caret in `area`: held where the caret stays in
  /// view, else moved the least that shows it.
  pub(crate) fn settle(&mut self, area: Area) {
    self.window = self.settled_window(area);
    self.window_area = Some(area);
  }

  /// Scrolls the window by `delta`—rows of an editor, columns of a field—in `area`,
  /// clamped to its extent and leaving the caret where it is; `false` when it cannot
  /// move that way.
  pub(crate) fn scroll_window(&mut self, area: Area, delta: i64) -> bool {
    let limit = i64::try_from(self.window_limit(area)).unwrap_or(i64::MAX);
    let window = i64::try_from(self.window).unwrap_or(i64::MAX).min(limit);
    let next = window.saturating_add(delta).clamp(0, limit);

    if next == window {
      return false;
    }

    self.window = usize::try_from(next).unwrap_or(0);
    self.window_area = Some(area);

    true
  }

  /// The rows the entry shows in `area`, each with its start offset in the shown
  /// text—the placeholder while the value is empty—and the caret's row and column
  /// among them, `None` while the window has scrolled it out of view: one windowed row
  /// for a single-line entry, a wrapped row for each row of the area for an editor. A
  /// window settled in another area settles around the caret first.
  pub(crate) fn visible_rows(&self, area: Area) -> VisibleRows {
    let inner = area.columns.max(1);
    let empty = self.value.is_empty();
    let text = if empty {
      self.placeholder.as_deref().unwrap_or("")
    }
    else {
      self.value.as_str()
    };
    let window = if empty {
      0
    }
    else if self.window_area == Some(area) {
      self.window.min(self.window_limit(area))
    }
    else {
      self.settled_window(area)
    };

    if self.lines <= 1 {
      let row = if empty {
        text::fit(text, inner).to_owned()
      }
      else {
        self.shown_row(window, inner)
      };
      let column = self
        .clusters
        .column_of(self.caret)
        .checked_sub(self.clusters.column_of(window));
      let caret = column
        .filter(|column| self.caret >= window && *column < inner)
        .map(|column| (0, column));

      return (vec![(window, row)], caret);
    }

    let lines = area.rows.max(1);
    let (wrapped, row) = caret_line(text, self.caret, inner);
    let (row_start, line) = &wrapped[row];
    let column = text::cells(text::slice(line, 0, self.caret - row_start)).min(inner);
    let rows = wrapped.into_iter().skip(window).take(lines).collect();
    let caret = (window..window + lines)
      .contains(&row)
      .then(|| (row - window, column));

    (rows, caret)
  }

  /// A single-line entry's value from the cluster `window` as it shows in `inner`
  /// cells: every cluster that fits.
  fn shown_row(&self, window: usize, inner: usize) -> String {
    let base = self.clusters.column_of(window);
    let mut row = String::new();

    for index in window..self.clusters.count() {
      if self.clusters.column_of(index + 1) - base > inner {
        break;
      }

      row.push_str(&self.value[self.byte_of(index)..self.byte_of(index + 1)]);
    }

    row
  }

  /// The caret position at the cell (`column`, `row`) of the value shown from `window`
  /// in `area`, clamped into the visible rows and their ends.
  fn caret_in_window(&self, window: usize, area: Area, column: usize, row: usize) -> usize {
    let inner = area.columns.max(1);
    let column = column.min(inner - 1);

    if self.lines <= 1 {
      return self.clusters.index_at_column(window, column);
    }

    let lines = area.rows.max(1);
    let wrapped = wrap_spans(&self.value, inner);
    let target = (window + row.min(lines - 1)).min(wrapped.len() - 1);
    let (start, line) = &wrapped[target];

    start + text::index_at_column(line, column)
  }

  /// The ends of the row the caret is on in `area`: the whole value of a single-line
  /// entry, the wrapped row of an editor.
  fn row_bounds(&self, area: Area) -> (usize, usize) {
    if self.lines <= 1 {
      return (0, self.count());
    }

    let (wrapped, row) = caret_line(&self.value, self.caret, area.columns.max(1));
    let (start, line) = &wrapped[row];

    (*start, start + text::count(line))
  }

  /// The text as the entry holds it: an editor's line breaks as line feeds, a
  /// single-line entry's removed.
  pub(crate) fn sanitized(&self, text: &str) -> String {
    if self.lines > 1 {
      text.replace("\r\n", "\n").replace('\r', "\n")
    }
    else {
      text.replace(['\r', '\n'], "")
    }
  }
}

/// An edit a key made to an entry.
enum Edit {
  Value,
  Caret,
}

/// Applies `event` to `field` shown in `area`: characters insert at the caret,
/// replacing the selection; `Backspace`/`Delete` remove the selection or one character
/// around the caret; arrows and `Home`/`End` move the caret, extending the selection
/// under `Shift` and collapsing it otherwise; `Ctrl` makes the arrows jump and the
/// removals delete whole words, `Ctrl+A` selects the value, `Ctrl+Z` undoes the last
/// edit and `Ctrl+Shift+Z` or `Ctrl+Y` redoes it. In an editor, `Enter` breaks the line,
/// `ArrowUp`/`ArrowDown` move between rows toward the column the first of them left,
/// to the value's ends past the first and last rows, and `Home`/`End` go to the ends
/// of the row, or of the value under `Ctrl`. An entry that is not `editable` takes no
/// edit to its value.
#[expect(
  clippy::too_many_lines,
  reason = "a match over the keys; length is the key count, not complexity"
)]
fn apply(event: &KeyEvent, field: &mut Field, area: Area, editable: bool) -> Option<Edit> {
  let count = field.count();
  let ctrl = event.modifiers.ctrl;
  let alt = event.modifiers.alt;
  let shift = event.modifiers.shift;
  let shortcut = event.shortcut_key();
  let key = shortcut.as_ref();
  let single = match &event.key {
    Key::Character(text) => {
      let mut characters = text.chars();

      characters.next().filter(|_| characters.next().is_none())
    }
    Key::Named(_) => None,
  };
  let selection = field.selection();
  let caret = field.caret;

  match key {
    "a" if ctrl && !alt => {
      field.select_all();

      Some(Edit::Caret)
    }
    "z" if ctrl && !alt && !shift && editable => field.undo().then_some(Edit::Value),
    "z" | "y" if ctrl && !alt && editable => field.redo().then_some(Edit::Value),
    _ if !editable && is_removal(key, ctrl) => None,
    // A removal over a selection removes the selection alone, by the word or not.
    "Delete" if selection.is_some() => {
      let (from, to) = selection?;

      field.remove(from, to);

      Some(Edit::Value)
    }
    "Backspace" if !alt && selection.is_some() => {
      let (from, to) = selection?;

      field.remove(from, to);

      Some(Edit::Value)
    }
    "h" | "w" if ctrl && !alt && selection.is_some() => {
      let (from, to) = selection?;

      field.remove(from, to);

      Some(Edit::Value)
    }
    // `Ctrl+W` matches `Ctrl+Backspace`: legacy keyboard encodings cannot express the
    // latter, and the readline binding is muscle memory anyway.
    "Backspace" | "w" if ctrl && caret > 0 => {
      field.remove(word_before(&field.value, caret), caret);

      Some(Edit::Value)
    }
    "Delete" if ctrl && caret < count => {
      field.remove(caret, word_after(&field.value, caret));

      Some(Edit::Value)
    }
    // Legacy encodings send `Ctrl+Backspace` as `0x08`—the byte of `Ctrl+H`—with no
    // way to tell the two apart; deleting one character honors both readings.
    "h" if ctrl && !alt && caret > 0 => {
      field.remove(caret - 1, caret);

      Some(Edit::Value)
    }
    "Backspace" if !alt && caret > 0 => {
      field.remove(caret - 1, caret);

      Some(Edit::Value)
    }
    "Delete" if caret < count => {
      field.remove(caret, caret + 1);

      Some(Edit::Value)
    }
    "Enter" if field.lines > 1 && editable => {
      field.replace_selection("\n");

      Some(Edit::Value)
    }
    // A plain arrow over a selection collapses it to the end it points at.
    "ArrowLeft" if !shift && !ctrl && selection.is_some() => {
      field.move_caret(selection?.0, false);

      Some(Edit::Caret)
    }
    "ArrowRight" if !shift && !ctrl && selection.is_some() => {
      field.move_caret(selection?.1, false);

      Some(Edit::Caret)
    }
    "ArrowLeft" if ctrl && caret > 0 => {
      field.move_caret(word_before(&field.value, caret), shift);

      Some(Edit::Caret)
    }
    "ArrowRight" if ctrl && caret < count => {
      field.move_caret(word_after(&field.value, caret), shift);

      Some(Edit::Caret)
    }
    "ArrowLeft" if caret > 0 => {
      field.move_caret(caret - 1, shift);

      Some(Edit::Caret)
    }
    "ArrowRight" if caret < count => {
      field.move_caret(caret + 1, shift);

      Some(Edit::Caret)
    }
    "ArrowUp" | "ArrowDown" if field.lines > 1 => {
      let up = key == "ArrowUp";
      // Over a selection, an unshifted move starts from the end it leaves behind.
      let from = match (shift, selection) {
        (false, Some((start, end))) => {
          if up {
            start
          }
          else {
            end
          }
        }
        _ => caret,
      };
      let (wrapped, row) = caret_line(&field.value, from, area.columns.max(1));
      let (row_start, line) = &wrapped[row];
      let goal = field
        .goal
        .unwrap_or_else(|| text::cells(text::slice(line, 0, from - row_start)));
      let target = if up {
        row.checked_sub(1)
      }
      else {
        (row + 1 < wrapped.len()).then_some(row + 1)
      };
      let to = match target {
        Some(target) => {
          let (start, line) = &wrapped[target];

          start + text::index_at_column(line, goal)
        }
        None if up => 0,
        None => count,
      };

      if to == caret && selection.is_none() {
        return None;
      }

      field.move_caret(to, shift);
      field.goal = Some(goal);

      Some(Edit::Caret)
    }
    "Home" | "End" => {
      let (start, end) = if ctrl {
        (0, count)
      }
      else {
        field.row_bounds(area)
      };
      let to = if key == "Home" { start } else { end };

      if to == caret && selection.is_none() {
        return None;
      }

      field.move_caret(to, shift);

      Some(Edit::Caret)
    }
    _ => {
      let character = single.filter(|_| !ctrl && !alt && editable)?;

      field.replace_selection(&character.to_string());

      Some(Edit::Value)
    }
  }
}

/// Whether `key`, under `Ctrl` when `ctrl`, removes text from an entry.
fn is_removal(key: &str, ctrl: bool) -> bool {
  matches!(key, "Backspace" | "Delete") || (ctrl && matches!(key, "h" | "w"))
}

impl Records {
  /// The cells an entry shows its value in: inside its border and padding.
  pub(crate) fn field_area(&self, handle: Handle) -> Area {
    let (width, height) = self.node_size(handle);
    let border = self.node_border(handle);
    let padding = self
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .and_then(|style| style.padding)
      .unwrap_or_default();

    Area {
      columns: usize::try_from(
        width - border.0 - border.2 - i64::from(padding.start) - i64::from(padding.end),
      )
      .unwrap_or(0),
      rows: usize::try_from(
        height - border.1 - border.3 - i64::from(padding.top) - i64::from(padding.bottom),
      )
      .unwrap_or(0),
    }
  }

  /// Whether the entry at `handle` takes edits to its value: it is neither disabled nor
  /// read-only.
  #[expect(
    clippy::unused_self,
    reason = "every reader holds the records, and an entry's editability is asked of it as of the rest"
  )]
  pub(crate) fn is_editable(&self, handle: Handle) -> bool {
    Scene::<Terminal>::read(|scene| scene.is_editable(handle))
  }

  /// The inset from an entry's edge to its value: border plus padding.
  fn field_inset(&self, handle: Handle) -> (i64, i64) {
    let border = self.node_border(handle);
    let padding = self
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .and_then(|style| style.padding)
      .unwrap_or_default();

    (
      border.0 + i64::from(padding.start),
      border.1 + i64::from(padding.top),
    )
  }

  /// An entry's local cell as a column and row of its value: past the inset, clamped
  /// at its start.
  fn entry_cell(&self, handle: Handle, local_x: i64, local_y: i64) -> (usize, usize) {
    let (inset_x, inset_y) = self.field_inset(handle);

    (
      usize::try_from(local_x - inset_x).unwrap_or(0),
      usize::try_from(local_y - inset_y).unwrap_or(0),
    )
  }

  /// The caret position at an entry's local cell, mapped through the inset and the
  /// value's visible window, clamped into the nearest visible row and its ends.
  fn caret_at(&self, handle: Handle, local_x: i64, local_y: i64) -> Option<usize> {
    let area = self.field_area(handle);
    let (column, row) = self.entry_cell(handle, local_x, local_y);
    let field = self.record(handle)?.kind.field()?;

    Some(field.caret_in_window(field.settled_window(area), area, column, row))
  }

  /// Whether the node is an editor, whose window scrolls down its wrapped rows.
  pub(crate) fn is_editor(&self, handle: Handle) -> bool {
    self
      .record(handle)
      .and_then(|record| record.kind.field())
      .is_some_and(|field| field.lines > 1)
  }

  /// An editor's window, clamped to its extent in its laid-out area; `None` for any
  /// other node.
  pub(crate) fn editor_window(&self, handle: Handle) -> Option<u32> {
    let field = self
      .record(handle)
      .and_then(|record| record.kind.field())
      .filter(|field| field.lines > 1)?;
    let area = self.field_area(handle);

    u32::try_from(field.window.min(field.window_limit(area))).ok()
  }

  /// How far an editor's window can scroll in its laid-out area; `None` for any other
  /// node.
  pub(crate) fn editor_window_limit(&self, handle: Handle) -> Option<u32> {
    let field = self
      .record(handle)
      .and_then(|record| record.kind.field())
      .filter(|field| field.lines > 1)?;
    let area = self.field_area(handle);

    u32::try_from(field.window_limit(area)).ok()
  }

  /// Moves an editor's window to `window`, clamped, in its laid-out area.
  pub(crate) fn set_editor_window(&mut self, handle: Handle, window: u32) {
    let area = self.field_area(handle);

    if let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
      .filter(|field| field.lines > 1)
    {
      field.window = usize::try_from(window)
        .unwrap_or(usize::MAX)
        .min(field.window_limit(area));
      field.window_area = Some(area);
    }
  }

  /// Scrolls the single-line field at `handle` by `delta` columns in its laid-out area,
  /// damaging it: `None` when the node is no field or `delta` is zero, else whether it
  /// moved.
  pub(crate) fn scroll_field_by(&mut self, handle: Handle, delta: i64) -> Option<bool> {
    if delta == 0 {
      return None;
    }

    let area = self.field_area(handle);
    let field = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
      .filter(|field| field.lines <= 1)?;
    let moved = field.scroll_window(area, delta);

    if moved {
      self.damage(handle);
    }

    Some(moved)
  }

  /// Settles an entry's window around its caret, in its laid-out area.
  pub(crate) fn settle_entry(&mut self, handle: Handle) {
    let area = self.field_area(handle);

    if let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
    {
      field.settle(area);
    }
  }

  /// Places an entry's caret at a press's local cell, extending the selection from
  /// its anchor or collapsing it, and starts the drag that follows the press.
  pub(crate) fn place_caret(&mut self, handle: Handle, local_x: i64, local_y: i64, extend: bool) {
    let Some(caret) = self.caret_at(handle, local_x, local_y)
    else {
      return;
    };

    if let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
    {
      field.move_caret(caret, extend);
    }

    self.settle_entry(handle);
    self.drag = Some(Drag::Caret(handle));
    self.damage(handle);
  }

  /// Follows a caret drag to the screen cell (`x`, `y`): the caret moves to the
  /// position under the pointer, clamped to the visible value's edge, the anchor held;
  /// past the edge, the autoscroll carries it on from there.
  pub(crate) fn drag_caret(&mut self, handle: Handle, x: u16, y: u16) {
    let (origin_x, origin_y) = self.screen_origin(handle);
    let Some(caret) = self.caret_at(handle, i64::from(x) - origin_x, i64::from(y) - origin_y)
    else {
      return;
    };
    let moved = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
      .is_some_and(|field| {
        let before = (field.caret, field.anchor);

        field.move_caret(caret, true);

        (field.caret, field.anchor) != before
      });

    if moved {
      self.settle_entry(handle);
      self.damage(handle);
    }
  }

  /// The caret drag's overshoot: how far past the entry's visible value the pointer is
  /// held, in columns for a field and rows for an editor, negative before its start;
  /// `None` while the pointer is inside it or the window can scroll no further that way.
  fn caret_drag_overshoot(&self) -> Option<(Handle, i64)> {
    let Some(Drag::Caret(handle)) = self.drag
    else {
      return None;
    };
    let field = self.record(handle)?.kind.field()?;
    let area = self.field_area(handle);
    let (origin_x, origin_y) = self.screen_origin(handle);
    let (inset_x, inset_y) = self.field_inset(handle);
    let (x, y) = self.last_pointer;
    let (_, _, span) = field.window_geometry(area);
    let along = if field.lines <= 1 {
      i64::from(x) - origin_x - inset_x
    }
    else {
      i64::from(y) - origin_y - inset_y
    };
    let span = i64::try_from(span).unwrap_or(i64::MAX);
    let overshoot = if along < 0 {
      along
    }
    else if along >= span {
      along - span + 1
    }
    else {
      return None;
    };
    let room = if overshoot < 0 {
      field.window > 0
    }
    else {
      field.window < field.window_limit(area)
    };

    room.then_some((handle, overshoot))
  }

  /// Whether a caret drag is held past its entry's edge with room to scroll.
  pub(crate) fn autoscrolls(&self) -> bool {
    self.caret_drag_overshoot().is_some()
  }

  /// Steps the caret drag's autoscroll on the frame clock: once per interval while the
  /// pointer is held past the entry's visible value, the window scrolls toward it by
  /// the overshoot and the caret follows to the edge, extending the selection.
  pub(crate) fn autoscroll_caret_drag(&mut self) {
    let Some((handle, overshoot)) = self.caret_drag_overshoot()
    else {
      self.autoscroll_at = None;

      return;
    };
    let now = self.now();
    let interval = AUTOSCROLL_INTERVAL.as_secs_f64();
    let Some(at) = self.autoscroll_at
    else {
      self.autoscroll_at = Some(now);

      return;
    };
    let steps = ((now - at) / interval + 1e-9).floor();

    if steps < 1. {
      return;
    }

    self.autoscroll_at = Some(at + steps * interval);

    let area = self.field_area(handle);
    let (origin_x, origin_y) = self.screen_origin(handle);
    let (x, y) = self.last_pointer;
    let (column, row) = self.entry_cell(handle, i64::from(x) - origin_x, i64::from(y) - origin_y);
    let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
    else {
      return;
    };
    let limit = i64::try_from(field.window_limit(area)).unwrap_or(i64::MAX);
    let window = i64::try_from(field.window).unwrap_or(i64::MAX);
    #[expect(clippy::cast_possible_truncation, reason = "at least one whole step")]
    let delta = overshoot.saturating_mul(steps as i64);
    let window = usize::try_from((window + delta).clamp(0, limit)).unwrap_or(0);
    let caret = field.caret_in_window(window, area, column, row);

    field.window = window;
    field.move_caret(caret, true);
    field.settle(area);
    self.damage(handle);
  }

  /// Selects the word around an entry's caret.
  pub(crate) fn select_entry_word(&mut self, handle: Handle) {
    if let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
    {
      let (from, to) = word_bounds(&field.value, field.caret);

      field.select(from, to);
    }

    self.settle_entry(handle);
    self.damage(handle);
  }

  /// Selects the line around an entry's caret: the whole value of a single-line entry,
  /// the hard line of an editor.
  pub(crate) fn select_entry_line(&mut self, handle: Handle) {
    if let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
    {
      if field.lines > 1 {
        let (from, to) = line_bounds(&field.value, field.caret);

        field.select(from, to);
      }
      else {
        field.select_all();
      }
    }

    self.settle_entry(handle);
    self.damage(handle);
  }

  /// Whether the character at an entry's local cell lies inside its selection.
  pub(crate) fn entry_selection_contains(
    &self,
    handle: Handle,
    local_x: i64,
    local_y: i64,
  ) -> bool {
    let Some((from, to)) = self
      .record(handle)
      .and_then(|record| record.kind.field())
      .and_then(Field::selection)
    else {
      return false;
    };

    self
      .caret_at(handle, local_x, local_y)
      .is_some_and(|at| at >= from && at < to)
  }

  /// Edits the focused entry with `edit`, damaging it; a changed value is returned with
  /// its node for the caller to report.
  fn edit_focused_entry(
    &mut self,
    edit: impl FnOnce(&mut Field, Area, bool) -> Option<Edit>,
  ) -> Option<(skene_foundation::NodeId, String)> {
    let focused = self.focused()?;

    self.edit_entry(focused, edit)
  }

  /// Applies `edit` to the entry at `entry`, given its laid-out area and whether it is
  /// editable, returning the changed value with its node when the edit changed it;
  /// nothing happens when the entry is gone.
  fn edit_entry(
    &mut self,
    entry: Handle,
    edit: impl FnOnce(&mut Field, Area, bool) -> Option<Edit>,
  ) -> Option<(skene_foundation::NodeId, String)> {
    let area = self.field_area(entry);
    let editable = self.is_editable(entry);
    let record = self.record_mut(entry)?;
    let node = record.node;
    let field = record.kind.field_mut()?;
    let edit = edit(field, area, editable)?;
    let value = field.value.clone();

    field.settle(area);
    self.damage(entry);

    match edit {
      Edit::Value => {
        self.queue_restyle(entry);
        self.layout_dirty = true;

        Some((node?, value))
      }
      Edit::Caret => None,
    }
  }
}

/// Applies an editing or caret key to the focused entry, reporting a changed value
/// through the node's change handlers.
pub(crate) fn edit_focused(event: &KeyEvent) {
  let changed = with_records(|records| {
    records.edit_focused_entry(|field, area, editable| apply(event, field, area, editable))
  });

  if let Some((node, value)) = changed {
    notify_change::<Terminal>(node, &value);
  }

  report_selections();
}

/// Reports every entry's selection that moved since it was last reported, through
/// the node's selection handlers.
pub(crate) fn report_selections() {
  let reports = with_records(Records::selection_reports);

  for (node, selection) in reports {
    notify_selection_change::<Terminal>(node, selection);
  }
}

/// Inserts `text` into the focused entry in place of its selection, reporting the
/// changed value through the node's change handlers.
pub(crate) fn paste_focused(text: &str) {
  let target = with_records(|records| records.focused());

  paste_into(target, text);
}

/// Inserts `text`, the terminal's answer to the last clipboard read, into the entry
/// that asked for it, reporting the changed value through the node's change handlers;
/// nothing happens when no entry asked or it is gone.
pub(crate) fn paste_requested(text: &str) {
  let target = with_records(|records| records.paste_target.take());

  paste_into(target, text);
}

/// Inserts `text` into the entry at `target`, in place of its selection, where the
/// entry is editable.
fn paste_into(target: Option<Handle>, text: &str) {
  let Some(target) = target
  else {
    return;
  };
  let changed = with_records(|records| {
    records.edit_entry(target, |field, _, editable| {
      let text = field.sanitized(text);

      if !editable || (text.is_empty() && field.selection().is_none()) {
        return None;
      }

      field.replace_selection(&text);

      Some(Edit::Value)
    })
  });

  if let Some((node, value)) = changed {
    notify_change::<Terminal>(node, &value);
  }

  report_selections();
}

/// Stages the clipboard entry's selection for the clipboard and removes it, reporting
/// the changed value through the node's change handlers; nothing happens when `Cut` is
/// not offered or the entry is not editable.
pub(crate) fn cut_focused() {
  let changed = with_records(|records| {
    if !records.offers(ClipboardAction::Cut) {
      return None;
    }

    let entry = records
      .clipboard_entry()
      .filter(|entry| records.is_editable(*entry))?;

    records.copy_selection();
    records.edit_entry(entry, |field, _, _| {
      let (from, to) = field.selection()?;

      field.remove(from, to);

      Some(Edit::Value)
    })
  });

  if let Some((node, value)) = changed {
    notify_change::<Terminal>(node, &value);
  }

  report_selections();
}
