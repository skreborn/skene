//! A ready-made card surface in the Simple design language, in cells.

use skene_core::{HasChildren, ItemAlignment, NodeId, View};

use crate::{Children, Column, HasDecoration, StyledBox, simple, simple::SimplePalette};

/// A ready-made card: a raised, hairline-bordered surface grouping related content,
/// laid out as a column.
///
/// The default look—padding, the card fill, the hairline border, one cell of
/// spacing—is applied up front, so the whole surface remains open: any decoration,
/// sizing, or event setter overrides the default it targets.
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
        .set_spacing(1)
        .set_padding(crate::Inset::symmetric(2, 1))
        .set_color(SimplePalette::card())
        .set_border(SimplePalette::border())
        .set_border_glyphs(simple::GLYPHS),
    }
  }

  /// Sets the spacing between the card's children, in cells.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: u32) -> Self {
    self.base = self.base.set_spacing(spacing);

    self
  }

  /// Sets a derived spacing, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_spacing(mut self, spacing: impl Fn() -> u32 + 'static) -> Self {
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

impl HasChildren<crate::TuiOps> for SimpleCard {
  type Kind = skene_core::PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.base = self.base.set_children_to(children);

    self
  }
}

impl View for SimpleCard {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.base.build()
  }
}
