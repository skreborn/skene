//! A platform's records, keyed by the scene's node ids.

use super::NodeId;

/// A platform's per-node records, keyed by the scene's [`NodeId`]s.
///
/// Dense by index; each entry remembers its node's generation, so a stale id reaches
/// nothing even after its slot is reused.
#[derive(Debug, Clone)]
#[must_use]
pub struct NodeMap<T> {
  entries: Vec<Option<(u32, T)>>,
}

impl<T> Default for NodeMap<T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<T> NodeMap<T> {
  /// An empty map.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      entries: Vec::new(),
    }
  }

  /// Stores `value` as `node`'s record, returning whatever its slot held.
  pub fn insert(&mut self, node: NodeId, value: T) -> Option<T> {
    let index = node.index() as usize;

    if index >= self.entries.len() {
      self.entries.resize_with(index + 1, || None);
    }

    self.entries[index]
      .replace((node.generation(), value))
      .map(|(_, previous)| previous)
  }

  /// `node`'s record, if it holds one.
  #[inline(always)]
  #[must_use]
  pub fn get(&self, node: NodeId) -> Option<&T> {
    match self.entries.get(node.index() as usize) {
      Some(Some((generation, value))) if *generation == node.generation() => Some(value),
      _ => None,
    }
  }

  /// `node`'s record, mutably, if it holds one.
  #[inline(always)]
  #[must_use]
  pub fn get_mut(&mut self, node: NodeId) -> Option<&mut T> {
    match self.entries.get_mut(node.index() as usize) {
      Some(Some((generation, value))) if *generation == node.generation() => Some(value),
      _ => None,
    }
  }

  /// The node whose record sits at `index`, if one does.
  #[inline(always)]
  #[must_use]
  pub fn node_at(&self, index: u32) -> Option<NodeId> {
    let (generation, _) = self.entries.get(index as usize)?.as_ref()?;

    Some(NodeId::new(index, *generation))
  }

  /// Whether `node` has a record.
  #[inline(always)]
  #[must_use]
  pub fn contains(&self, node: NodeId) -> bool {
    self.get(node).is_some()
  }

  /// Takes `node`'s record out.
  pub fn remove(&mut self, node: NodeId) -> Option<T> {
    let slot = self.entries.get_mut(node.index() as usize)?;

    match slot {
      Some((generation, _)) if *generation == node.generation() => {
        slot.take().map(|(_, value)| value)
      }
      _ => None,
    }
  }

  /// Every record with its node, in index order.
  pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
    self
      .entries
      .iter()
      .enumerate()
      .filter_map(|(index, entry)| {
        let (generation, value) = entry.as_ref()?;

        Some((NodeId::new(u32::try_from(index).ok()?, *generation), value))
      })
  }

  /// Every record, mutably, with its node, in index order.
  pub fn iter_mut(&mut self) -> impl Iterator<Item = (NodeId, &mut T)> {
    self
      .entries
      .iter_mut()
      .enumerate()
      .filter_map(|(index, entry)| {
        let (generation, value) = entry.as_mut()?;

        Some((NodeId::new(u32::try_from(index).ok()?, *generation), value))
      })
  }

  /// Every record, in index order.
  pub fn values(&self) -> impl Iterator<Item = &T> {
    self
      .entries
      .iter()
      .filter_map(|entry| entry.as_ref().map(|(_, value)| value))
  }

  /// Every record, mutably, in index order.
  pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
    self
      .entries
      .iter_mut()
      .filter_map(|entry| entry.as_mut().map(|(_, value)| value))
  }

  /// Drops every record.
  #[inline(always)]
  pub fn clear(&mut self) {
    self.entries.clear();
  }
}

#[cfg(test)]
mod tests {
  use super::NodeMap;
  use crate::NodeId;

  /// A record is reached by its node alone: a stale id of the same slot reaches nothing,
  /// and a reuse of the slot replaces it.
  #[test]
  fn a_record_is_reached_by_its_node_alone() {
    let mut map = NodeMap::new();
    let first = NodeId::new(2, 0);
    let reused = NodeId::new(2, 1);

    assert_eq!(map.insert(first, "first"), None);
    assert_eq!(map.get(first), Some(&"first"));
    assert_eq!(map.get(reused), None);
    assert_eq!(map.remove(reused), None);
    assert_eq!(map.insert(reused, "reused"), Some("first"));
    assert_eq!(map.get(first), None);
    assert_eq!(map.iter().collect::<Vec<_>>(), [(reused, &"reused")]);
    assert_eq!(map.remove(reused), Some("reused"));
    assert!(!map.contains(reused));
  }
}
