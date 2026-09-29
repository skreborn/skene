//! A handle naming a built view as the box an overlay is placed against.

use std::{cell::Cell, rc::Rc};

use skene_core::NodeId;

/// A handle naming a built view as an overlay's anchor.
///
/// Bound to a view through its `bind_anchor` and given to
/// [`Overlay::set_anchor`](crate::Overlay::set_anchor); the open overlay is placed
/// against the bound view. Holds nothing until that view builds, and an overlay reads
/// it each time it opens.
#[derive(Clone, Debug, Default)]
#[must_use]
pub struct Anchor(Rc<Cell<Option<NodeId>>>);

impl Anchor {
  /// An unbound anchor.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Binds the anchor to `node`, a built view's root.
  #[inline(always)]
  pub fn assign(&self, node: NodeId) {
    self.0.set(Some(node));
  }

  /// The bound node, once the view built.
  #[inline(always)]
  #[must_use]
  pub fn node(&self) -> Option<NodeId> {
    self.0.get()
  }
}
