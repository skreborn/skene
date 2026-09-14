//! The terminal's editing of a text entry: the caret and the selection over the value,
//! the visible window, and the keys that move, select and edit.

use skene_core::{KeyEvent, notify_change};

use crate::{
  Tui,
  paint::wrap_spans,
  store::{AUTOSCROLL_INTERVAL, Drag, Field, Handle, Store},
  with_store,
};

/// A caret-motion character class; each maximal same-class run is one word, so a run
/// of punctuation breaks like a word of its own.
#[derive(PartialEq, Eq, Clone, Copy)]
enum CharClass {
  Whitespace,
  Punctuation,
  Word,
}

/// The class of one character: word characters are alphanumerics and `_`.
fn char_class(character: char) -> CharClass {
  if character.is_whitespace() {
    CharClass::Whitespace
  }
  else if character.is_alphanumeric() || character == '_' {
    CharClass::Word
  }
  else {
    CharClass::Punctuation
  }
}

/// The caret position at the start of the word before `caret`: whitespace skipped,
/// then one same-class run.
fn word_before(value: &str, caret: usize) -> usize {
  let characters: Vec<char> = value.chars().collect();
  let mut position = caret;

  while position > 0 && char_class(characters[position - 1]) == CharClass::Whitespace {
    position -= 1;
  }

  if position > 0 {
    let class = char_class(characters[position - 1]);

    while position > 0 && char_class(characters[position - 1]) == class {
      position -= 1;
    }
  }

  position
}

/// The caret position at the end of the word after `caret`: whitespace skipped, then
/// one same-class run.
fn word_after(value: &str, caret: usize) -> usize {
  let characters: Vec<char> = value.chars().collect();
  let mut position = caret;

  while position < characters.len() && char_class(characters[position]) == CharClass::Whitespace {
    position += 1;
  }

  if position < characters.len() {
    let class = char_class(characters[position]);

    while position < characters.len() && char_class(characters[position]) == class {
      position += 1;
    }
  }

  position
}

/// The bounds of the same-class run around the character at `at`—a word, a whitespace
/// gap, or a punctuation run—`at` clamped into a non-empty `text`.
pub(crate) fn word_bounds(text: &str, at: usize) -> (usize, usize) {
  let characters: Vec<char> = text.chars().collect();

  if characters.is_empty() {
    return (0, 0);
  }

  let at = at.min(characters.len() - 1);
  let class = char_class(characters[at]);
  let mut from = at;
  let mut to = at + 1;

  while from > 0 && char_class(characters[from - 1]) == class {
    from -= 1;
  }

  while to < characters.len() && char_class(characters[to]) == class {
    to += 1;
  }

  (from, to)
}

