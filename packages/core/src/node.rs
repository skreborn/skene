/// A stable identifier for a node in a render tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u64);

impl NodeId {
  /// Creates a new instance from `value`.
  #[must_use]
  #[inline(always)]
  pub const fn new(value: u64) -> Self {
    NodeId(value)
  }

  /// Returns the contained integer value.
  #[must_use]
  #[inline(always)]
  pub const fn value(self) -> u64 {
    self.0
  }
}
