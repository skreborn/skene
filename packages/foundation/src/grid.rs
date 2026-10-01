//! The grid layout.

use crate::{
  Alignment,
  AnyKind,
  Arrangement,
  Children,
  Condition,
  DeclaredBoxStyle,
  GridAreas,
  GridChild,
  GridFlow,
  GridTemplate,
  GridTrack,
  HasChildren,
  Inset,
  Layout,
  LayoutBox,
  NodeId,
  Platform,
  Seal,
  Spacing,
  StaticLeaf,
  TrackLimit,
  Value,
  View,
  properties,
};

/// Children placed into tracks.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a
/// [`GridCell`](crate::GridCell) wrapper as a child.
#[derive(Debug)]
#[must_use]
pub struct Grid<P: Platform> {
  inner: LayoutBox<P, GridChild>,
}

impl<P: Platform> Default for Grid<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Grid<P> {
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

  /// A grid with as many columns as fit, each at least `minimum` wide, sharing what
  /// width remains equally.
  #[inline(always)]
  pub fn adaptive(minimum: P::Scalar) -> Self {
    Self::new().set_columns(vec![GridTrack::Adaptive(
      TrackLimit::Length(minimum),
      TrackLimit::Flex(1.),
    )])
  }

  #[inline(always)]
  const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.inner.style_mut()
  }

  properties! { style_mut;
    set_padding / derive_padding (padding) => padding: Inset<P::Scalar>,
      "the inset between the box and the children it arranges";
    set_spacing / derive_spacing (spacing) => spacing: Spacing<P::Scalar>,
      "the spacing between tracks per axis; a plain length spaces both";
    set_columns / derive_columns (tracks) => grid_columns: GridTemplate<P::Scalar>,
      "the explicit column tracks, with the names of their lines; a list of tracks names none";
    set_rows / derive_rows (tracks) => grid_rows: GridTemplate<P::Scalar>,
      "the explicit row tracks, with the names of their lines; a list of tracks names none";
    set_areas / derive_areas (areas) => grid_areas: GridAreas,
      "the named areas over the grid's cells, which a child's placement names";
    set_implicit_columns / derive_implicit_columns (tracks)
      => grid_implicit_columns: Vec<GridTrack<P::Scalar>>,
      "the sizes given to implicitly created columns, cycled";
    set_implicit_rows / derive_implicit_rows (tracks)
      => grid_implicit_rows: Vec<GridTrack<P::Scalar>>,
      "the sizes given to implicitly created rows, cycled";
    set_flow / derive_flow (flow) => grid_flow: GridFlow,
      "the direction auto-placed children fill";
    set_column_arrangement / derive_column_arrangement (arrangement) => arrangement: Arrangement,
      "the horizontal distribution of the column tracks";
    set_row_arrangement / derive_row_arrangement (arrangement) => line_arrangement: Arrangement,
      "the vertical distribution of the row tracks";
  }

  /// Sets the alignment of each child within its cell, on both axes.
  pub fn set_item_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.inner = self.inner.set_alignment(alignment);

    self
  }

  /// Derives the alignment of each child within its cell from the signals `alignment`
  /// reads.
  pub fn derive_item_alignment<R: Into<StaticLeaf<Alignment>>>(
    mut self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.inner = self.inner.derive_alignment(alignment);

    self
  }
}

impl<P: Platform> HasChildren<P> for Grid<P> {
  type Kind = GridChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<P, GridChild> {
    self.inner.children_mut(seal)
  }
}

impl<P: Platform> AnyKind for Grid<P> {}

impl<P: Platform> View<P> for Grid<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}