/// The bounds of the hard line around `at` in `text`: from after the previous line
/// break to the next one.
fn line_bounds(text: &str, at: usize) -> (usize, usize) {
  let characters: Vec<char> = text.chars().collect();
  let at = at.min(characters.len());
  let from = characters[..at]
    .iter()
    .rposition(|character| *character == '\n')
    .map_or(0, |index| index + 1);
  let to = characters[at..]
    .iter()
    .position(|character| *character == '\n')
    .map_or(characters.len(), |index| at + index);

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

/// A window of `span` cells over `extent` of them, starting at `window` and settled
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

impl Field {
  /// The window's geometry at an inner width: the caret's position along it, the
  /// extent it covers and the span it shows—columns of the value plus the caret's for
  /// a field, wrapped rows for an editor.
  fn window_geometry(&self, inner: usize) -> (usize, usize, usize) {
    if self.lines <= 1 {
      return (self.caret, self.count() + 1, inner.max(1));
    }

    let (wrapped, row) = caret_line(&self.value, self.caret, inner.max(1));

    (row, wrapped.len(), usize::from(self.lines))
  }

  /// The window settled around the caret at an inner width.
  fn settled_window(&self, inner: usize) -> usize {
    let (at, extent, span) = self.window_geometry(inner);

    settled_window(self.window, at, extent, span)
  }

  /// How far the window can scroll from its start, at an inner width.
  pub(crate) fn window_limit(&self, inner: usize) -> usize {
    let (_, extent, span) = self.window_geometry(inner);

    extent.saturating_sub(span)
  }

  /// Settles the window around the caret at an inner width: held where the caret stays
  /// in view, else moved the least that shows it.
  pub(crate) fn settle(&mut self, inner: usize) {
    self.window = self.settled_window(inner);
    self.window_width = inner;
  }

  /// Scrolls the window by `delta`—rows of an editor, columns of a field—at an inner
  /// width, clamped to its extent and leaving the caret where it is; `false` when it
  /// cannot move that way.
  pub(crate) fn scroll_window(&mut self, inner: usize, delta: i64) -> bool {
    let limit = i64::try_from(self.window_limit(inner)).unwrap_or(i64::MAX);
    let window = i64::try_from(self.window).unwrap_or(i64::MAX).min(limit);
    let next = window.saturating_add(delta).clamp(0, limit);

    if next == window {
      return false;
    }

    self.window = usize::try_from(next).unwrap_or(0);
    self.window_width = inner;

    true
  }

  /// The rows the entry shows at an inner width, each with its start offset in the
  /// shown text—the placeholder while the value is empty—and the caret's row and
  /// column among them, `None` while the window has scrolled it out of view: one
  /// windowed row for a single-line entry, `lines` wrapped rows for an editor. A
  /// window settled at another width settles around the caret first.
  pub(crate) fn visible_rows(&self, inner: usize) -> VisibleRows {
    let inner = inner.max(1);
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
    else if self.window_width == inner {
      self.window.min(self.window_limit(inner))
    }
    else {
      self.settled_window(inner)
    };

    if self.lines <= 1 {
      let row: String = text.chars().skip(window).take(inner).collect();
      let caret = (window..window + inner)
        .contains(&self.caret)
        .then(|| (0, self.caret - window));

      return (vec![(window, row)], caret);
    }

    let lines = usize::from(self.lines);
    let (wrapped, row) = caret_line(text, self.caret, inner);
    let column = (self.caret - wrapped[row].0).min(inner);
    let rows = wrapped.into_iter().skip(window).take(lines).collect();
    let caret = (window..window + lines)
      .contains(&row)
      .then(|| (row - window, column));

    (rows, caret)
  }

  /// The caret position at the cell (`column`, `row`) of the value shown from `window`
  /// at an inner width, clamped into the visible rows and their ends.
  fn caret_in_window(&self, window: usize, inner: usize, column: usize, row: usize) -> usize {
    let inner = inner.max(1);
    let column = column.min(inner - 1);

    if self.lines <= 1 {
      return (window + column).min(self.count());
    }

    let lines = usize::from(self.lines);
    let wrapped = wrap_spans(&self.value, inner);
    let target = (window + row.min(lines - 1)).min(wrapped.len() - 1);
    let (start, line) = &wrapped[target];

    start + column.min(line.chars().count())
  }
}

/// An edit a key made to an entry.
enum Edit {
  Value,
  Caret,
}

/// Applies `event` to `field`: characters insert at the caret, replacing the selection;
/// `Backspace`/`Delete` remove the selection or one character around the caret;
/// arrows and `Home`/`End` move the caret, extending the selection under `Shift` and
/// collapsing it otherwise; `Ctrl` makes the arrows jump and the removals delete whole
/// words, `Ctrl+A` selects the value, and an editor's `Enter` breaks the line while
/// `ArrowUp`/`ArrowDown` move between lines.
// A match over the keys; length is the key count, not complexity.
#[expect(clippy::too_many_lines)]
fn apply(event: &KeyEvent, field: &mut Field, width: usize) -> Option<Edit> {
  let count = field.count();
  let ctrl = event.modifiers.ctrl;
  let alt = event.modifiers.alt;
  let shift = event.modifiers.shift;
  let key = event.key.as_str();
  let mut characters = key.chars();
  let single = characters.next().filter(|_| characters.next().is_none());
  let selection = field.selection();
  let caret = field.caret;

  match key {
    "a" | "A" if ctrl && !alt => {
      field.select_all();

      Some(Edit::Caret)
    }
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
    "Enter" if field.lines > 1 => {
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
      // Over a selection, an unshifted move starts from the end it leaves behind.
      let from = match (shift, selection) {
        (false, Some((start, end))) => {
          if key == "ArrowUp" {
            start
          }
          else {
            end
          }
        }
        _ => caret,
      };
      let (wrapped, row) = caret_line(&field.value, from, width.max(1));
      let column = from - wrapped[row].0;
      let target = if key == "ArrowUp" {
        row.checked_sub(1)?
      }
      else {
        (row + 1 < wrapped.len()).then_some(row + 1)?
      };
      let (start, line) = &wrapped[target];

      field.move_caret(start + column.min(line.chars().count()), shift);

      Some(Edit::Caret)
    }
    "Home" if caret > 0 || selection.is_some() => {
      field.move_caret(0, shift);

      Some(Edit::Caret)
    }
    "End" if caret < count || selection.is_some() => {
      field.move_caret(count, shift);

      Some(Edit::Caret)
    }
    _ => {
      let character = single.filter(|_| !ctrl && !alt)?;

      field.replace_selection(&character.to_string());

      Some(Edit::Value)
    }
  }
}

/// The text a paste inserts into an entry: an editor takes it whole, a single-line
/// entry drops its line breaks.
fn pasted_text(text: &str, lines: u16) -> String {
  if lines > 1 {
    text.to_owned()
  }
  else {
    text.replace(['\r', '\n'], "")
  }
}

impl Store {
  /// The width an entry shows its value in: inside its border and padding.
  pub(crate) fn field_inner_width(&self, handle: Handle) -> usize {
    let (width, _) = self.node_size(handle);
    let border = self.node_border(handle);
    let padding = self
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .and_then(|style| style.padding)
      .unwrap_or_default();

    usize::try_from(
      width - border.0 - border.2 - i64::from(padding.left) - i64::from(padding.right),
    )
    .unwrap_or(0)
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
      border.0 + i64::from(padding.left),
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
    let inner = self.field_inner_width(handle);
    let (column, row) = self.entry_cell(handle, local_x, local_y);
    let field = self.record(handle)?.kind.field()?;

    Some(field.caret_in_window(field.settled_window(inner), inner, column, row))
  }

  /// Whether the node is an editor, whose window scrolls down its wrapped rows.
  pub(crate) fn is_editor(&self, handle: Handle) -> bool {
    self
      .record(handle)
      .and_then(|record| record.kind.field())
      .is_some_and(|field| field.lines > 1)
  }

  /// An editor's window, clamped to its extent at its laid-out inner width; `None`
  /// for any other node.
  pub(crate) fn editor_window(&self, handle: Handle) -> Option<u32> {
    let inner = self.field_inner_width(handle);
    let field = self
      .record(handle)
      .and_then(|record| record.kind.field())
      .filter(|field| field.lines > 1)?;

    u32::try_from(field.window.min(field.window_limit(inner))).ok()
  }

  /// How far an editor's window can scroll at its laid-out inner width; `None` for
  /// any other node.
  pub(crate) fn editor_window_limit(&self, handle: Handle) -> Option<u32> {
    let inner = self.field_inner_width(handle);
    let field = self
      .record(handle)
      .and_then(|record| record.kind.field())
      .filter(|field| field.lines > 1)?;

    u32::try_from(field.window_limit(inner)).ok()
  }

  /// Moves an editor's window to `window`, clamped, at its laid-out inner width.
  pub(crate) fn set_editor_window(&mut self, handle: Handle, window: u32) {
    let inner = self.field_inner_width(handle);

    if let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
      .filter(|field| field.lines > 1)
    {
      field.window = usize::try_from(window)
        .unwrap_or(usize::MAX)
        .min(field.window_limit(inner));
      field.window_width = inner;
    }
  }

  /// Scrolls the single-line field at `handle` by `delta` columns at its laid-out
  /// inner width, damaging it: `None` when the node is no field or `delta` is zero,
  /// else whether it moved.
  pub(crate) fn scroll_field_by(&mut self, handle: Handle, delta: i64) -> Option<bool> {
    if delta == 0 {
      return None;
    }

    let inner = self.field_inner_width(handle);
    let field = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
      .filter(|field| field.lines <= 1)?;
    let moved = field.scroll_window(inner, delta);

    if moved {
      self.damage(handle);
    }

    Some(moved)
  }

  /// Settles an entry's window around its caret, at its laid-out inner width.
  pub(crate) fn settle_entry(&mut self, handle: Handle) {
    let inner = self.field_inner_width(handle);

    if let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
    {
      field.settle(inner);
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
    let inner = self.field_inner_width(handle);
    let (origin_x, origin_y) = self.screen_origin(handle);
    let (inset_x, inset_y) = self.field_inset(handle);
    let (x, y) = self.last_pointer;
    let (_, _, span) = field.window_geometry(inner);
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
      field.window < field.window_limit(inner)
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

    let inner = self.field_inner_width(handle);
    let (origin_x, origin_y) = self.screen_origin(handle);
    let (x, y) = self.last_pointer;
    let (column, row) = self.entry_cell(handle, i64::from(x) - origin_x, i64::from(y) - origin_y);
    let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
    else {
      return;
    };
    let limit = i64::try_from(field.window_limit(inner)).unwrap_or(i64::MAX);
    let window = i64::try_from(field.window).unwrap_or(i64::MAX);
    #[expect(clippy::cast_possible_truncation, reason = "at least one whole step")]
    let delta = overshoot.saturating_mul(steps as i64);
    let window = usize::try_from((window + delta).clamp(0, limit)).unwrap_or(0);
    let caret = field.caret_in_window(window, inner, column, row);

    field.window = window;
    field.move_caret(caret, true);
    field.settle(inner);
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
    edit: impl FnOnce(&mut Field, usize) -> Option<Edit>,
  ) -> Option<(skene_core::NodeId, String)> {
    let focused = self.focused?;

    self.edit_entry(focused, edit)
  }

  /// Applies `edit` to the entry at `entry`, returning the changed value with its node
  /// when the edit changed it; nothing happens when the entry is gone.
  fn edit_entry(
    &mut self,
    entry: Handle,
    edit: impl FnOnce(&mut Field, usize) -> Option<Edit>,
  ) -> Option<(skene_core::NodeId, String)> {
    let width = self.field_inner_width(entry);
    let record = self.record_mut(entry)?;
    let node = record.node;
    let field = record.kind.field_mut()?;
    let edit = edit(field, width)?;
    let value = field.value.clone();

    field.settle(width);
    self.damage(entry);

    match edit {
      Edit::Value => {
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
  let changed =
    with_store(|store| store.edit_focused_entry(|field, width| apply(event, field, width)));

  if let Some((node, value)) = changed {
    notify_change::<Tui>(node, &value);
  }
}

/// Inserts `text` into the focused entry in place of its selection, reporting the
/// changed value through the node's change handlers.
pub(crate) fn paste_focused(text: &str) {
  let target = with_store(|store| store.focused);

  paste_into(target, text);
}

/// Inserts `text`, the terminal's answer to the last clipboard read, into the entry
/// that asked for it, reporting the changed value through the node's change handlers;
/// nothing happens when no entry asked or it is gone.
pub(crate) fn paste_requested(text: &str) {
  let target = with_store(|store| store.paste_target.take());

  paste_into(target, text);
}

/// Inserts `text` into the entry at `target`, in place of its selection.
fn paste_into(target: Option<Handle>, text: &str) {
  let Some(target) = target
  else {
    return;
  };
  let changed = with_store(|store| {
    store.edit_entry(target, |field, _| {
      let text = pasted_text(text, field.lines);

      if text.is_empty() && field.selection().is_none() {
        return None;
      }

      field.replace_selection(&text);

      Some(Edit::Value)
    })
  });

  if let Some((node, value)) = changed {
    notify_change::<Tui>(node, &value);
  }
}

/// Stages the focused entry's selection for the clipboard and removes it, reporting
/// the changed value through the node's change handlers; nothing happens when the
/// launch offers no `Copy`.
pub(crate) fn cut_focused() {
  let changed = with_store(|store| {
    if !store.clipboard.copy {
      return None;
    }

    store.copy_selection();
    store.edit_focused_entry(|field, _| {
      let (from, to) = field.selection()?;

      field.remove(from, to);

      Some(Edit::Value)
    })
  });

  if let Some((node, value)) = changed {
    notify_change::<Tui>(node, &value);
  }
}
