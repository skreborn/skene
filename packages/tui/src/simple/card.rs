//! A ready-made card surface in the Simple design language, in cells.

use skene_core::{HasChildren, ItemAlignment, NodeId, Source, View};

use crate::{Children, Column, HasDecoration, simple, simple::SimplePalette};

/// A ready-made card: a raised, hairline-bordered surface grouping related content,
/// laid out as a column.
#[must_use]
pub struct SimpleCard {
  spacing: Source<u32>,
  item_alignment: Option<Source<ItemAlignment>>,
  children: Children,
}

impl SimpleCard {
  /// Creates an empty card.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      spacing: Source::Static(1),
      item_alignment: None,
      children: Children::default(),
    }
  }

  /// Sets the spacing between the card's children, in cells.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: u32) -> Self {
    self.spacing = Source::Static(spacing);

    self
  }

  /// Sets a derived spacing, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_spacing(mut self, spacing: impl Fn() -> u32 + 'static) -> Self {
    self.spacing = Source::Derived(std::rc::Rc::new(spacing));

    self
  }

  /// Sets the alignment of the card's children on the horizontal axis.
  #[inline(always)]
  pub fn set_item_alignment(mut self, item_alignment: ItemAlignment) -> Self {
    self.item_alignment = Some(Source::Static(item_alignment));

    self
  }

  /// Sets a derived alignment, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_item_alignment(
    mut self,
    item_alignment: impl Fn() -> ItemAlignment + 'static,
  ) -> Self {
    self.item_alignment = Some(Source::Derived(std::rc::Rc::new(item_alignment)));

    self
  }
}

impl Default for SimpleCard {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl HasChildren<crate::TuiOps> for SimpleCard {
  type Kind = skene_core::PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl View for SimpleCard {
  fn build(self) -> NodeId {
    let mut card = Column::new()
      .set_padding(crate::Inset::symmetric(2, 1))
      .set_color(SimplePalette::card())
      .set_border(SimplePalette::border())
      .set_border_glyphs(simple::GLYPHS);

    card = match self.spacing {
      Source::Static(spacing) => card.set_spacing(spacing),
      Source::Derived(spacing) => card.derive_spacing(move || spacing()),
    };

    card = match self.item_alignment {
      Some(Source::Static(item_alignment)) => card.set_item_alignment(item_alignment),
      Some(Source::Derived(item_alignment)) => card.derive_item_alignment(move || item_alignment()),
      None => card,
    };

    card.set_children_to(self.children).build()
  }
}
