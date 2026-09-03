//! The identity of a node in the scene.

/// A node in a [`Scene`](super::Scene): an arena index and the generation it was
/// allocated in.
///
/// An id whose node has been destroyed resolves to nothing, even after the arena slot
/// is reused, so a stale id never reaches another node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct NodeId {
  index: u32,
  generation: u32,
}

impl NodeId {
  #[inline(always)]
  pub(super) const fn new(index: u32, generation: u32) -> Self {
    Self { index, generation }
  }

  /// The arena slot the node occupies.
  #[inline(always)]
  #[must_use]
  pub const fn index(self) -> u32 {
    self.index
  }

  /// The generation of the slot this id was minted in.
  #[inline(always)]
  #[must_use]
  pub const fn generation(self) -> u32 {
    self.generation
  }
}
