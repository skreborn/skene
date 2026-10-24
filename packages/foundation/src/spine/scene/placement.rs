//! Where the platform places a node: under the nearest ancestor it made, before the
//! next node it made after it, a node it did not make placing its children in its
//! stead.

use smallvec::SmallVec;

use super::Scene;
use crate::spine::{NodeId, Realization};

/// A tree as the platform's placement reads it: each node's parent and children, which
/// nodes the platform made, and which caption a control keeps beside it.
pub(in crate::spine) trait Placing {
  /// The node `node` stands under.
  fn parent_of(&self, node: NodeId) -> Option<NodeId>;

  /// The first of `node`'s children.
  fn first_child_of(&self, node: NodeId) -> Option<NodeId>;

  /// The node after `node` among its parent's children.
  fn next_of(&self, node: NodeId) -> Option<NodeId>;

  /// Whether the platform made a node for `node`.
  fn is_realized(&self, node: NodeId) -> bool;

  /// Whether `node` is the caption the control it stands under keeps beside it, which
  /// the platform places before the control rather than among its children.
  fn is_kept_beside(&self, node: NodeId) -> bool;

  /// The nearest node at or above `node` the platform made: the parent the platform
  /// places a child of `node` under.
  fn realized_ancestor(&self, node: NodeId) -> Option<NodeId> {
    let mut current = Some(node);

    while let Some(visited) = current {
      if self.is_realized(visited) {
        return Some(visited);
      }

      current = self.parent_of(visited);
    }

    None
  }

  /// The nodes the platform holds for `node`, in order: `node` where it made it, else
  /// those it holds for each of its children.
  fn realized_within(&self, node: NodeId) -> SmallVec<[NodeId; 1]> {
    let mut realized = SmallVec::new();

    if self.is_realized(node) {
      realized.push(node);

      return realized;
    }

    let mut stack: SmallVec<[NodeId; 8]> = SmallVec::new();

    stack.extend(self.first_child_of(node));

    while let Some(current) = stack.pop() {
      stack.extend(self.next_of(current));

      if self.is_realized(current) {
        realized.push(current);
      }
      else if let Some(first) = self.first_child_of(current) {
        stack.push(first);
      }
    }

    realized
  }

  /// The first of the nodes the platform holds for `node`.
  fn first_realized_within(&self, node: NodeId) -> Option<NodeId> {
    if self.is_realized(node) {
      return Some(node);
    }

    let mut child = self.first_child_of(node);

    while let Some(current) = child {
      if let Some(first) = self.first_realized_within(current) {
        return Some(first);
      }

      child = self.next_of(current);
    }

    None
  }

  /// The node the platform holds next after `node` under the parent it places `node`
  /// under: the first it holds for a later sibling, a caption kept beside its control
  /// passed over, and past the last sibling of a parent it did not make, for that
  /// parent's later siblings in turn; `None` where `node` stands last.
  fn realized_after(&self, node: NodeId) -> Option<NodeId> {
    let mut current = node;

    loop {
      let mut sibling = self.next_of(current);

      while let Some(visited) = sibling {
        if !self.is_kept_beside(visited)
          && let Some(first) = self.first_realized_within(visited)
        {
          return Some(first);
        }

        sibling = self.next_of(visited);
      }

      let parent = self.parent_of(current)?;

      if self.is_realized(parent) {
        return None;
      }

      current = parent;
    }
  }
}

/// A change of structure the platform is told of, naming only the nodes it made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::spine) enum Structural {
  /// [`Realization::attach`].
  Attach {
    parent: NodeId,
    child: NodeId,
    before: Option<NodeId>,
  },
  /// [`Realization::relocate`].
  Relocate {
    parent: NodeId,
    child: NodeId,
    before: Option<NodeId>,
  },
  /// [`Realization::hide`].
  Hide { parent: NodeId, child: NodeId },
  /// [`Realization::remove`].
  Remove { parent: NodeId, child: NodeId },
}

impl Structural {
  /// What the platform is told as `child`, just placed in `tree`, arrives, `from` the
  /// node it held `child` under before, `None` where it held it under none.
  ///
  /// Each node the platform holds for `child` is attached, or relocated from `from`,
  /// under the nearest node it made above `child`, before the next it holds after
  /// `child`; where it made none above, each is hidden from `from`, if anything.
  pub(in crate::spine) fn arrival(
    tree: &impl Placing,
    child: NodeId,
    from: Option<NodeId>,
  ) -> SmallVec<[Self; 1]> {
    let to = tree
      .parent_of(child)
      .and_then(|parent| tree.realized_ancestor(parent));
    let before = to.and_then(|_| tree.realized_after(child));

    tree
      .realized_within(child)
      .into_iter()
      .filter_map(|node| match (from, to) {
        (None, Some(parent)) => Some(Self::Attach {
          parent,
          child: node,
          before,
        }),
        (Some(_), Some(parent)) => Some(Self::Relocate {
          parent,
          child: node,
          before,
        }),
        (Some(parent), None) => Some(Self::Hide {
          parent,
          child: node,
        }),
        (None, None) => None,
      })
      .collect()
  }

