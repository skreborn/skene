//! A ready-made card surface in the Simple design language.

use skene_core::{Border, HasChildren, ItemAlignment, View};

use crate::{Children, Column, HasDecoration, Inset, SimplePalette, StyledBox, simple};

/// A ready-made card: a raised, hairline-bordered surface grouping related content,
/// laid out as a column.
///
/// The default look—padding, the card fill, the hairline border, the corner radius,
/// 16 pixels of spacing—is applied up front, so the whole surface remains open: any
/// decoration, sizing, or event setter overrides the default it targets.
#[must_use]
pub struct SimpleCard {
  base: Column,
}

impl SimpleCard {
  /// Creates an empty card.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: Column::new()
        .set_spacing(16.)
        .set_padding(Inset::all(24.))
        .set_color(SimplePalette::card())
        .set_border(Border::new(1., SimplePalette::border()))
        .set_border_radius(simple::CARD_RADIUS),
    }
  }

  /// Sets the spacing between the card's children, in logical pixels.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: f32) -> Self {
    self.base = self.base.set_spacing(spacing);

    self
  }

  /// Sets a derived spacing, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_spacing(mut self, spacing: impl Fn() -> f32 + 'static) -> Self {
    self.base = self.base.derive_spacing(spacing);

    self
  }

  /// Sets the alignment of the card's children on the horizontal axis.
  #[inline(always)]
  pub fn set_item_alignment(mut self, item_alignment: ItemAlignment) -> Self {
    self.base = self.base.set_item_alignment(item_alignment);

    self
  }

  /// Sets a derived alignment, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_item_alignment(
    mut self,
    item_alignment: impl Fn() -> ItemAlignment + 'static,
  ) -> Self {
    self.base = self.base.derive_item_alignment(item_alignment);

    self
  }
}

impl Default for SimpleCard {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl crate::HasSize for SimpleCard {
  #[inline(always)]
  fn size_box(&mut self) -> &mut StyledBox {
    self.base.size_box()
  }
}

impl HasDecoration for SimpleCard {
  #[inline(always)]
  fn decoration_box(&mut self) -> &mut StyledBox {
    self.base.decoration_box()
  }
}

impl crate::HasEvents for SimpleCard {
  #[inline(always)]
  fn events_box(&mut self) -> &mut StyledBox {
    self.base.events_box()
  }
}

impl HasChildren<crate::GuiOps> for SimpleCard {
  type Kind = skene_core::PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.base = self.base.set_children_to(children);

    self
  }
}

impl View for SimpleCard {
  #[inline(always)]
  fn build(self) -> skene_core::NodeId {
    self.base.build()
  }
}
