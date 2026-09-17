//! A ready-made card surface in the Simple design language.

use skene_core::{
  AnyKind,
  Children,
  Condition,
  FlexChild,
  HasChildren,
  ItemAlignment,
  NodeId,
  Platform,
  Seal,
  Spacing,
  StaticLeaf,
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
pub struct SimpleCard<P: Platform> {
  column: Column<P>,
}

impl<P: Platform> SimpleCard<P>
where
  P::Scalar: SimpleUnit,
{
  /// An empty card.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      column: Column::new().set_spacing(shape::<P>().card_spacing),
    }
  }
}

impl<P: Platform> SimpleCard<P> {
  /// Sets the spacing between the card's children.
  #[inline(always)]
  pub fn set_spacing(
    mut self,
    spacing: impl Into<Value<Spacing<P::Scalar>, Condition<P::Scalar>>>,
  ) -> Self {
    self.column = self.column.set_spacing(spacing);

    self
  }

  /// Derives the spacing between the card's children from the signals `spacing` reads.
  #[inline(always)]
  pub fn derive_spacing<R: Into<StaticLeaf<Spacing<P::Scalar>>>>(
    mut self,
    spacing: impl Fn() -> R + 'static,
  ) -> Self {
    self.column = self.column.derive_spacing(spacing);

    self
  }

  /// Sets the alignment of the card's children on the horizontal axis.
  #[inline(always)]
  pub fn set_item_alignment(
    mut self,
    alignment: impl Into<Value<ItemAlignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.column = self.column.set_item_alignment(alignment);

    self
  }

  /// Derives the alignment of the card's children from the signals `alignment` reads.
  #[inline(always)]
  pub fn derive_item_alignment<R: Into<StaticLeaf<ItemAlignment>>>(
    mut self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.column = self.column.derive_item_alignment(alignment);

    self
  }
}

impl<P: Platform> Default for SimpleCard<P>
where
  P::Scalar: SimpleUnit,
{
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> HasChildren<P> for SimpleCard<P> {
  type Kind = FlexChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<P, FlexChild> {
    self.column.children_mut(seal)
  }
}

impl<P: Platform> AnyKind for SimpleCard<P> {}

impl<P: Platform> View<P> for SimpleCard<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<P>();

    Container::new()
      .set_padding(shape::<P>().card_inset)
      .set_color(SimplePalette::<P::Scalar>::CARD)
      .set_border(hairline(SimplePalette::<P::Scalar>::BORDER))
      .set_corner_radius(CARD_RADIUS)
      .set_child(self.column)
      .build()
  }
}