  /// What the platform is told as `child` leaves `parent` in `tree`, for good where
  /// `removed`, else to return: each node it holds for `child`, taken from the nearest
  /// node it made at or above `parent`; nothing where it made none.
  pub(in crate::spine) fn departure(
    tree: &impl Placing,
    parent: NodeId,
    child: NodeId,
    removed: bool,
  ) -> SmallVec<[Self; 1]> {
    let Some(parent) = tree.realized_ancestor(parent)
    else {
      return SmallVec::new();
    };

    tree
      .realized_within(child)
      .into_iter()
      .map(|node| {
        if removed {
          Self::Remove {
            parent,
            child: node,
          }
        }
        else {
          Self::Hide {
            parent,
            child: node,
          }
        }
      })
      .collect()
  }

  /// Hands the change to the platform.
  pub(in crate::spine) fn run<P: Realization>(self) {
    match self {
      Self::Attach {
        parent,
        child,
        before,
      } => P::attach(parent, child, before),
      Self::Relocate {
        parent,
        child,
        before,
      } => P::relocate(parent, child, before),
      Self::Hide { parent, child } => P::hide(parent, child),
      Self::Remove { parent, child } => P::remove(parent, child),
    }
  }
}

impl<P: Realization> Placing for Scene<P> {
  #[inline(always)]
  fn parent_of(&self, node: NodeId) -> Option<NodeId> {
    self.parent(node)
  }

  #[inline(always)]
  fn first_child_of(&self, node: NodeId) -> Option<NodeId> {
    self.record(node).and_then(|record| record.first_child)
  }

  #[inline(always)]
  fn next_of(&self, node: NodeId) -> Option<NodeId> {
    self.next_sibling(node)
  }

  #[inline(always)]
  fn is_realized(&self, node: NodeId) -> bool {
    self.record(node).is_some_and(|record| record.realized)
  }

  fn is_kept_beside(&self, node: NodeId) -> bool {
    self
      .parent(node)
      .and_then(|parent| self.caption_beside(parent))
      == Some(node)
  }
}

#[cfg(test)]
mod tests {
  use rustc_hash::{FxHashMap, FxHashSet};
  use smallvec::SmallVec;

  use super::{Placing, Structural};
  use crate::spine::NodeId;

  /// A tree as plain data: each node's children in order, the nodes the platform did
  /// not make, and the captions kept beside their controls.
  #[derive(Default)]
  struct Tree {
    children: FxHashMap<NodeId, Vec<NodeId>>,
    parents: FxHashMap<NodeId, NodeId>,
    unrealized: FxHashSet<NodeId>,
    beside: FxHashSet<NodeId>,
  }

  impl Tree {
    /// Places `children` under `parent`, in order, after any it has.
    fn add(&mut self, parent: NodeId, children: &[NodeId]) -> &mut Self {
      for child in children {
        self.parents.insert(*child, parent);
      }

      self
        .children
        .entry(parent)
        .or_default()
        .extend_from_slice(children);

      self
    }

    /// Records the platform made no node for each of `nodes`.
    fn unrealized(&mut self, nodes: &[NodeId]) -> &mut Self {
      self.unrealized.extend(nodes);

      self
    }

    /// Takes `child` from under its parent.
    fn take(&mut self, child: NodeId) {
      if let Some(parent) = self.parents.remove(&child)
        && let Some(children) = self.children.get_mut(&parent)
      {
        children.retain(|sibling| *sibling != child);
      }
    }
  }

  impl Placing for Tree {
    fn parent_of(&self, node: NodeId) -> Option<NodeId> {
      self.parents.get(&node).copied()
    }

    fn first_child_of(&self, node: NodeId) -> Option<NodeId> {
      self.children.get(&node)?.first().copied()
    }

    fn next_of(&self, node: NodeId) -> Option<NodeId> {
      let siblings = self.children.get(self.parents.get(&node)?)?;
      let index = siblings.iter().position(|sibling| *sibling == node)?;

      siblings.get(index + 1).copied()
    }

    fn is_realized(&self, node: NodeId) -> bool {
      !self.unrealized.contains(&node)
    }

    fn is_kept_beside(&self, node: NodeId) -> bool {
      self.beside.contains(&node)
    }
  }

  fn list(nodes: &[NodeId]) -> SmallVec<[NodeId; 1]> {
    nodes.iter().copied().collect()
  }

