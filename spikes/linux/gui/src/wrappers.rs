//! The wrappers: per-child properties a layout reads, each accepted only by the layout that
//! honors it. A wrapper creates no node: its properties are folded into its child's own
//! before the child is created, so they take conditions, tokens and derived values like any
//! property. A wrapper without a child shows nothing and takes no room.

use skene_foundation::{
  view::{self as base, ChildOf, FlexibleBase, GridCellBase, Node, PinnedBase},
  wraps,
};

use crate::{Active, Cx};

/// How its child grows and shrinks in a `Row` or a `Column`, and where it sits across.
///
/// ```
/// use skene_gui::prelude::*;
///
/// Row::new()
///   .add_child(Flexible::new().set_flexibility(1).set_child(Text::new("Sync")))
///   .add_child(Toggle::new("On"));
/// ```
#[must_use]
pub struct Flexible(base::Flexible<Active>);

wraps!(wrapper [] Flexible => base::Flexible<Active>);

impl FlexibleBase<Active> for Flexible {}

impl<L> ChildOf<L, Active> for Flexible
where
  base::Flexible<Active>: ChildOf<L, Active>,
{
  #[inline(always)]
  fn build_child(self, cx: &mut Cx) -> Node {
    self.0.build_child(cx)
  }
}

/// Where its child sits in a `Grid`: its cell, and its alignment in it.
#[must_use]
pub struct GridCell(base::GridCell<Active>);

wraps!(wrapper [] GridCell => base::GridCell<Active>);

impl GridCellBase<Active> for GridCell {}

impl<L> ChildOf<L, Active> for GridCell
where
  base::GridCell<Active>: ChildOf<L, Active>,
{
  #[inline(always)]
  fn build_child(self, cx: &mut Cx) -> Node {
    self.0.build_child(cx)
  }
}

/// Where its child sits in a `Stack`: the edges it keeps its distance to.
#[must_use]
pub struct Pinned(base::Pinned<Active>);

wraps!(wrapper [] Pinned => base::Pinned<Active>);

impl PinnedBase<Active> for Pinned {}

impl<L> ChildOf<L, Active> for Pinned
where
  base::Pinned<Active>: ChildOf<L, Active>,
{
  #[inline(always)]
  fn build_child(self, cx: &mut Cx) -> Node {
    self.0.build_child(cx)
  }
}
