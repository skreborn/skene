//! The structure the runtime reads from the scene: children, parents and ancestors, the
//! layout's view of them, and what is in the display.

use rustc_hash::FxHashSet;
use skene_foundation::{NodeId, Scene};
use smallvec::SmallVec;

use super::{Drawn, Runtime};

impl<P: Drawn> Runtime<P> {
  /// The children of `node` as the scene places them, in order; empty for a leaf or a
  /// stale node.
  #[must_use]
  pub fn children(&self, node: NodeId) -> SmallVec<[NodeId; 8]> {
    Scene::<P>::read(|scene| scene.placed_children(node).collect())
  }

  /// The children of `node` as the scene places them, last first.
  pub fn children_reversed(&self, node: NodeId) -> impl Iterator<Item = NodeId> {
    self.children(node).into_iter().rev()
  }

  /// The node `node` is placed under.
  #[must_use]
  pub fn parent(&self, node: NodeId) -> Option<NodeId> {
    Scene::<P>::read(|scene| scene.placed_parent(node))
  }

  /// The path from `node` up to the root, `node` first.
  #[must_use]
  pub fn ancestors(&self, node: NodeId) -> Vec<NodeId> {
    Scene::<P>::read(|scene| {
      let mut path = Vec::new();
      let mut current = Some(node);

      while let Some(visited) = current {
        path.push(visited);
        current = scene.placed_parent(visited);
      }

      path
    })
  }

  /// Whether walking up from `node` reaches the root.
  #[must_use]
  pub fn is_reachable(&self, node: NodeId) -> bool {
    Scene::<P>::read(|scene| {
      let mut current = Some(node);

      while let Some(visited) = current {
        if visited == self.root {
          return true;
        }

        current = scene.placed_parent(visited);
      }

      false
    })
  }

  /// The nearest laid-out ancestor of `node`: the node whose layout children include
  /// it, the nearest with a record.
  #[must_use]
  pub fn layout_parent(&self, node: NodeId) -> Option<NodeId> {
    Scene::<P>::read(|scene| {
      let mut current = scene.placed_parent(node);

      while let Some(visited) = current {
        if self.contains(visited) {
          return Some(visited);
        }

        current = scene.placed_parent(visited);
      }

      None
    })
  }

  /// The laid-out descendants of `node` as the layout sees them: the nodes without a
  /// record, anchors and scopes, flattened into their content, overlays left out.
  #[must_use]
  pub fn layout_children(&self, node: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();

    self.collect_layout_children(node, &mut out);

    out
  }

  fn collect_layout_children(&self, node: NodeId, out: &mut Vec<NodeId>) {
    for child in self.children(node) {
      match self.record(child) {
        Some(record) if record.is_overlay() => {}
        Some(_) => out.push(child),
        None => self.collect_layout_children(child, out),
      }
    }
  }

  /// The nearest control at or above `node` in the scene, a caption kept beside a
  /// control answering to that control.
  #[must_use]
  pub fn enclosing_control(&self, node: NodeId) -> Option<NodeId> {
    Scene::<P>::read(|scene| scene.enclosing_control(node))
  }

  /// Notes that the structure the scene places changed: the layout and the index of
  /// window rectangles follow it and the frame repaints.
  pub fn note_structure(&mut self) {
    self.restyle.move_structure();
    self.invalidate_frames();
    self.paint_dirty = true;
  }

  /// Every node in the display: reachable from the root through open overlays alone.
  #[must_use]
  pub fn displayed(&self) -> FxHashSet<NodeId> {
    let mut displayed = FxHashSet::default();
    let mut stack = vec![self.root];

    Scene::<P>::read(|scene| {
      while let Some(current) = stack.pop() {
        if self
          .record(current)
          .is_some_and(|record| record.is_overlay() && !record.is_open_overlay())
        {
          continue;
        }

        displayed.insert(current);
        stack.extend(scene.placed_children(current));
      }
    });

    displayed
  }
}
