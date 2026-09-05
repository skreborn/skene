//! The terminal's editing of a text entry: the caret, the visible window over the
//! value, and the keys that move and edit.

use skene_core::{KeyEvent, next::notify_change};

use crate::{
  Tui,
  paint::{field_window, wrap_spans},
  store::{Handle, Store},
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

/// The byte index of the character at `position`, or the end.
fn byte_of(value: &str, position: usize) -> usize {
  value
    .char_indices()
    .nth(position)
    .map_or(value.len(), |(index, _)| index)
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

/// The first visible row of a multi-line entry showing `lines` rows with the caret on
/// `row` in view.
const fn top_row(row: usize, lines: usize) -> usize {
  row.saturating_sub(lines.saturating_sub(1))
}

/// The rows an entry shows of `text` at an inner width, and the caret's row and column
/// within them: one windowed row for a single-line entry, `lines` wrapped rows for an
/// editor.
pub(crate) fn visible_rows(
  text: &str,
  caret: usize,
  inner: usize,
  lines: u16,
) -> (Vec<String>, (usize, usize)) {
  if lines <= 1 {
    let window = field_window(caret, inner);
    let row: String = text.chars().skip(window).take(inner).collect();

    return (vec![row], (0, caret - window));
  }

  let lines = usize::from(lines);
  let (wrapped, row) = caret_line(text, caret, inner.max(1));
  let top = top_row(row, lines);
  let column = caret - wrapped[row].0;
  let rows = wrapped
    .iter()
    .skip(top)
    .take(lines)
    .map(|(_, line)| line.clone())
    .collect();

  (rows, (row - top, column.min(inner)))
}

/// An edit a key made to an entry.
enum Edit {
  Value,
  Caret,
}

/// Applies `event` to `value` and `caret`: characters insert at the caret,
/// `Backspace`/`Delete` remove around it, arrows and `Home`/`End` move it, `Ctrl`
/// makes the arrows jump and the removals delete whole words, and an editor's `Enter`
/// breaks the line while `ArrowUp`/`ArrowDown` move between lines.
fn apply(
  event: &KeyEvent,
  value: &mut String,
  caret: &mut usize,
  lines: u16,
  width: usize,
) -> Option<Edit> {
  let count = value.chars().count();
  let ctrl = event.modifiers.ctrl;
  let alt = event.modifiers.alt;
  let key = event.key.as_str();
  let mut characters = key.chars();
  let single = characters.next().filter(|_| characters.next().is_none());

  match key {
    // `Ctrl+W` matches `Ctrl+Backspace`: legacy keyboard encodings cannot express the
    // latter, and the readline binding is muscle memory anyway.
    "Backspace" | "w" if ctrl && *caret > 0 => {
      let target = word_before(value, *caret);

      value.replace_range(byte_of(value, target)..byte_of(value, *caret), "");
      *caret = target;

      Some(Edit::Value)
    }
    "Delete" if ctrl && *caret < count => {
      value.replace_range(
        byte_of(value, *caret)..byte_of(value, word_after(value, *caret)),
        "",
      );

      Some(Edit::Value)
    }
    // Legacy encodings send `Ctrl+Backspace` as `0x08`—the byte of `Ctrl+H`—with no
    // way to tell the two apart; deleting one character honors both readings.
    "h" if ctrl && !alt && *caret > 0 => {
      *caret -= 1;
      value.remove(byte_of(value, *caret));

      Some(Edit::Value)
    }
    "Backspace" if !alt && *caret > 0 => {
      *caret -= 1;
      value.remove(byte_of(value, *caret));

      Some(Edit::Value)
    }
    "Delete" if *caret < count => {
      value.remove(byte_of(value, *caret));

      Some(Edit::Value)
    }
    "Enter" if lines > 1 => {
      value.insert(byte_of(value, *caret), '\n');
      *caret += 1;

      Some(Edit::Value)
    }
    "ArrowLeft" if ctrl && *caret > 0 => {
      *caret = word_before(value, *caret);

      Some(Edit::Caret)
    }
    "ArrowRight" if ctrl && *caret < count => {
      *caret = word_after(value, *caret);

      Some(Edit::Caret)
    }
    "ArrowLeft" if *caret > 0 => {
      *caret -= 1;

      Some(Edit::Caret)
    }
    "ArrowRight" if *caret < count => {
      *caret += 1;

      Some(Edit::Caret)
    }
    "ArrowUp" | "ArrowDown" if lines > 1 => {
      let (wrapped, row) = caret_line(value, *caret, width.max(1));
      let column = *caret - wrapped[row].0;
      let target = if key == "ArrowUp" {
        row.checked_sub(1)?
      }
      else {
        (row + 1 < wrapped.len()).then_some(row + 1)?
      };
      let (start, line) = &wrapped[target];

      *caret = start + column.min(line.chars().count());

      Some(Edit::Caret)
    }
    "Home" if *caret > 0 => {
      *caret = 0;

      Some(Edit::Caret)
    }
    "End" if *caret < count => {
      *caret = count;

      Some(Edit::Caret)
    }
    _ => {
      let character = single.filter(|_| !ctrl && !alt)?;

      value.insert(byte_of(value, *caret), character);
      *caret += 1;

      Some(Edit::Value)
    }
  }
}

impl Store {
  /// The width an entry shows its value in: inside its border and padding.
  fn field_inner_width(&self, handle: Handle) -> usize {
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

  /// Places an entry's caret at a press's local cell, mapped through the inset and the
  /// value's visible window.
  pub(crate) fn place_caret(&mut self, handle: Handle, local_x: i64, local_y: i64) {
    let inner = self.field_inner_width(handle);
    let (inset_x, inset_y) = self.field_inset(handle);
    let column = usize::try_from(local_x - inset_x).unwrap_or(0);
    let row = usize::try_from(local_y - inset_y).unwrap_or(0);
    let Some(field) = self
      .record_mut(handle)
      .and_then(|record| record.kind.field_mut())
    else {
      return;
    };

    if field.lines <= 1 {
      let window = field_window(field.caret, inner);

      field.caret = (window + column).min(field.value.chars().count());
    }
    else {
      let (wrapped, current) = caret_line(&field.value, field.caret, inner.max(1));
      let top = top_row(current, usize::from(field.lines));
      let target = (top + row).min(wrapped.len() - 1);
      let (start, line) = &wrapped[target];

      field.caret = start + column.min(line.chars().count());
    }

    self.damage(handle);
  }
}

/// Applies an editing or caret key to the focused entry, reporting a changed value
/// through the node's change handlers.
pub(crate) fn edit_focused(event: &KeyEvent) {
  let changed = with_store(|store| {
    let focused = store.focused?;
    let width = store.field_inner_width(focused);
    let record = store.record_mut(focused)?;
    let node = record.node;
    let field = record.kind.field_mut()?;
    let edit = apply(
      event,
      &mut field.value,
      &mut field.caret,
      field.lines,
      width,
    )?;
    let value = field.value.clone();

    store.damage(focused);

    match edit {
      Edit::Value => {
        store.layout_dirty = true;

        Some((node?, value))
      }
      Edit::Caret => None,
    }
  });

  if let Some((node, value)) = changed {
    notify_change::<Tui>(node, &value);
  }
}
