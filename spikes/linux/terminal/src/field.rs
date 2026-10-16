//! The terminal's text field: the shared editing core over the cell engine, its window
//! following the caret, its keys and pointer, and its cells.

use skene_foundation::{
  behavior::{Behavior as _, EntryAct, TextEntry},
  seam::report::{KeyEvent, Modifiers},
  text::{
    Laid as _, ParagraphStyle, RunStyle, Wrap,
    editing::{Binding, ClauseStyle, Command, Editor},
  },
  value::TextDecorationStyle,
};
use unicode_segmentation::UnicodeSegmentation as _;
use unicode_width::UnicodeWidthChar as _;

use crate::{
  cells::Cells,
  frame::{Cell, Frame},
};

/// What a key did to a field.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum KeyOutcome {
  /// The field acted on it: it goes no further.
  Handled,
  /// The field left it to its ancestors.
  Ignored,
  /// A paste: the caller reads the clipboard and hands its text to [`TextField::paste`].
  Paste,
  /// `Enter` in a field of one row: the field is submitted.
  Activate,
}

/// A window of `span` over `extent`, starting at `window`, settled around `at`: held where
/// `at` stays in view, moved the least that brings it in, never past the extent's end.
#[must_use]
pub const fn settled_window(window: usize, at: usize, extent: usize, span: usize) -> usize {
  let last = extent.saturating_sub(span);
  let window = if window > last { last } else { window };

  if at < window {
    at
  } else if at + 1 > window + span {
    at + 1 - span
  } else {
    window
  }
}

/// How many cells a cluster takes: its characters' widths, a control character one blank.
fn cluster_cells(cluster: &str) -> usize {
  cluster
    .chars()
    .map(|character| {
      if character.is_control() {
        1
      } else {
        character.width().unwrap_or(0)
      }
    })
    .sum()
}

/// A text entry in the terminal: a field of one row scrolling sideways, or an editor of
/// several rows wrapping its lines and scrolling down. Its editing is the shared core's; its
/// cells are the cell engine's layout of the text.
pub struct TextField {
  behavior: TextEntry,
  editor: Editor<Cells>,
  columns: u16,
  rows: u16,
  window: usize,
  placeholder: Option<String>,
  focused: bool,
  copied: Option<String>,
}

impl TextField {
  /// A field of one row, `columns` wide.
  #[must_use]
  pub fn new(columns: u16) -> Self {
    let mut editor = Editor::new(
      Cells::new(),
      RunStyle::new(1.),
      ParagraphStyle::default().with_wrap(Wrap::None),
    );

    editor.set_width(None);

    Self {
      behavior: TextEntry::new(false),
      editor,
      columns: columns.max(1),
      rows: 1,
      window: 0,
      placeholder: None,
      focused: false,
      copied: None,
    }
  }

  /// An editor of `rows` rows, `columns` wide, its lines wrapped at its width.
  #[must_use]
  pub fn multiline(columns: u16, rows: u16) -> Self {
    let mut editor = Editor::new(Cells::new(), RunStyle::new(1.), ParagraphStyle::default());

    editor.set_multiline(true);
    editor.set_width(Some(f32::from(columns.max(1))));

    Self {
      behavior: TextEntry::new(true),
      editor,
      columns: columns.max(1),
      rows: rows.max(1),
      window: 0,
      placeholder: None,
      focused: false,
      copied: None,
    }
  }

  /// The editing core.
  #[must_use]
  pub const fn editor(&self) -> &Editor<Cells> {
    &self.editor
  }

  /// The editing core, to write the app's truth into it or adapt an input method to it.
  pub const fn editor_mut(&mut self) -> &mut Editor<Cells> {
    &mut self.editor
  }

  /// Makes the text read only where `read_only`: the selection still moves and copies.
  pub const fn set_read_only(&mut self, read_only: bool) {
    self.behavior = self.behavior.with_read_only(read_only);
    self.editor.set_read_only(read_only);
  }

  /// Makes the field secure where `secure`: it offers neither copy nor cut.
  pub const fn set_secure(&mut self, secure: bool) {
    self.behavior = self.behavior.with_secure(secure);
  }

