//! The wrappers: per-child properties a layout reads, each accepted only by the layout that
//! honors it ([`ChildOf`]). A wrapper creates no node: its properties are folded into its
//! child's own before the child is created, so they take conditions, tokens and derived
//! values like any property. A wrapper without a child shows nothing and takes no room.

use skene_foundation::{
  Alignment, AxisAlignment, Edges, Flexibility, GridPlacement, Value,
  seam::record::{Record, keys},
};

use crate::{
  Column, Grid, Row, Stack,
  view::{Child, ChildOf, Cx, Node, View, child_of, properties},
};

/// Builds `child` with `record` folded into the placement it is built under, or nothing where
/// there is none.
fn build_wrapped(record: &Record, child: Option<Child>, cx: &mut Cx) -> Node {
  let mut placement = cx.placement.clone();

  placement.fold(record);

  let mut inner = Cx {
    scope: cx.scope.clone(),
    placement,
  };

  match child {
    Some(child) => child(&mut inner),
    None => None::<crate::Container>.build(&mut inner),
  }
}

/// How its child grows and shrinks in a `Row` or a `Column`, and where it sits across.
///
/// ```
/// use skene_gui::prelude::*;
///
/// Row::new()
///   .add_child(Flexible::new().set_flexibility(1).set_child(Text::new("Sync")))
///   .add_child(Toggle::new("On"));
/// ```
#[derive(Default)]
#[must_use]
pub struct Flexible {
  record: Record,
  child: Option<Child>,
}

impl Flexible {
  /// A wrapper that changes nothing until told.
  pub fn new() -> Self {
    Self::default()
  }

  properties! {
    /// How the child grows and shrinks: a weight (`1`) or a whole [`Flexibility`].
    set_flexibility, derive_flexibility: FLEXIBILITY, Flexibility;
  }

  /// Where the child sits across the layout's axis, overriding the layout's item alignment.
  pub fn set_alignment(mut self, alignment: impl Into<Value<AxisAlignment>>) -> Self {
    self.record.set(
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
  pub fn derive_alignment<V: Into<Value<AxisAlignment>>>(
    self,
    alignment: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_alignment(Value::derived(alignment))
  }

  /// The child, last.
  pub fn set_child(mut self, child: impl View) -> Self {
    self.child = Some(child_of::<Self>(child));

    self
  }
}

impl ChildOf<Row> for Flexible {
  fn build_child(self, cx: &mut Cx) -> Node {
    build_wrapped(&self.record, self.child, cx)
  }
}

impl ChildOf<Column> for Flexible {
  fn build_child(self, cx: &mut Cx) -> Node {
    build_wrapped(&self.record, self.child, cx)
  }
}

/// Where its child sits in a `Grid`: its cell, and its alignment in it.
#[derive(Default)]
#[must_use]
pub struct GridCell {
  record: Record,
  child: Option<Child>,
}

impl GridCell {
  /// A wrapper that changes nothing until told.
  pub fn new() -> Self {
    Self::default()
  }

  properties! {
    /// The cell, counted from 1, and its span.
    set_cell, derive_cell: CELL, GridPlacement;
    /// Where the child sits in its cell, overriding the grid's alignment.
    set_alignment, derive_alignment: SELF_ALIGNMENT, Alignment;
  }

  /// The child, last.
  pub fn set_child(mut self, child: impl View) -> Self {
    self.child = Some(child_of::<Self>(child));

    self
  }
}

impl ChildOf<Grid> for GridCell {
  fn build_child(self, cx: &mut Cx) -> Node {
    build_wrapped(&self.record, self.child, cx)
  }
}

/// Where its child sits in a `Stack`: the edges it keeps its distance to.
#[derive(Default)]
#[must_use]
pub struct Pinned {
  record: Record,
  child: Option<Child>,
}

impl Pinned {
  /// A wrapper that changes nothing until told.
  pub fn new() -> Self {
    Self::default()
  }

  properties! {
    /// The edges the child keeps its distance to, in pixels.
    set_edges, derive_edges: PIN, Edges;
  }

  /// The child, last.
  pub fn set_child(mut self, child: impl View) -> Self {
    self.child = Some(child_of::<Self>(child));

    self
  }
}

impl ChildOf<Stack> for Pinned {
  fn build_child(self, cx: &mut Cx) -> Node {
    build_wrapped(&self.record, self.child, cx)
  }
}
