use std::any::Any;

use crate::NodeId;

/// A renderer backend.
///
/// Owns a retained tree of nodes and realizes structural mutations against some target.
pub trait Backend: Any {
  /// Returns the root node of the tree.
  fn root(&self) -> NodeId;

  /// Allocates a detached grouping node.
  fn create_group(&self) -> NodeId;

  /// Attaches `child` to `parent`.
  ///
  /// If `before` is `Some(anchor)`, the child is attached immediately preceding `anchor`;
  /// otherwise, it is attached at the end.
  fn attach_child(&self, parent: NodeId, child: NodeId, before: Option<NodeId>);

  /// Detaches `child` from `parent`.
  fn detach_child(&self, parent: NodeId, child: NodeId);

  /// Destroys a node and its entire subtree, releasing their resources.
  ///
  /// The node must already be detached from its parent.
  fn destroy(&self, node: NodeId);
}
