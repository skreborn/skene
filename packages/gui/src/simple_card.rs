//! A ready-made card surface in the Simple design language.

use skene_core::{Border, HasChildren, ItemAlignment, View};

use crate::{Children, Column, Inset, SimplePalette, simple};

/// A ready-made card: a raised, hairline-bordered surface grouping related content,
/// laid out as a column.
#[must_use]
pub struct SimpleCard {
  spacing: f32,
  item_alignment: Option<ItemAlignment>,
  children: Children,
}

impl SimpleCard {
  /// Creates an empty card.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      spacing: 16.,
      item_alignment: None,
      children: Children::default(),
    }
  }

  /// Sets the spacing between the card's children, in logical pixels.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: f32) -> Self {
    self.spacing = spacing;

    self
  }

  /// Sets the alignment of the card's children on the horizontal axis.
  #[inline(always)]
  pub fn set_item_alignment(mut self, item_alignment: ItemAlignment) -> Self {
    self.item_alignment = Some(item_alignment);

    self
  }
}

impl Default for SimpleCard {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl HasChildren<crate::GuiOps> for SimpleCard {
  type Kind = skene_core::PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl View for SimpleCard {
  fn build(self) -> skene_core::NodeId {
    let mut card = Column::new()
      .set_spacing(self.spacing)
      .set_padding(Inset::all(24.))
      .set_color(SimplePalette::card())
      .set_border(Border::new(1., SimplePalette::border()))
      .set_border_radius(simple::CARD_RADIUS);

    if let Some(item_alignment) = self.item_alignment {
      card = card.set_item_alignment(item_alignment);
    }

    card.set_children_to(self.children).build()
  }
}
