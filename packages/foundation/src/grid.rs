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
  HasBoxStyle,
  HasChildren,
  Inset,
  Layout,
  LayoutBox,
  Leaf,
  NodeId,
  Realization,
  Seal,
  Spacing,
  StaticLeaf,
  TrackLimit,
  Value,
  View,
  Wraps,
};

/// Children placed into tracks.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a
/// [`GridCell`](crate::GridCell) wrapper as a child.
#[derive(Debug)]
#[must_use]
pub struct Grid<P: Realization> {
  inner: LayoutBox<P, GridChild>,
}

impl<P: Realization> Default for Grid<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Grid<P> {
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
  pub fn adaptive(minimum: P::Scalar) -> Self {
    Self::new().set_columns(vec![GridTrack::Adaptive(
      TrackLimit::Length(minimum),
      TrackLimit::Flex(1.),
    )])
  }
}

impl<P: Realization> HasChildren<P> for Grid<P> {
  type Kind = GridChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<P, GridChild> {
    self.inner.children_mut(seal)
  }
}

impl<P: Realization> AnyKind for Grid<P> {}

impl<P: Realization> View<P> for Grid<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}

impl<P: Realization> HasBoxStyle<P> for Grid<P> {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.inner.style_mut()
  }
}

impl<P: Realization> GridBase<P> for Grid<P> {}

/// What a [`Grid`] carries, with its constructors on a facade's newtype.
///
/// The inset around its children, the spacing and arrangement of its tracks, the tracks
/// themselves, the areas over its cells, how it places children and how it aligns each in
/// its cell.
pub trait GridBase<P: Realization>: HasBoxStyle<P> + Sized {
  /// A grid with no explicit tracks.
  #[must_use]
  fn new() -> Self
  where
    Self: Wraps<Inner = Grid<P>>,
  {
    Self::wrap(Grid::new())
  }

  /// A grid with `count` columns of equal width.
  #[must_use]
  fn uniform(count: u16) -> Self
  where
    Self: Wraps<Inner = Grid<P>>,
  {
    Self::wrap(Grid::uniform(count))
  }

  /// A grid with as many columns as fit, each at least `minimum` wide, sharing what width
  /// remains equally.
  #[must_use]
  fn adaptive(minimum: P::Scalar) -> Self
  where
    Self: Wraps<Inner = Grid<P>>,
  {
    Self::wrap(Grid::adaptive(minimum))
  }

  crate::trait_properties! { box_style_mut, Condition<P::Scalar>;
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
  #[must_use]
  fn set_item_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>,
  ) -> Self {
    let (horizontal, vertical) = alignment.into().split_axes();
    let style = self.box_style_mut();

    style.item_horizontal_alignment = Some(horizontal);
    style.item_alignment = Some(vertical);

    self
  }

  /// Derives the alignment of each child within its cell from the signals `alignment`
  /// reads.
  #[must_use]
  fn derive_item_alignment<R: Into<StaticLeaf<Alignment>>>(
    self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.set_item_alignment(Value::<Alignment, Condition<P::Scalar>>::new(Leaf::derive(
      alignment,
    )))
  }
}
