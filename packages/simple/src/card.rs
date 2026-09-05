//! A ready-made card surface in the Simple design language.

use skene_core::{
  AnyKind,
  Children,
  Condition,
  Driver,
  FlexChild,
  HasChildren,
  ItemAlignment,
  NodeId,
  Seal,
  Spacing,
  Value,
  View,
};
use skene_views::{Column, Container, CornerRadius};

use super::{
  SimplePalette,
  shape::{CARD_RADIUS, SimpleUnit, define_defaults, hairline, shape},
};

/// A ready-made card: a raised, hairline-bordered, rounded surface grouping related
/// content, laid out as a column.
///
/// Its own properties shape the column; wrap it in a [`Container`] to change the
/// surface itself. Accepts any view or a [`Flexible`](skene_views::Flexible) wrapper as
/// a child.
#[must_use]
pub struct SimpleCard<D: Driver> {
  column: Column<D>,
}

impl<D: Driver> SimpleCard<D>
where
  D::Scalar: SimpleUnit,
{
  /// An empty card.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      column: Column::new().set_spacing(shape::<D>().card_spacing),
    }
  }
}

impl<D: Driver> SimpleCard<D> {
  /// Sets the spacing between the card's children.
  #[inline(always)]
  pub fn set_spacing(
    mut self,
    spacing: impl Into<Value<Spacing<D::Scalar>, Condition<D::Scalar>>>,
  ) -> Self {
    self.column = self.column.set_spacing(spacing);

    self
  }

  /// Derives the spacing between the card's children from the signals `spacing` reads.
  #[inline(always)]
  pub fn derive_spacing(mut self, spacing: impl Fn() -> Spacing<D::Scalar> + 'static) -> Self {
    self.column = self.column.derive_spacing(spacing);

    self
  }

  /// Sets the alignment of the card's children on the horizontal axis.
  #[inline(always)]
  pub fn set_item_alignment(
    mut self,
    alignment: impl Into<Value<ItemAlignment, Condition<D::Scalar>>>,
  ) -> Self {
    self.column = self.column.set_item_alignment(alignment);

    self
  }

  /// Derives the alignment of the card's children from the signals `alignment` reads.
  #[inline(always)]
  pub fn derive_item_alignment(mut self, alignment: impl Fn() -> ItemAlignment + 'static) -> Self {
    self.column = self.column.derive_item_alignment(alignment);

    self
  }
}

impl<D: Driver> Default for SimpleCard<D>
where
  D::Scalar: SimpleUnit,
{
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> HasChildren<D> for SimpleCard<D> {
  type Kind = FlexChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<D, FlexChild> {
    self.column.children_mut(seal)
  }
}

impl<D: Driver> AnyKind for SimpleCard<D> {}

impl<D: Driver> View<D> for SimpleCard<D>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<D>();

    Container::new()
      .set_padding(shape::<D>().card_inset)
      .set_color(SimplePalette::<D::Scalar>::CARD)
      .set_border(hairline(SimplePalette::<D::Scalar>::BORDER))
      .set_corner_radius(CARD_RADIUS)
      .set_child(self.column)
      .build()
  }
}
