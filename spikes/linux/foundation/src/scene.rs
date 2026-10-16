//! The scene: a generational arena of nodes, each with a parent and ordered children, holding
//! whatever a realization keeps per node (`N`).
//!
//! A [`NodeId`] names one node for as long as it lives; once the node is removed its id names
//! nothing, even after its slot is reused.

use std::fmt;

/// A node of a [`Scene`], valid until the node is removed.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId {
  index: u32,
  generation: u32,
}

impl fmt::Debug for NodeId {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "#{}v{}", self.index, self.generation)
  }
}

impl NodeId {
  /// The slot this node occupies, unique among the scene's living nodes.
  #[inline(always)]
  #[must_use]
  pub const fn index(self) -> u32 {
    self.index
  }

  /// How many nodes occupied the slot before this one.
  #[inline(always)]
  #[must_use]
  pub const fn generation(self) -> u32 {
    self.generation
  }
}

struct Entry<N> {
  data: N,
  parent: Option<NodeId>,
  children: Vec<NodeId>,
}

/// A tree of nodes holding `N` each; nodes start detached and are attached under a parent at
/// an index.
pub struct Scene<N> {
  slots: Vec<(u32, Option<Entry<N>>)>,
  free: Vec<u32>,
}

impl<N> Default for Scene<N> {
  fn default() -> Self {
    Self::new()
  }
}

impl<N> Scene<N> {
  /// An empty scene.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      slots: Vec::new(),
      free: Vec::new(),
    }
  }

  fn entry(&self, id: NodeId) -> Option<&Entry<N>> {
    match self.slots.get(id.index as usize) {
      Some((generation, Some(entry))) if *generation == id.generation => Some(entry),
      _ => None,
    }
  }

  fn entry_mut(&mut self, id: NodeId) -> Option<&mut Entry<N>> {
    match self.slots.get_mut(id.index as usize) {
      Some((generation, Some(entry))) if *generation == id.generation => Some(entry),
      _ => None,
    }
  }

  /// Adds a detached node holding `data`.
  pub fn insert(&mut self, data: N) -> NodeId {
    let entry = Entry {
      data,
      parent: None,
      children: Vec::new(),
    };

    if let Some(index) = self.free.pop() {
      let slot = &mut self.slots[index as usize];

      slot.0 += 1;
      slot.1 = Some(entry);

      return NodeId {
        index,
        generation: slot.0,
      };
    }

    self.slots.push((0, Some(entry)));

    NodeId {
      index: (self.slots.len() - 1) as u32,
      generation: 0,
    }
  }

  /// Whether `id` names a living node.
  #[must_use]
  pub fn contains(&self, id: NodeId) -> bool {
    self.entry(id).is_some()
  }

  /// What the node holds.
  #[must_use]
  pub fn get(&self, id: NodeId) -> Option<&N> {
    self.entry(id).map(|entry| &entry.data)
  }

  /// What the node holds, to change.
  pub fn get_mut(&mut self, id: NodeId) -> Option<&mut N> {
    self.entry_mut(id).map(|entry| &mut entry.data)
  }

  /// The node's parent, if it is attached.
  #[must_use]
  pub fn parent(&self, id: NodeId) -> Option<NodeId> {
    self.entry(id).and_then(|entry| entry.parent)
  }

  /// The node's children, in order.
  #[must_use]
  pub fn children(&self, id: NodeId) -> &[NodeId] {
    self.entry(id).map_or(&[], |entry| &entry.children)
  }

  /// Attaches `child` under `parent` at `index` (clamped to the children's count), detaching it
  /// from wherever it was first.
  pub fn attach(&mut self, parent: NodeId, child: NodeId, index: usize) {
    self.detach(child);

    if let Some(entry) = self.entry_mut(parent) {
      let index = index.min(entry.children.len());

      entry.children.insert(index, child);
    }

    if let Some(entry) = self.entry_mut(child) {
      entry.parent = Some(parent);
    }
  }

  /// Detaches `child` from its parent, keeping it and its subtree.
  pub fn detach(&mut self, child: NodeId) {
    let Some(parent) = self.parent(child) else {
      return;
    };

    if let Some(entry) = self.entry_mut(parent) {
      entry.children.retain(|id| *id != child);
    }

    if let Some(entry) = self.entry_mut(child) {
      entry.parent = None;
    }
  }

  /// Removes the node and its subtree, returning what each held, the node's first.
  pub fn remove(&mut self, id: NodeId) -> Vec<N> {
    self.detach(id);

    let mut removed = Vec::new();
    let mut stack = vec![id];

    while let Some(next) = stack.pop() {
      let slot = &mut self.slots[next.index as usize];

      if slot.0 != next.generation {
        continue;
      }

      if let Some(entry) = slot.1.take() {
        self.free.push(next.index);
        stack.extend(entry.children.iter().rev());
        removed.push(entry.data);
      }
    }

    removed
  }

  /// The node's ancestors, nearest first.
  #[must_use]
  pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
    let mut ancestors = Vec::new();
    let mut next = self.parent(id);

    while let Some(parent) = next {
      ancestors.push(parent);
      next = self.parent(parent);
    }

    ancestors
  }

  /// The node and its descendants in document order, each with its depth below `id`.
  #[must_use]
  pub fn descendants(&self, id: NodeId) -> Vec<(NodeId, usize)> {
    let mut order = Vec::new();
    let mut stack = vec![(id, 0)];

    while let Some((next, depth)) = stack.pop() {
      if !self.contains(next) {
        continue;
      }

      order.push((next, depth));

      stack.extend(
        self
          .children(next)
          .iter()
          .rev()
          .map(|child| (*child, depth + 1)),
      );
    }

    order
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_removed_id_names_nothing_after_reuse() {
    let mut scene = Scene::new();
    let root = scene.insert("root");
    let child = scene.insert("child");

    scene.attach(root, child, 0);
    assert_eq!(scene.remove(child), ["child"]);

    let reused = scene.insert("reused");

    assert_eq!(reused.index(), child.index());
    assert!(!scene.contains(child));
    assert_eq!(scene.children(root), &[] as &[NodeId]);
  }

  #[test]
  fn attaching_moves_a_child() {
    let mut scene = Scene::new();
    let a = scene.insert('a');
    let b = scene.insert('b');
    let c = scene.insert('c');

    scene.attach(a, c, 0);
    scene.attach(b, c, 0);

    assert_eq!(scene.children(a), &[] as &[NodeId]);
    assert_eq!(scene.parent(c), Some(b));
  }
}
