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
  /// An id as the scene mints it, of the node in slot `index` allocated in
  /// `generation`; one no live node of the scene holds resolves to nothing in it.
  #[inline(always)]
  pub const fn new(index: u32, generation: u32) -> Self {
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

  /// The node's id in the accessibility tree the scene reports
  /// ([`Scene::access_tree`](super::Scene::access_tree)): its generation and index,
  /// past the window's zero, its top bit clear.
  #[inline(always)]
  #[must_use]
  pub const fn access_id(self) -> accesskit::NodeId {
    let id = (((self.generation as u64) << 32) | self.index as u64) + 1;

    debug_assert!(
      id >> 63 == 0,
      "a node's accessibility id sets the bit of a run's"
    );

    accesskit::NodeId(id)
  }

  /// The id in the accessibility tree of the text run of line `line` of the node, a drawn
  /// text entry ([`AccessText`](super::AccessText)): its top bit set, then the low 15 bits
  /// of its generation, its index and the line.
  ///
  /// Only one generation of an index is live at a time, so no two runs of a tree share an
  /// id, nor a run a node's.
  #[inline(always)]
  #[must_use]
  pub const fn access_run_id(self, line: u16) -> accesskit::NodeId {
    accesskit::NodeId(
      1 << 63
        | ((self.generation as u64 & 0x7fff) << 48)
        | ((self.index as u64) << 16)
        | line as u64,
    )
  }

  /// A plain-data id for a test, of slot `index` in the first generation.
  ///
  /// # Panics
  ///
  /// Where `index` is past the largest slot, `u32::MAX`.
  #[cfg(test)]
  pub fn plain(index: usize) -> Self {
    Self::new(u32::try_from(index).unwrap(), 0)
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashSet;

  use super::NodeId;

  #[test]
  fn run_ids_are_unique_and_apart_from_node_ids() {
    let nodes = [
      NodeId::new(0, 0),
      NodeId::new(1, 0),
      NodeId::new(0, 1),
      NodeId::new(u32::MAX, 0x7fff),
      NodeId::new(70_000, 3),
    ];
    let mut seen = HashSet::new();

    for node in nodes {
      assert!(seen.insert(node.access_id()), "{node:?}'s id is its own");

      for line in [0, 1, 2, u16::MAX] {
        assert!(
          seen.insert(node.access_run_id(line)),
          "{node:?} line {line}"
        );
      }
    }

    assert_ne!(
      NodeId::new(5, 1).access_run_id(0),
      NodeId::new(5, 2).access_run_id(0)
    );
    assert_eq!(NodeId::new(0, 0).access_run_id(0).0 >> 63, 1);
    assert_eq!(NodeId::new(u32::MAX, (1 << 31) - 2).access_id().0 >> 63, 0);
  }
}
