//! The wrapper placing a grid child in the grid's tracks.

use std::marker::PhantomData;

use crate::{
  Alignment,
  Annotation,
  AnyView,
  Condition,
  GridChild,
  GridPlacement,
  IntoChildOf,
  Leaf,
  Line,
  NoChild,
  Platform,
  Scene,
  StaticLeaf,
  Value,
  View,
};

/// Generates a [`GridCell`]'s setters on a facade's newtype holding one in `$field`, each
/// forwarding to the wrapper's own, on the platform `$platform`.
#[macro_export]
macro_rules! grid_cell_surface {
  ($field:tt, $platform:ty) => {
    $crate::forward! { $field;
      /// Sets the row placement.
      fn set_row(
        row: impl Into<
          $crate::Value<
            $crate::Line<$crate::GridPlacement>,
            $crate::Condition<<$platform as $crate::Platform>::Scalar>,
          >,
        >,
      );
      /// Derives the row placement from the signals `row` reads: a value, a token or an
      /// expression.
      fn derive_row[R: Into<$crate::StaticLeaf<$crate::Line<$crate::GridPlacement>>>](
        row: impl Fn() -> R + 'static,
      );
      /// Sets the column placement.
      fn set_column(
        column: impl Into<
          $crate::Value<
            $crate::Line<$crate::GridPlacement>,
            $crate::Condition<<$platform as $crate::Platform>::Scalar>,
          >,
        >,
      );
      /// Derives the column placement from the signals `column` reads: a value, a token
      /// or an expression.
      fn derive_column[R: Into<$crate::StaticLeaf<$crate::Line<$crate::GridPlacement>>>](
        column: impl Fn() -> R + 'static,
      );
      /// Sets the child's alignment within its cell, on both axes, overriding the grid's.
      fn set_alignment(
        alignment: impl Into<
          $crate::Value<$crate::Alignment, $crate::Condition<<$platform as $crate::Platform>::Scalar>>,
        >,
      );
      /// Derives the child's alignment within its cell from the signals `alignment`
      /// reads: a value, a token or an expression.
      fn derive_alignment[R: Into<$crate::StaticLeaf<$crate::Alignment>>](
        alignment: impl Fn() -> R + 'static,
      );
    }
  };
}

/// A placement value under the platform's conditions.
type Placement<P> = Value<Line<GridPlacement>, Condition<<P as Platform>::Scalar>>;

/// Places the child it wraps in the tracks of a [`Grid`](crate::Grid).
///
/// Creates no node: the placements and the alignment become properties of the child's
/// own style, so they compile, condition and derive like any property. The child is
/// auto-placed on an axis with no placement set. Accepted only where a grid's children
/// are; holds [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct GridCell<P: Platform, C = NoChild> {
  row: Option<Placement<P>>,
  column: Option<Placement<P>>,
  alignment: Option<Value<Alignment, Condition<P::Scalar>>>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, C> std::fmt::Debug for GridCell<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("GridCell").finish_non_exhaustive()
  }
}

impl<P: Platform> Default for GridCell<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> GridCell<P> {
  /// A wrapper placing its child automatically, with no child yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      row: None,
      column: None,
      alignment: None,
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> GridCell<P> {
  /// Sets the row placement.
  pub fn set_row(mut self, row: impl Into<Placement<P>>) -> Self {
    self.row = Some(row.into());

    self
  }

  /// Derives the row placement from the signals `row` reads: a value, a token or an
  /// expression.
  pub fn derive_row<R: Into<StaticLeaf<Line<GridPlacement>>>>(
    mut self,
    row: impl Fn() -> R + 'static,
  ) -> Self {
    self.row = Some(Value::new(Leaf::derive(row)));

    self
  }

  /// Sets the column placement.
  pub fn set_column(mut self, column: impl Into<Placement<P>>) -> Self {
    self.column = Some(column.into());

    self
  }

  /// Derives the column placement from the signals `column` reads: a value, a token or
  /// an expression.
  pub fn derive_column<R: Into<StaticLeaf<Line<GridPlacement>>>>(
    mut self,
    column: impl Fn() -> R + 'static,
  ) -> Self {
    self.column = Some(Value::new(Leaf::derive(column)));

    self
  }

  /// Sets the child's alignment within its cell, on both axes, overriding the grid's.
  pub fn set_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.alignment = Some(alignment.into());

    self
  }

  /// Derives the child's alignment within its cell from the signals `alignment` reads:
  /// a value, a token or an expression.
  pub fn derive_alignment<R: Into<StaticLeaf<Alignment>>>(
    mut self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.alignment = Some(Value::new(Leaf::derive(alignment)));

    self
  }

  /// Sets the child.
  pub fn set_child(self, child: impl View<P>) -> GridCell<P, AnyView<P>> {
    GridCell {
      row: self.row,
      column: self.column,
      alignment: self.alignment,
      child: child.into_any(),
      platform: PhantomData,
    }
  }
}

impl<P: Platform> IntoChildOf<P, GridChild> for GridCell<P, AnyView<P>> {
  fn into_child(self) -> AnyView<P> {
    AnyView::new(move || {
      let (horizontal, vertical) = match self.alignment.as_ref().map(Value::split_axes) {
        Some((horizontal, vertical)) => (Some(horizontal), Some(vertical)),
        None => (None, None),
      };

      let annotation = Annotation {
        grid_row_placement: self.row,
        grid_column_placement: self.column,
        self_horizontal_alignment: horizontal,
        self_alignment: vertical,
        ..Annotation::default()
      };

      Scene::<P>::annotate(annotation, || self.child.build())
    })
  }
}

/// Implements a facade's newtype `$name<C>` over the foundation's [`GridCell`].
///
/// The newtype holds the view on the platform `$platform` in its field; the macro
/// implements the constructor, the setters, the child and the child kind it is accepted
/// as.
#[macro_export]
macro_rules! impl_grid_cell {
  ($name:ident, $platform:ty) => {
    impl<C> ::core::fmt::Debug for $name<C> {
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
      /// A wrapper placing its child automatically, with no child yet.
      #[inline(always)]
      pub fn new() -> Self {
        Self($crate::GridCell::new())
      }

      $crate::grid_cell_surface! { 0, $platform }

      /// Sets the child.
      #[inline(always)]
      pub fn set_child(
        self,
        child: impl $crate::View<$platform>,
      ) -> $name<$crate::AnyView<$platform>> {
        $name(self.0.set_child(child))
      }
    }

    impl $crate::IntoChildOf<$platform, $crate::GridChild> for $name<$crate::AnyView<$platform>> {
      #[inline(always)]
      fn into_child(self) -> $crate::AnyView<$platform> {
        $crate::IntoChildOf::into_child(self.0)
      }
    }
  };
}
