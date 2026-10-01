//! The grid layout.

use crate::{
  AnyKind,
  Children,
  GridChild,
  GridTrack,
  HasChildren,
  Layout,
  LayoutBox,
  NodeId,
  Platform,
  Seal,
  TrackLimit,
  View,
};

/// Generates a [`Grid`]'s builder surface on a type holding one in `$field`.
///
/// The surface is the declared style, the properties a grid honors and the alignment of
/// its children, on the platform `$platform`. The foundation's grid expands it over its
/// box, and a facade's newtype over the grid it wraps.
#[macro_export]
macro_rules! grid_surface {
  ($field:tt, $platform:ty) => {
    /// The declared style, for a facade's medium-specific setters to write into.
    #[inline(always)]
    pub const fn style_mut(&mut self) -> &mut $crate::DeclaredBoxStyle<$platform> {
      self.$field.style_mut()
    }

    $crate::properties! { style_mut, $platform;
      set_padding / derive_padding (padding) => padding: $crate::Inset<<$platform as $crate::Platform>::Scalar>,
        "the inset between the box and the children it arranges";
      set_spacing / derive_spacing (spacing) => spacing: $crate::Spacing<<$platform as $crate::Platform>::Scalar>,
        "the spacing between tracks per axis; a plain length spaces both";
      set_columns / derive_columns (tracks) => grid_columns: $crate::GridTemplate<<$platform as $crate::Platform>::Scalar>,
        "the explicit column tracks, with the names of their lines; a list of tracks names none";
      set_rows / derive_rows (tracks) => grid_rows: $crate::GridTemplate<<$platform as $crate::Platform>::Scalar>,
        "the explicit row tracks, with the names of their lines; a list of tracks names none";
      set_areas / derive_areas (areas) => grid_areas: $crate::GridAreas,
        "the named areas over the grid's cells, which a child's placement names";
      set_implicit_columns / derive_implicit_columns (tracks)
        => grid_implicit_columns: Vec<$crate::GridTrack<<$platform as $crate::Platform>::Scalar>>,
        "the sizes given to implicitly created columns, cycled";
      set_implicit_rows / derive_implicit_rows (tracks)
        => grid_implicit_rows: Vec<$crate::GridTrack<<$platform as $crate::Platform>::Scalar>>,
        "the sizes given to implicitly created rows, cycled";
      set_flow / derive_flow (flow) => grid_flow: $crate::GridFlow,
        "the direction auto-placed children fill";
      set_column_arrangement / derive_column_arrangement (arrangement) => arrangement: $crate::Arrangement,
        "the horizontal distribution of the column tracks";
      set_row_arrangement / derive_row_arrangement (arrangement) => line_arrangement: $crate::Arrangement,
        "the vertical distribution of the row tracks";
    }

    $crate::alignment_properties! { style_mut, $platform;
      set_item_alignment / derive_item_alignment, "the alignment of each child within its cell"
    }
  };
}

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

  crate::grid_surface! { inner, P }
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

/// Implements a facade's newtype `$name` over the foundation's [`Grid`].
///
/// The newtype holds the view on the platform `$platform` in its field; the macro
/// implements the constructors, the builder surface and the traits a grid meets.
#[macro_export]
macro_rules! impl_grid {
  ($name:ident, $platform:ty) => {
    impl ::core::fmt::Debug for $name {
      fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Debug::fmt(&self.0, formatter)
      }
    }
    impl ::core::default::Default for $name {
      #[inline(always)]
      fn default() -> Self {
        Self::new()
      }
    }
    impl $name {
      /// A grid with no explicit tracks.
      #[inline(always)]
      pub fn new() -> Self {
        Self($crate::Grid::new())
      }

      /// A grid with `count` columns of equal width.
      #[inline(always)]
      pub fn uniform(count: u16) -> Self {
        Self($crate::Grid::uniform(count))
      }

      /// A grid with as many columns as fit, each at least `minimum` wide, sharing what
      /// width remains equally.
      #[inline(always)]
      pub fn adaptive(minimum: <$platform as $crate::Platform>::Scalar) -> Self {
        Self($crate::Grid::adaptive(minimum))
      }

      $crate::grid_surface! { 0, $platform }
    }

    impl $crate::HasChildren<$platform> for $name {
      type Kind = $crate::GridChild;

      #[inline(always)]
      fn children_mut(
        &mut self,
        seal: $crate::Seal,
      ) -> &mut $crate::Children<$platform, $crate::GridChild> {
        $crate::HasChildren::children_mut(&mut self.0, seal)
      }
    }

    impl $crate::AnyKind for $name {}

    impl $crate::View<$platform> for $name {
      #[inline(always)]
      fn build(self) -> $crate::NodeId {
        $crate::View::build(self.0)
      }
    }
  };
}
