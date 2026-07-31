//! The SSR web platform's tree mutations, for the shared composition helpers.
//!
//! Selected by target-gating: the composition helpers call `attach` and it resolves, at
//! compile time, to the one leaf the target configures — here, a string tree rather than
//! a live DOM.

use skene_core::NodeId;

use super::with_state;

/// Allocates a detached grouping node.
#[inline(always)]
pub fn create_group() -> NodeId {
  with_state(super::State::create_group)
}

/// Attaches `child` to `parent`, immediately before `before` when given, otherwise at the
/// end.
#[inline(always)]
pub fn attach(parent: NodeId, child: NodeId, before: Option<NodeId>) {
  with_state(|state| state.attach_child(parent, child, before));
}

/// Detaches `child` from `parent`.
#[inline(always)]
pub fn detach(parent: NodeId, child: NodeId) {
  with_state(|state| state.detach_child(parent, child));
}

/// Destroys an already-detached node and its entire subtree.
#[inline(always)]
pub fn destroy(node: NodeId) {
  with_state(|state| state.destroy(node));
}
