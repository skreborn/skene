//! A ready-made card surface in the Simple design language.

use skene_core::{HasChildren, ItemAlignment, NodeId, PlainChild, View};

use crate::{Children, Column, Container, Inset, ThemedBox, simple};

/// A ready-made card: a raised, hairline-bordered surface grouping related content,
/// laid out as a column.
///
/// Its own knobs shape the column; wrap it in a [`Container`] to change the surface
/// itself.
#[must_use]
pub struct SimpleCard {
  column: Column,
}

impl SimpleCard {
  /// Creates an empty card.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      column: Column::new().set_spacing(16.),
    }
  }

  /// Sets the spacing between the card's children, in logical pixels.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: f32) -> Self {
    self.column = self.column.set_spacing(spacing);

    self
  }

  /// Sets a derived spacing, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_spacing(mut self, spacing: impl Fn() -> f32 + 'static) -> Self {
    self.column = self.column.derive_spacing(spacing);

    self
  }

  /// Sets the alignment of the card's children on the horizontal axis.
  #[inline(always)]
  pub fn set_item_alignment(mut self, item_alignment: ItemAlignment) -> Self {
    self.column = self.column.set_item_alignment(item_alignment);

    self
  }

  /// Sets a derived alignment, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_item_alignment(
    mut self,
    item_alignment: impl Fn() -> ItemAlignment + 'static,
  ) -> Self {
    self.column = self.column.derive_item_alignment(item_alignment);

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
  type Kind = PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.column = self.column.set_children_to(children);

    self
  }
}

impl View for SimpleCard {
  #[inline(always)]
  fn build(self) -> NodeId {
    Container::new()
      .set_padding(Inset::all(24.))
      .theme_color(|palette| palette.card)
      .theme_border(1., |palette| palette.border)
      .set_border_radius(simple::CARD_RADIUS)
      .set_child(self.column)
      .build()
  }
}
