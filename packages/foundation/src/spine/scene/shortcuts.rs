//! The window's shortcuts: the key each control or activatable row is activated by from
//! anywhere in the window, and the node a key pressed reaches.

use smallvec::SmallVec;

use super::{NodeKind, Scene};
use crate::spine::{KeyEvent, NodeId, Realization, Shortcut};

impl<P: Realization> Scene<P> {
  /// Records `shortcut` as the key that activates `node`, a control or an activatable
  /// row, from anywhere in the window, until the node is destroyed.
  pub fn note_shortcut(&mut self, node: NodeId, shortcut: Shortcut) {
    if self.contains(node) {
      self.shortcuts.insert(node, shortcut);
    }
  }

  /// The shortcut that activates `node`, if it has one.
  #[inline(always)]
  #[must_use]
  pub fn shortcut(&self, node: NodeId) -> Option<Shortcut> {
    self.shortcuts.get(&node).copied()
  }

  /// The node `event`, pressed at `at`, activates as its shortcut under `root`: the first
  /// in document order whose shortcut it presses that is a control not disabled or an
  /// activatable row, under `root` outside every closed overlay and every subtree
  /// `hidden` answers for, and that the topmost open modal does not exclude, as the tab
  /// order reaches its stops; `None` within a composition, and for a shortcut holding no
  /// modifier but `Shift` while `at` is inside a text entry, whose key it is.
  pub(in crate::spine) fn shortcut_target(
    &self,
    root: NodeId,
    at: Option<NodeId>,
    event: &KeyEvent,
    hidden: impl Fn(NodeId) -> bool,
  ) -> Option<NodeId> {
    if event.composing || self.shortcuts.is_empty() {
      return None;
    }

    let typing = at.is_some_and(|at| self.is_in_entry(at));
    let overlays = self.overlay_stack(&self.open_overlays);
    let reached: SmallVec<[NodeId; 4]> = self
      .shortcuts
      .iter()
      .filter(|(_, shortcut)| event.matches(shortcut) && !(typing && shortcut.is_unmodified()))
      .map(|(node, _)| *node)
      .filter(|node| {
        self.acts_on_shortcut(*node)
          && self.is_shown_under(root, *node, &hidden)
          && overlays.reaches(|| self.ancestry(*node))
      })
      .collect();

    match reached.as_slice() {
      [] => None,
      [only] => Some(*only),
      _ => {
        let positions = self.document_positions(root);

        reached
          .into_iter()
          .min_by_key(|node| positions.get(node).copied().unwrap_or(usize::MAX))
      }
    }
  }

  /// Whether `node` acts on its shortcut: a control not disabled, or an activatable row.
  fn acts_on_shortcut(&self, node: NodeId) -> bool {
    self.record(node).is_some_and(|record| match record.kind {
      NodeKind::Control => record.control.is_some_and(|state| !state.disabled),
      _ => record.is_acting(),
    })
  }

  /// Whether `node` is attached under `root`, no node along the way a closed overlay or
  /// one `hidden` answers for.
  fn is_shown_under(&self, root: NodeId, node: NodeId, hidden: &impl Fn(NodeId) -> bool) -> bool {
    let mut current = Some(node);

    while let Some(visited) = current {
      let Some(record) = self.record(visited)
      else {
        return false;
      };

      if record.overlay.is_some_and(|overlay| !overlay.open) || hidden(visited) {
        return false;
      }

      if visited == root {
        return true;
      }

      current = record.parent;
    }

    false
  }

  /// Whether `node` is a text entry or lies inside one.
  fn is_in_entry(&self, node: NodeId) -> bool {
    let entry = |node: NodeId| self.record(node).is_some_and(|record| record.entry);

    entry(node) || self.enclosing_control(node).is_some_and(entry)
  }
}
