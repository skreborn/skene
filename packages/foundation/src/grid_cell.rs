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
  Realization,
  Scene,
  StaticLeaf,
  Value,
  View,
  Wraps,
};

/// A placement value under the platform's conditions.
type Placement<P> = Value<Line<GridPlacement>, Condition<<P as Realization>::Scalar>>;

/// Places the child it wraps in the tracks of a [`Grid`](crate::Grid).
///
/// Creates no node: the placements and the alignment become properties of the child's
/// own style, so they compile, condition and derive like any property. The child is
/// auto-placed on an axis with no placement set. Accepted only where a grid's children
/// are; holds [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct GridCell<P: Realization, C = NoChild> {
  row: Option<Placement<P>>,
  column: Option<Placement<P>>,
  alignment: Option<Value<Alignment, Condition<P::Scalar>>>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization, C> std::fmt::Debug for GridCell<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("GridCell").finish_non_exhaustive()
  }
}

impl<P: Realization> Default for GridCell<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> GridCell<P> {
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

impl<P: Realization> GridCell<P> {
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

  /// Derives the child's alignment within its cell from the signals `alignment` reads.
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

impl<P: Realization> IntoChildOf<P, GridChild> for GridCell<P, AnyView<P>> {
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

/// What a facade's [`GridCell`] carries: its constructor and its setters.
pub trait GridCellBase<P: Realization>: Wraps<Inner = GridCell<P>> {
  /// A wrapper placing its child automatically, with no child yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(GridCell::new())
  }

  /// Sets the row placement.
  #[must_use]
  fn set_row(self, row: impl Into<Value<Line<GridPlacement>, Condition<P::Scalar>>>) -> Self {
    Self::wrap(self.into_inner().set_row(row))
  }

  /// Derives the row placement from the signals `row` reads: a value, a token or an
  /// expression.
  #[must_use]
  fn derive_row<R: Into<StaticLeaf<Line<GridPlacement>>>>(
    self,
    row: impl Fn() -> R + 'static,
  ) -> Self {
    Self::wrap(self.into_inner().derive_row(row))
  }

  /// Sets the column placement.
  #[must_use]
  fn set_column(self, column: impl Into<Value<Line<GridPlacement>, Condition<P::Scalar>>>) -> Self {
    Self::wrap(self.into_inner().set_column(column))
  }

  /// Derives the column placement from the signals `column` reads: a value, a token
  /// or an expression.
  #[must_use]
  fn derive_column<R: Into<StaticLeaf<Line<GridPlacement>>>>(
    self,
    column: impl Fn() -> R + 'static,
  ) -> Self {
    Self::wrap(self.into_inner().derive_column(column))
  }

  /// Sets the child's alignment within its cell, on both axes, overriding the grid's.
  #[must_use]
  fn set_alignment(self, alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>) -> Self {
    Self::wrap(self.into_inner().set_alignment(alignment))
  }

  /// Derives the child's alignment within its cell from the signals `alignment`
  /// reads.
  #[must_use]
  fn derive_alignment<R: Into<StaticLeaf<Alignment>>>(
    self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    Self::wrap(self.into_inner().derive_alignment(alignment))
  }
}
