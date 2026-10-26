//! A ready-made checkbox in the Simple design language.

use skene_foundation::{Alignment, Dimension};
use skene_web::{
  Checkbox,
  Container,
  ContainerBase,
  ControlBase,
  DecorationBase,
  DecorationWithCornerRadius,
  HasChild,
  Inset,
  Part,
  RunBase,
  State,
  Text,
  TextBase,
  When,
};

use crate::{SimplePalette, shape};

/// A ready-made checkbox, a preset of [`Checkbox`]: a drawn mark beside its caption.
///
/// Its mark is a rounded box on the card surface that fills with the primary surface
/// while checked, its check showing then, and rings while focus shows.
/// [`new`](Self::new) returns the checkbox, so every member of a checkbox applies, its
/// checked state flowing both ways; [`styled`](Self::styled) dresses a checkbox built
/// otherwise, and [`mark`](Self::mark) is the mark it shows, its
/// [`Part::Mark`](skene_web::Part::Mark), which follows the control's checked state.
/// Its colors are [`SimplePalette`]'s tokens, inked in the palette's default unless a
/// [`Theme`](skene_web::Theme) of the palette above redefines them.
///
/// ```
/// use skene_simple_web::prelude::*;
/// use skene_web::prelude::*;
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

  /// The mark: a rounded box on the card surface that fills with the primary surface
  /// while the control is checked and rings while focus shows, its check inked in the
  /// primary ink and hidden until checked.
  pub fn mark() -> Container {
    let shape = shape();

    Container::new()
      .set_width(Dimension::Length(shape.mark_size))
      .set_height(Dimension::Length(shape.mark_size))
      .set_background(
        shape
          .mark_surface
          .when(State::Checked, SimplePalette::PRIMARY_SURFACE),
      )
      .set_border_width(Inset::all(shape.mark_ring))
      .set_border_color(
        SimplePalette::BORDER
          .when(State::Checked, SimplePalette::PRIMARY_SURFACE)
          .when(State::FocusVisible, SimplePalette::RING),
      )
      .set_corner_radius(shape.mark_radius)
      .set_alignment(Alignment::CENTER)
      .set_child(
        Container::new()
          .set_hidden(true.when(State::Checked, false))
          .set_child(Text::new("\u{2713}").set_color(SimplePalette::PRIMARY_INK)),
      )
  }
}
