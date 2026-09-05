//! The grid layout.

use skene_core::{
  Alignment,
  AnyKind,
  Arrangement,
  Children,
  Condition,
  DeclaredBoxStyle,
  Driver,
  GridChild,
  GridFlow,
  GridTrack,
  HasChildren,
  Inset,
  Layout,
  NodeId,
  Seal,
  Spacing,
  Value,
  View,
};

use crate::{LayoutBox, macros::properties};

/// Children placed into tracks.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a
/// [`GridCell`](crate::GridCell) wrapper as a child.
#[must_use]
pub struct Grid<D: Driver> {
  inner: LayoutBox<D, GridChild>,
}

impl<D: Driver> Default for Grid<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> Grid<D> {
  /// A grid with no explicit tracks.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Grid),
    }
  }

  /// A grid with `count` columns of equal width.
  #[inline(always)]
  pub fn uniform(count: u16) -> Self {
    Self::new().set_columns(GridTrack::Flex(1.).repeated(count).collect::<Vec<_>>())
  }

  #[inline(always)]
  const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<D> {
    self.inner.style_mut()
  }

  properties! { style_mut;
    set_padding / derive_padding (padding) => padding: Inset<D::Scalar>,
      "the inset between the box and the children it arranges";
    set_spacing / derive_spacing (spacing) => spacing: Spacing<D::Scalar>,
      "the spacing between tracks per axis; a plain length spaces both";
    set_columns / derive_columns (tracks) => grid_columns: Vec<GridTrack<D::Scalar>>,
      "the explicit column tracks";
    set_rows / derive_rows (tracks) => grid_rows: Vec<GridTrack<D::Scalar>>,
      "the explicit row tracks";
    set_implicit_columns / derive_implicit_columns (tracks)
      => grid_implicit_columns: Vec<GridTrack<D::Scalar>>,
      "the sizes given to implicitly created columns, cycled";
    set_implicit_rows / derive_implicit_rows (tracks)
      => grid_implicit_rows: Vec<GridTrack<D::Scalar>>,
      "the sizes given to implicitly created rows, cycled";
    set_flow / derive_flow (flow) => grid_flow: GridFlow,
      "the direction auto-placed children fill";
    set_column_arrangement / derive_column_arrangement (arrangement) => arrangement: Arrangement,
      "the horizontal distribution of the column tracks";
    set_row_arrangement / derive_row_arrangement (arrangement) => line_arrangement: Arrangement,
      "the vertical distribution of the row tracks";
  }

  /// Sets the alignment of each child within its cell, on both axes.
  #[inline(always)]
  pub fn set_item_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<D::Scalar>>>,
  ) -> Self {
    self.inner = self.inner.set_alignment(alignment);

    self
  }

  /// Derives the alignment of each child within its cell from the signals `alignment`
  /// reads.
  #[inline(always)]
  pub fn derive_item_alignment(mut self, alignment: impl Fn() -> Alignment + 'static) -> Self {
    self.inner = self.inner.derive_alignment(alignment);

    self
  }
}

impl<D: Driver> HasChildren<D> for Grid<D> {
  type Kind = GridChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<D, GridChild> {
    self.inner.children_mut(seal)
  }
}

impl<D: Driver> AnyKind for Grid<D> {}

impl<D: Driver> View<D> for Grid<D> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}