  /// A segment's content inside a scope the platform did not make is placed under the
  /// scope's realized parent, before what follows the scope there, its anchors standing
  /// for nothing; the scope stands for that content alone.
  #[test]
  fn an_anchor_pair_inside_a_scope_places_its_content_beside_the_scope() {
    let [root, first, scope, start, inner, end, last] = std::array::from_fn(NodeId::plain);
    let mut tree = Tree::default();

    tree
      .add(root, &[first, scope, last])
      .add(scope, &[start, inner, end])
      .unrealized(&[scope, start, end]);

    assert_eq!(tree.realized_ancestor(scope), Some(root));
    assert_eq!(tree.realized_after(inner), Some(last));
    assert_eq!(tree.realized_after(first), Some(inner));
    assert_eq!(tree.realized_after(start), Some(inner));
    assert_eq!(tree.realized_within(scope), list(&[inner]));
    assert_eq!(tree.realized_within(start), list(&[]));
    assert_eq!(
      Structural::arrival(&tree, inner, None).as_slice(),
      [Structural::Attach {
        parent: root,
        child: inner,
        before: Some(last),
      }]
    );
    assert_eq!(
      Structural::arrival(&tree, scope, None).as_slice(),
      [Structural::Attach {
        parent: root,
        child: inner,
        before: Some(last),
      }]
    );
    assert!(Structural::arrival(&tree, start, None).is_empty());
  }

  /// An empty segment the platform did not make tells it of nothing, and a node
  /// placed after it is placed before what follows it.
  #[test]
  fn an_empty_segment_places_nothing() {
    let [root, first, start, end, last] = std::array::from_fn(NodeId::plain);
    let mut tree = Tree::default();

    tree
      .add(root, &[first, start, end, last])
      .unrealized(&[start, end]);

    assert_eq!(tree.realized_after(first), Some(last));
    assert_eq!(tree.realized_after(end), Some(last));
    assert!(Structural::arrival(&tree, start, None).is_empty());
    assert!(Structural::departure(&tree, root, end, true).is_empty());
    assert_eq!(
      Structural::departure(&tree, root, last, true).as_slice(),
      [Structural::Remove {
        parent: root,
        child: last,
      }]
    );
  }

  /// A caption a control keeps beside it is passed over among the control's children,
  /// where the platform holds it elsewhere, while the control stands for it among its
  /// own siblings.
  #[test]
  fn a_caption_kept_beside_is_passed_over() {
    let [root, first, control, inner, caption, face, last] = std::array::from_fn(NodeId::plain);
    let mut tree = Tree::default();

    tree
      .add(root, &[first, control, last])
      .add(control, &[inner, caption, face]);
    tree.beside.insert(caption);

    assert_eq!(tree.realized_after(inner), Some(face));
    assert_eq!(tree.realized_after(first), Some(control));
    assert_eq!(tree.realized_after(face), None);
    assert_eq!(
      Structural::arrival(&tree, caption, None).as_slice(),
      [Structural::Attach {
        parent: control,
        child: caption,
        before: Some(face),
      }]
    );
  }

  /// A move across an unrealized boundary relocates every node the platform holds for
  /// the moved node, attaches them where they come from under nothing it made, and
  /// hides them where they go under nothing it made.
  #[test]
  fn a_move_across_an_unrealized_boundary_maps_to_what_the_platform_holds() {
    let [root, scope, inner, other, last, detached, below, end] =
      std::array::from_fn(NodeId::plain);
    let mut tree = Tree::default();

    tree
      .add(root, &[scope, last])
      .add(scope, &[inner, other])
      .add(detached, &[below])
      .unrealized(&[scope, detached, below]);

    tree.take(scope);
    tree.add(detached, &[scope, end]);

    assert_eq!(tree.realized_ancestor(scope), None);
    assert_eq!(
      Structural::arrival(&tree, scope, Some(root)).as_slice(),
      [
        Structural::Hide {
          parent: root,
          child: inner,
        },
        Structural::Hide {
          parent: root,
          child: other,
        },
      ]
    );

    tree.take(scope);
    tree.take(end);
    tree.add(root, &[scope]);

    assert_eq!(
      Structural::arrival(&tree, scope, None).as_slice(),
      [
        Structural::Attach {
          parent: root,
          child: inner,
          before: None,
        },
        Structural::Attach {
          parent: root,
          child: other,
          before: None,
        },
      ]
    );

    tree.take(inner);
    tree.add(root, &[inner]);

    assert_eq!(
      Structural::arrival(&tree, inner, Some(root)).as_slice(),
      [Structural::Relocate {
        parent: root,
        child: inner,
        before: None,
      }]
    );
    assert_eq!(tree.realized_after(other), Some(inner));

    tree.take(inner);
    tree.add(below, &[inner]);

    assert!(Structural::arrival(&tree, inner, None).is_empty());
    assert!(Structural::departure(&tree, below, inner, false).is_empty());
  }
}