  /// Sets the text shown while the value is empty.
  pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
    self.placeholder = Some(placeholder.into());
  }

  /// Focus enters the field: by keyboard, its whole value is selected, as an `<input>` is.
  pub fn focus(&mut self, by_keyboard: bool) {
    self.focused = true;

    if by_keyboard {
      let _ = self.editor.perform(Command::SelectAll);
    }
  }

  /// Focus leaves the field.
  pub const fn blur(&mut self) {
    self.focused = false;
  }

  /// Whether the field has focus.
  #[must_use]
  pub const fn is_focused(&self) -> bool {
    self.focused
  }

  /// The text the last copy or cut put on the clipboard, taken.
  pub const fn take_copied(&mut self) -> Option<String> {
    self.copied.take()
  }

  /// A key press, answered by the text entry's behavior ([`TextEntry`]). The terminal's legacy
  /// encodings stand for the keys they encode: `Ctrl+W` for `Ctrl+Backspace`, which some
  /// keyboard encodings cannot tell, and `Ctrl+H` (`0x08`) for `Backspace`.
  pub fn key(&mut self, event: &KeyEvent) -> KeyOutcome {
    let modifiers = event.modifiers;
    let control = modifiers.control && !modifiers.alt && !modifiers.meta && !modifiers.shift;
    let legacy = match event.key.as_str() {
      "w" if control => Some(true),
      "h" if control => Some(false),
      _ => None,
    };
    let encoded;
    let event = match legacy {
      Some(word) => {
        encoded = KeyEvent {
          key: String::from("Backspace"),
          modifiers: Modifiers {
            control: word,
            ..modifiers
          },
          ..event.clone()
        };

        &encoded
      }
      None => event,
    };
    let response = self.behavior.key(event);

    match response.act {
      Some(EntryAct::Activate) => KeyOutcome::Activate,
      Some(EntryAct::Edit(Binding::Paste)) => KeyOutcome::Paste,
      Some(EntryAct::Edit(Binding::Command(command))) => {
        let outcome = self.editor.perform(command);

        if let Some(copied) = outcome.clipboard {
          self.copied = Some(copied);
        }

        if outcome.handled {
          KeyOutcome::Handled
        } else {
          KeyOutcome::Ignored
        }
      }
      None if response.consumed => KeyOutcome::Handled,
      None => KeyOutcome::Ignored,
    }
  }

  /// Pastes `text` over the selection.
  pub fn paste(&mut self, text: &str) {
    let _ = self.editor.perform(Command::Paste(text.to_owned()));
  }

  /// The layout position of a cell's center, a hair to its left: a press on a cell's center
  /// lands before the cluster it starts, a press on a wide cluster's second cell after it.
  fn point(&self, column: u16, row: u16) -> (f32, f32) {
    #[expect(clippy::cast_precision_loss, reason = "a window in cells")]
    let window = self.window as f32;

    if self.rows > 1 {
      (f32::from(column) + 0.49, f32::from(row) + window + 0.5)
    } else {
      (f32::from(column) + window + 0.49, f32::from(row) + 0.5)
    }
  }

  /// A press at `column`, `row` of the field, the `clicks`th in a row (a double press selects
  /// the word, a triple the value or the line), extending the selection where `extend`.
  pub fn press(&mut self, column: u16, row: u16, clicks: u8, extend: bool) {
    let (x, y) = self.point(column, row);

    self.focused = true;
    self.editor.press(x, y, clicks, extend);
  }

  /// A drag to `column`, `row` after a press.
  pub fn drag(&mut self, column: u16, row: u16) {
    let (x, y) = self.point(column, row);

    self.editor.drag(x, y);
  }

  /// The first visible column of a field, or row of an editor.
  #[must_use]
  pub const fn window(&self) -> usize {
    self.window
  }

  /// The field's cells: its window settled around the caret first.
  pub fn render(&mut self) -> Frame {
    let laid = self.editor.laid();
    let caret = self.editor.caret_rect();
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "cells"
    )]
    let (caret_column, caret_row) = (
      caret.x.round().max(0.) as usize,
      caret.y.round().max(0.) as usize,
    );
    let lines = laid.lines();

    if self.rows > 1 {
      self.window = settled_window(self.window, caret_row, lines.len(), usize::from(self.rows));
    } else {
      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "cells"
      )]
      let extent = laid.size().width.ceil() as usize + 1;

      self.window = settled_window(self.window, caret_column, extent, usize::from(self.columns));

      // A window starting inside a wide cluster starts after it.
      for run in laid.glyph_runs() {
        for glyph in &run.glyphs {
          let end = self.editor.index().next_grapheme(glyph.at);
          #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "cells"
          )]
          let start = glyph.x.round().max(0.) as usize;
          let cells = cluster_cells(&self.editor.text()[glyph.at..end]);

          if start < self.window && self.window < start + cells {
            self.window = start + cells;
          }
        }
      }
    }

    let (column_shift, row_shift) = if self.rows > 1 {
      (0, self.window)
    } else {
      (self.window, 0)
    };
    let mut frame = Frame::new(self.columns, self.rows);
    let text = self.editor.text();
    let selected = self.editor.selected();
    let composition = self.editor.composition();
    let place = |column: usize, row: usize| -> Option<(u16, u16)> {
      let column = u16::try_from(column.checked_sub(column_shift)?).ok()?;
      let row = u16::try_from(row.checked_sub(row_shift)?).ok()?;

      (column < self.columns && row < self.rows).then_some((column, row))
    };

    if text.is_empty()
      && let Some(placeholder) = &self.placeholder
    {
      let mut column = 0;

      for cluster in placeholder.graphemes(true) {
        if let Some(cell) = frame.cell_mut(column, 0) {
          cell.text = cluster.to_owned();
          cell.style.placeholder = true;
        }

        column += u16::try_from(cluster_cells(cluster)).unwrap_or(1);
      }
    }

    let mut drawn = Vec::new();

    for run in laid.glyph_runs() {
      for glyph in &run.glyphs {
        if drawn.contains(&glyph.at) {
          continue;
        }

        drawn.push(glyph.at);

        let end = self.editor.index().next_grapheme(glyph.at);
        let cluster = &text[glyph.at..end];

        if cluster == "\n" || cluster == "\r\n" {
          continue;
        }

        #[expect(
          clippy::cast_possible_truncation,
          clippy::cast_sign_loss,
          reason = "cells"
        )]
        let (column, row) = (
          glyph.x.round().max(0.) as usize,
          (glyph.y - 1.).round().max(0.) as usize,
        );
        let cells = cluster_cells(cluster);
        let underline = composition.and_then(|marked| {
          if !marked.range.contains(&glyph.at) {
            return None;
          }

          let style = marked
            .clauses
            .iter()
            .find(|clause| clause.range.contains(&glyph.at))
            .map_or(ClauseStyle::Raw, |clause| clause.style);

          Some(match style {
            ClauseStyle::Raw => TextDecorationStyle::Dotted,
            ClauseStyle::Converted => TextDecorationStyle::Solid,
            ClauseStyle::Target => TextDecorationStyle::Double,
          })
        });

        for offset in 0..cells.max(1) {
          let Some((column, row)) = place(column + offset, row) else {
            continue;
          };
          let Some(cell) = frame.cell_mut(column, row) else {
            continue;
          };

          *cell = Cell {
            text: if offset > 0 {
              String::new()
            } else if cluster.chars().any(char::is_control) {
              " ".to_owned()
            } else {
              cluster.to_owned()
            },
            style: crate::frame::CellStyle {
              selected: selected.contains(&glyph.at),
              underline,
              placeholder: false,
            },
          };
        }
      }
    }

    if self.focused {
      frame.cursor = place(caret_column, caret_row);
    }

    frame
  }

  /// The cell the caret stands in, where it shows.
  pub fn caret_cell(&mut self) -> Option<(u16, u16)> {
    self.render().cursor
  }
}
