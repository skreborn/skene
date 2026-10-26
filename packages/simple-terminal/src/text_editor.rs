//! A ready-made multi-line text editor in the Simple design language.

use skene_terminal::TextEditor;

use crate::field_chrome;

/// A ready-made multi-line text editor, a preset of [`TextEditor`]: the field chrome of
/// a [`SimpleTextField`](crate::SimpleTextField) over several lines at once, three
/// unless its `set_lines` says otherwise.
///
/// [`new`](Self::new) returns the editor, so every member of a text editor applies, its
/// value flowing both ways; [`styled`](Self::styled) dresses an editor built otherwise.
/// Its colors are [`SimplePalette`](crate::SimplePalette)'s tokens, inked in the
/// palette's default unless a [`Theme`](skene_terminal::Theme) of the palette above
/// redefines them.
///
/// ```
/// use skene_simple_terminal::prelude::*;
/// use skene_terminal::prelude::*;
///
/// let notes = Signal::new(String::new());
/// let editor = SimpleTextEditor::new()
///   .set_lines(4)
///   .set_placeholder("Notes")
///   .bind_value(notes);
/// ```
#[derive(Debug)]
pub enum SimpleTextEditor {}

impl SimpleTextEditor {
  /// An empty editor showing three lines.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new() -> TextEditor {
    Self::styled(TextEditor::new())
  }

  /// `editor` in the Simple field chrome, as [`SimpleTextField::styled`] dresses a
  /// field.
  ///
  /// [`SimpleTextField::styled`]: crate::SimpleTextField::styled
  pub fn styled(editor: TextEditor) -> TextEditor {
    field_chrome(editor)
  }
}
