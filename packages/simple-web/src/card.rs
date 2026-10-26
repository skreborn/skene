//! A ready-made card surface in the Simple design language.

use skene_web::{Container, ContainerBase, DecorationBase, DecorationWithCornerRadius, Inset};

use crate::{SimplePalette, shape};

/// A ready-made card: a raised, hairline-bordered, rounded surface grouping related
/// content, a preset of [`Container`].
///
/// [`new`](Self::new) returns the container, so every member of a container applies; a
/// card holds exactly one child, a [`Column`](skene_web::Column) spaced
/// [`spacing`](Self::spacing) apart where it groups several. [`styled`](Self::styled)
/// dresses a container built otherwise. Its colors are [`SimplePalette`]'s tokens,
/// inked in the palette's default unless a [`Theme`](skene_web::Theme) of the palette
/// above redefines them.
///
/// ```
/// use skene_simple_web::prelude::*;
/// use skene_web::prelude::*;
///
/// let card = SimpleCard::new().set_child(
///   Column::new()
///     .set_spacing(SimpleCard::spacing())
///     .set_children((Text::new("Title"), Text::new("Body"))),
/// );
/// ```
#[derive(Debug)]
pub enum SimpleCard {}

impl SimpleCard {
  /// An empty card.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new() -> Container {
    Self::styled(Container::new())
  }

  /// `container` on the card surface, inside a hairline border, rounded and padded.
  pub fn styled(container: Container) -> Container {
    let shape = shape();

    container
      .set_padding(shape.card_inset)
      .set_background(SimplePalette::CARD)
      .set_border_width(Inset::all(1))
      .set_border_color(SimplePalette::BORDER)
      .set_corner_radius(shape.card_radius)
  }

  /// The spacing between the children a card groups, in logical pixels.
  #[must_use]
  pub fn spacing() -> f32 {
    shape().card_spacing
  }
}
