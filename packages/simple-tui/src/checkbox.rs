//! A ready-made checkbox in the Simple design language.

use skene_tui::{Checkbox, ControlBase, Part, Stack};

use crate::{SimpleGlyphs, glyph_mark};

/// A ready-made checkbox, a preset of [`Checkbox`]: a glyph mark beside its caption.
///
/// Its mark is the glyphs of [`SimpleGlyphs`], `[x]` while checked and `[ ]` otherwise
/// unless a theme redefines them, inked in the foreground and the ring while focus
/// shows. [`new`](Self::new) returns the checkbox, so every member of a checkbox
/// applies, its checked state flowing both ways; [`styled`](Self::styled) dresses a
/// checkbox built otherwise, and [`mark`](Self::mark) is the mark it shows, its
/// [`Part::Mark`](skene_tui::Part::Mark), which follows the control's checked
/// state. Its colors are [`SimplePalette`](crate::SimplePalette)'s tokens, inked in the
/// palette's default unless a [`Theme`](skene_tui::Theme) of the palette above
/// redefines them.
///
/// ```
/// use skene_simple_tui::prelude::*;
/// use skene_tui::prelude::*;
///
/// let agreed = Signal::new(false);
/// let terms = SimpleCheckbox::new()
///   .set_label("I agree")
///   .bind_checked(agreed);
/// ```
#[derive(Debug)]
pub enum SimpleCheckbox {}

impl SimpleCheckbox {
  /// An unchecked checkbox.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new() -> Checkbox {
    Self::styled(Checkbox::new())
  }

  /// `checkbox` showing the Simple mark.
  pub fn styled(checkbox: Checkbox) -> Checkbox {
    checkbox.set_part(Part::Mark, Self::mark())
  }

  /// The mark: the checked glyph while the control is checked and the unchecked one
  /// otherwise, in one place.
  pub fn mark() -> Stack {
    glyph_mark(SimpleGlyphs::checkbox)
  }
}
