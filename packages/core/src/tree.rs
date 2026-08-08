//! The structural driver seam: the four tree mutations every facade's driver provides.
//!
//! A facade defines a zero-sized ops type calling its driver's functions and instantiates
//! the shared structural machinery ([`Children`](crate::Children),
//! [`reconcile`](crate::reconcile), [`derived`](crate::derived)) with it. The parameter
//! monomorphizes, so every call resolves statically into the one compiled driver—there is
//! no trait object and no runtime dispatch.

use crate::NodeId;

/// A driver's tree mutations, as a zero-sized, statically-dispatched parameter.
pub trait TreeOps: Copy + Default + 'static {
  /// Allocates a detached grouping node.
  fn create_group(self) -> NodeId;

  /// Attaches `child` to `parent`, immediately before `before` when given, otherwise at
  /// the end.
  fn attach(self, parent: NodeId, child: NodeId, before: Option<NodeId>);

  /// Detaches `child` from `parent`.
  fn detach(self, parent: NodeId, child: NodeId);

  /// Destroys an already-detached node and its entire subtree.
  fn destroy(self, node: NodeId);
}
