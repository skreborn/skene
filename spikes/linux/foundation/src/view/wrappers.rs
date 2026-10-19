//! The wrappers: per-child properties a layout reads, each accepted only by the layout that
//! honors it ([`ChildOf`]). A wrapper creates no node: its properties are folded into its
//! child's own before the child is created, so they take conditions, tokens and derived
//! values like any property. A wrapper without a child shows nothing and takes no room.

use crate::{
  Alignment, AxisAlignment, Edges, Flexibility, GridPlacement, Value,
  behavior::Behavior,
  seam::{
    Realization,
    record::{Record, keys},
  },
  view::{
    ChildOf, Column, Control, Cx, Grid, HasRecord, Node, Row, Stack, View, Wraps, trait_properties,
    wraps_itself,
  },
};

/// A wrapper's record and child: what [`build_wrapped`] builds.
fn build_wrapped<R: Realization>(
  record: &Record,
  child: Option<crate::view::Child<R>>,
  cx: &mut Cx<R>,
) -> Node {
  let mut inner = cx.inherit();

  inner.fold_placement(record);

  match child {
    Some(child) => child.build(&mut inner),
    None => None::<crate::view::Container<R>>.build(&mut inner),
  }
}

/// How its child grows and shrinks in a `Row` or a `Column`, and where it sits across.
#[must_use]
pub struct Flexible<R: Realization> {
  record: Record,
  child: Option<crate::view::Child<R>>,
}

wraps_itself!(Flexible<R> where R: Realization);

impl<R: Realization> HasRecord for Flexible<R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    &mut self.record
  }
}

/// What a [`Flexible`] carries: its flexibility, its alignment and its child.
pub trait FlexibleBase<R: Realization>: Wraps<Inner = Flexible<R>> {
  /// A wrapper that changes nothing until told.
  fn new() -> Self {
    Self::wrap(Flexible {
      record: Record::new(),
      child: None,
    })
  }

  trait_properties! {
    /// How the child grows and shrinks: a weight (`1`) or a whole [`Flexibility`].
    set_flexibility, derive_flexibility: FLEXIBILITY, Flexibility;
  }

  /// Where the child sits across the layout's axis, overriding the layout's item alignment.
  fn set_alignment(mut self, alignment: impl Into<Value<AxisAlignment>>) -> Self {
    self.inner_mut().record.set(
      keys::SELF_ALIGNMENT,
      alignment
        .into()
        .map(|alignment| Alignment::new(alignment, alignment)),
    );

    self
  }

  /// Where the child sits across the layout's axis, overriding the layout's item alignment.
  ///
  /// Derived: recomputed whenever a signal the closure reads changes.
  fn derive_alignment<V: Into<Value<AxisAlignment>>>(
    self,
    alignment: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_alignment(Value::derived(alignment))
  }

  /// The child, last.
  fn set_child(mut self, child: impl View<R>) -> Self {
    self.inner_mut().child = Some(crate::view::Child::view(child));

    self
  }
}

impl<R: Realization> FlexibleBase<R> for Flexible<R> {}

impl<R: Realization> ChildOf<Row<R>, R> for Flexible<R> {
  fn build_child(self, cx: &mut Cx<R>) -> Node {
    build_wrapped(&self.record, self.child, cx)
  }
}

impl<R: Realization> ChildOf<Column<R>, R> for Flexible<R> {
  fn build_child(self, cx: &mut Cx<R>) -> Node {
    build_wrapped(&self.record, self.child, cx)
  }
}

/// A control lays its label, its parts and its body out along a row or a column.
impl<B: Behavior, R: Realization> ChildOf<Control<B, R>, R> for Flexible<R> {
  fn build_child(self, cx: &mut Cx<R>) -> Node {
    build_wrapped(&self.record, self.child, cx)
  }
}

/// Where its child sits in a `Grid`: its cell, and its alignment in it.
#[must_use]
pub struct GridCell<R: Realization> {
  record: Record,
  child: Option<crate::view::Child<R>>,
}

wraps_itself!(GridCell<R> where R: Realization);

impl<R: Realization> HasRecord for GridCell<R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    &mut self.record
  }
}

/// What a [`GridCell`] carries: its cell, its alignment and its child.
pub trait GridCellBase<R: Realization>: Wraps<Inner = GridCell<R>> {
  /// A wrapper that changes nothing until told.
  fn new() -> Self {
    Self::wrap(GridCell {
      record: Record::new(),
      child: None,
    })
  }

  trait_properties! {
    /// The cell, counted from 1, and its span.
    set_cell, derive_cell: CELL, GridPlacement;
    /// Where the child sits in its cell, overriding the grid's alignment.
    set_alignment, derive_alignment: SELF_ALIGNMENT, Alignment;
  }

  /// The child, last.
  fn set_child(mut self, child: impl View<R>) -> Self {
    self.inner_mut().child = Some(crate::view::Child::view(child));

    self
  }
}

impl<R: Realization> GridCellBase<R> for GridCell<R> {}

impl<R: Realization> ChildOf<Grid<R>, R> for GridCell<R> {
  fn build_child(self, cx: &mut Cx<R>) -> Node {
    build_wrapped(&self.record, self.child, cx)
  }
}

/// Where its child sits in a `Stack`: the edges it keeps its distance to.
#[must_use]
pub struct Pinned<R: Realization> {
  record: Record,
  child: Option<crate::view::Child<R>>,
}

wraps_itself!(Pinned<R> where R: Realization);

impl<R: Realization> HasRecord for Pinned<R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    &mut self.record
  }
}

/// What a [`Pinned`] carries: its edges and its child.
pub trait PinnedBase<R: Realization>: Wraps<Inner = Pinned<R>> {
  /// A wrapper that changes nothing until told.
  fn new() -> Self {
    Self::wrap(Pinned {
      record: Record::new(),
      child: None,
    })
  }

  trait_properties! {
    /// The edges the child keeps its distance to, in pixels.
    set_edges, derive_edges: PIN, Edges;
  }

  /// The child, last.
  fn set_child(mut self, child: impl View<R>) -> Self {
    self.inner_mut().child = Some(crate::view::Child::view(child));

    self
  }
}

impl<R: Realization> PinnedBase<R> for Pinned<R> {}

impl<R: Realization> ChildOf<Stack<R>, R> for Pinned<R> {
  fn build_child(self, cx: &mut Cx<R>) -> Node {
    build_wrapped(&self.record, self.child, cx)
  }
}
