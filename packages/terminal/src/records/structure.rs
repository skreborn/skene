//! The structure the records read from the scene: children, parents and ancestors, the
//! layout tree's view of them, and what is in the display.

use rustc_hash::FxHashSet;
use skene_foundation::Scene;
use smallvec::SmallVec;

use super::{Handle, Records};
use crate::Terminal;

impl Records {
  /// The children of `handle` as the scene places them, in order; empty for a leaf or a
  /// stale handle.
  #[expect(
    clippy::unused_self,
    reason = "every reader holds the records, and structure is asked of it as of the rest"
  )]
  pub(crate) fn children(&self, handle: Handle) -> SmallVec<[Handle; 8]> {
    Scene::<Terminal>::read(|scene| scene.placed_children(handle).collect())
  }

  /// The children of `handle` as the scene places them, last first.
  pub(crate) fn children_reversed(&self, handle: Handle) -> impl Iterator<Item = Handle> {
    self.children(handle).into_iter().rev()
  }

  /// The node `handle` is placed under.
  #[expect(
    clippy::unused_self,
    reason = "every reader holds the records, and structure is asked of it as of the rest"
  )]
  pub(crate) fn parent(&self, handle: Handle) -> Option<Handle> {
    Scene::<Terminal>::read(|scene| scene.placed_parent(handle))
  }

  /// The path from `handle` up to the root, `handle` first.
  #[expect(
    clippy::unused_self,
    reason = "every reader holds the records, and structure is asked of it as of the rest"
  )]
  pub(crate) fn ancestors(&self, handle: Handle) -> Vec<Handle> {
    Scene::<Terminal>::read(|scene| {
      let mut path = Vec::new();
      let mut current = Some(handle);

      while let Some(visited) = current {
        path.push(visited);
        current = scene.placed_parent(visited);
      }

      path
    })
  }

  /// Whether walking up from `handle` reaches the root.
  pub(crate) fn is_reachable(&self, handle: Handle) -> bool {
    Scene::<Terminal>::read(|scene| {
      let mut current = Some(handle);

      while let Some(visited) = current {
        if visited == self.root {
          return true;
        }

        current = scene.placed_parent(visited);
      }

      false
    })
  }

  /// The nearest laid-out ancestor of `handle`: the node whose taffy children include
  /// it.
  pub(crate) fn layout_parent(&self, handle: Handle) -> Option<Handle> {
    Scene::<Terminal>::read(|scene| {
      let mut current = scene.placed_parent(handle);

      while let Some(visited) = current {
        if self
          .record(visited)
          .is_some_and(|record| record.kind.is_laid_out())
        {
          return Some(visited);
        }

        current = scene.placed_parent(visited);
      }

      None
    })
  }

  /// The laid-out descendants of `handle` as the layout tree sees them: anchors and
  /// scopes flattened into their content, overlays left out.
  pub(crate) fn layout_children(&self, handle: Handle) -> Vec<Handle> {
    let mut out = Vec::new();

    self.collect_layout_children(handle, &mut out);

    out
  }

  fn collect_layout_children(&self, handle: Handle, out: &mut Vec<Handle>) {
    for child in self.children(handle) {
      let Some(record) = self.record(child)
      else {
        continue;
      };

      if record.is_overlay() {
        continue;
      }

      if record.kind.is_laid_out() {
        out.push(child);
      }
      else {
        self.collect_layout_children(child, out);
      }
    }
  }

  /// The nearest control at or above `handle` in the scene, a caption kept beside a
  /// control answering to that control.
  #[expect(
    clippy::unused_self,
    reason = "every reader holds the records, and the control is asked of it as of the rest"
  )]
  pub(crate) fn enclosing_control(&self, handle: Handle) -> Option<Handle> {
    Scene::<Terminal>::read(|scene| scene.enclosing_control(handle))
  }

  /// Notes that the structure the scene places changed: the layout tree follows it
  /// and the frame repaints.
  pub(crate) const fn note_structure(&mut self) {
    self.restyle.move_structure();
    self.paint_dirty = true;
  }

  /// Every node in the display: reachable from the root through open overlays alone.
  pub(crate) fn displayed(&self) -> FxHashSet<Handle> {
    let mut displayed = FxHashSet::default();
    let mut stack = vec![self.root];

    Scene::<Terminal>::read(|scene| {
      while let Some(current) = stack.pop() {
        let Some(record) = self.record(current)
        else {
          continue;
        };

        if record.is_overlay() && !record.is_open_overlay() {
          continue;
        }

        displayed.insert(current);
        stack.extend(scene.placed_children(current));
      }
    });

    displayed
  }
}
