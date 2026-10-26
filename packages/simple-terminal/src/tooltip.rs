//! A ready-made tooltip in the Simple design language.

use skene_terminal::{DecorationBase, DecorationWithCornerRadius, Tooltip, TooltipBase, View};

use crate::{SimplePalette, shape};

/// A ready-made tooltip, a preset of [`Tooltip`]: a bubble in the foreground color
/// carrying its text in the background color.
///
/// The bubble sits above the view it describes, opening after the standard delay on
/// hover and at once on focus. [`new`](Self::new) shows text, [`derive`](Self::derive)
/// text that follows a signal, and [`view`](Self::view) any view; each returns the
/// tooltip, so every member of a tooltip applies, and the described view comes last
/// through its `set_child`, so any view takes a tooltip by wrapping.
/// [`styled`](Self::styled) dresses a tooltip built otherwise. Its colors are
/// [`SimplePalette`]'s tokens, inked in the palette's default unless a
/// [`Theme`](skene_terminal::Theme) of the palette above redefines them.
///
/// ```
/// use skene_simple_terminal::prelude::*;
/// use skene_terminal::prelude::*;
///
/// let save = SimpleTooltip::new("Saves the document").set_child(SimpleButton::new("Save"));
/// ```
#[derive(Debug)]
pub enum SimpleTooltip {}

impl SimpleTooltip {
  /// A tooltip showing `text`.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new(text: impl Into<String>) -> Tooltip {
    Self::styled(Tooltip::new(text))
  }

  /// A tooltip whose text is recomputed whenever a signal `text` reads changes.
  pub fn derive(text: impl Fn() -> String + 'static) -> Tooltip {
    Self::styled(Tooltip::derive(text))
  }

  /// A tooltip showing `content`, a view of any kind.
  pub fn view(content: impl View) -> Tooltip {
    Self::styled(Tooltip::view(content))
  }

  /// `tooltip`'s bubble in the foreground color, its text in the background color,
  /// padded and rounded.
  pub fn styled(tooltip: Tooltip) -> Tooltip {
    let shape = shape();

    tooltip
      .set_background(SimplePalette::FOREGROUND)
      .set_text_color(SimplePalette::BACKGROUND)
      .set_padding(shape.tooltip_inset)
      .set_corner_radius(shape.mark_radius)
  }
}
