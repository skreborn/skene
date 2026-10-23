//! Overlays: which are open and in what order, what they are placed against, where
//! focus returns as they close, and what a press or a focus move dismisses.

use std::fmt::Debug;

use smallvec::SmallVec;

use super::{Mirror, NodeKind, Scene};
use crate::spine::{NodeId, Realization};

/// An overlay's standing: whether it blocks what lies beneath it while open, whether it
/// is open, the node it opened against, and the node focused when it opened.
#[derive(Debug, Clone, Copy)]
pub(super) struct OverlayRecord {
  pub(super) modal: bool,
  pub(super) open: bool,
  pub(super) anchor: Option<NodeId>,
  pub(super) opener: Option<NodeId>,
}

/// An open overlay as the stack weighs it: the overlay, whether it is modal, and the
/// node it opened against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::spine) struct OpenOverlay {
  /// The overlay.
  pub(in crate::spine) node: NodeId,
  /// Whether it blocks what lies beneath it.
  pub(in crate::spine) modal: bool,
  /// The node it opened against, if any.
  pub(in crate::spine) anchor: Option<NodeId>,
}

/// Open overlays in the order they opened, the last on top: what a press dismisses,
/// what focus leaves and what stays reachable under a modal, read from the ancestries of
/// the nodes they reach.
///
/// An ancestry is a node and the nodes it stands in, itself first, as
/// [`Scene::ancestry`] gives it.
#[derive(Debug, Clone)]
#[must_use]
pub(in crate::spine) struct OverlayStack {
  open: SmallVec<[OpenOverlay; 4]>,
}

impl OverlayStack {
  /// The stack of `open`, in the order they opened.
  pub(in crate::spine) fn new(open: impl IntoIterator<Item = OpenOverlay>) -> Self {
    Self {
      open: open.into_iter().collect(),
    }
  }

  /// The overlays a primary press on the node of `ancestry` dismisses, topmost first; an
  /// empty `ancestry` for a press on no node.
  ///
  /// From the top down, each overlay the press lands outside of—on neither the overlay
  /// nor, for a popup, the anchor it opened from—is dismissed, until the first it lands
  /// inside of or on the anchor of; a modal stops the descent, dismissed itself by a
  /// press outside it.
  #[must_use]
  pub(in crate::spine) fn dismissed_by(&self, ancestry: &[NodeId]) -> Vec<NodeId> {
    let mut dismissed = Vec::new();

    for overlay in self.open.iter().rev() {
      let on_anchor = !overlay.modal
        && overlay
          .anchor
          .is_some_and(|anchor| ancestry.contains(&anchor));

      if ancestry.contains(&overlay.node) || on_anchor {
        break;
      }

      dismissed.push(overlay.node);

      if overlay.modal {
        break;
      }
    }

    dismissed
  }

  /// The popups focus leaves as it moves to the node of `ancestry`: each non-modal
  /// overlay opened against an anchor, where neither the popup nor its anchor holds the
  /// node.
  #[must_use]
  pub(in crate::spine) fn left_by(&self, ancestry: &[NodeId]) -> Vec<NodeId> {
    self
      .open
      .iter()
      .filter(|overlay| {
        !overlay.modal
          && overlay
            .anchor
            .is_some_and(|anchor| !ancestry.contains(&overlay.node) && !ancestry.contains(&anchor))
      })
      .map(|overlay| overlay.node)
      .collect()
  }

  /// The topmost modal.
  #[must_use]
  pub(in crate::spine) fn topmost_modal(&self) -> Option<NodeId> {
    self
      .open
      .iter()
      .rev()
      .find(|overlay| overlay.modal)
      .map(|overlay| overlay.node)
  }

  /// Whether the node `ancestry` gives stays reachable under the topmost modal: inside
  /// that modal, or inside an overlay opened after it; every node is while none is
  /// open, and `ancestry` is asked for only while one is.
  #[must_use]
  pub(in crate::spine) fn reaches(
    &self,
    ancestry: impl FnOnce() -> SmallVec<[NodeId; 16]>,
  ) -> bool {
    let Some(modal) = self.open.iter().rposition(|overlay| overlay.modal)
    else {
      return true;
    };
    let ancestry = ancestry();

    self.open[modal..]
      .iter()
      .any(|overlay| ancestry.contains(&overlay.node))
  }
}

impl<P: Realization> Scene<P> {
  /// Records that `node` is an overlay, closed, `modal` when it blocks what lies beneath
  /// it while open.
  pub fn note_overlay(&mut self, node: NodeId, modal: bool) {
    if let Some(record) = self.record_mut(node) {
      record.overlay = Some(OverlayRecord {
        modal,
        open: false,
        anchor: None,
        opener: None,
      });
    }
  }

  /// Opens or closes the overlay `node`, placed against `anchor` while open, and mirrors
  /// it; an opening puts it on top of every open overlay, as a browser's top layer
  /// stacks, and a node of any other kind is left alone.
  ///
  /// A close of a modal, or of an overlay holding focus, returns focus as a browser's
  /// top layer does: to the node focused when it opened, while that node takes focus,
  /// else to the nearest node taking focus at or above the anchor it opened against.
  pub fn set_overlay_open(&mut self, node: NodeId, open: bool, anchor: Option<NodeId>) {
    let placed = anchor.and_then(|anchor| self.placement(anchor));
    let focused = self.focused();
    let Some(record) = self.record_mut(node)
    else {
      return;
    };
    let Some(overlay) = record.overlay.as_mut()
    else {
      return;
    };
    let was_open = std::mem::replace(&mut overlay.open, open);
    let opened_against = std::mem::replace(&mut overlay.anchor, anchor);
    let modal = overlay.modal;
    let opener = match (was_open, open) {
      (false, true) => {
        overlay.opener = focused;

        None
      }
      (true, false) => overlay.opener.take(),
      _ => None,
    };

    match (was_open, open) {
      (false, true) => self.open_overlays.push(node),
      (true, false) => self.open_overlays.retain(|open| *open != node),
      _ => {}
    }

    self.mirrors.push(Mirror::OverlayOpen {
      node,
      open,
      anchor: placed,
    });

    if was_open && !open {
      self.return_focus(node, modal, opener, opened_against);
    }
  }

  /// Returns focus from the overlay `node` as it closes, where it is `modal` or holds
  /// focus: to `opener` while it takes focus, else to the nearest node taking focus at
  /// or above `anchor`.
  fn return_focus(
    &mut self,
    node: NodeId,
    modal: bool,
    opener: Option<NodeId>,
    anchor: Option<NodeId>,
  ) {
    let inside = self
      .focused()
      .is_some_and(|focused| self.ancestry(focused).contains(&node));

    if !modal && !inside {
      return;
    }

    let stack = self.overlay_stack(&self.open_overlays);
    let takes_focus = |scene: &Self, candidate: NodeId| {
      scene.focus_order(candidate).is_some() && stack.reaches(|| scene.ancestry(candidate))
    };
    let target = opener
      .filter(|opener| takes_focus(self, *opener))
      .or_else(|| {
        anchor.and_then(|anchor| {
          self
            .ancestry(anchor)
            .into_iter()
            .find(|candidate| takes_focus(self, *candidate))
        })
      });

    if let Some(target) = target {
      self.mirrors.push(Mirror::Focus(target));
    }
  }

  /// The node focused when the overlay `node` opened, while it is open.
  #[inline(always)]
  #[must_use]
  pub fn overlay_opener(&self, node: NodeId) -> Option<NodeId> {
    self
      .record(node)
      .and_then(|record| record.overlay)
      .filter(|overlay| overlay.open)
      .and_then(|overlay| overlay.opener)
  }

  /// The node an overlay anchored on `node` is placed against: `node` itself, or for a
  /// segment or a scope, the first node inside it with a box of its own, in document
  /// order; `None` where there is none.
  #[must_use]
  pub fn placement(&self, node: NodeId) -> Option<NodeId> {
    match self.record(node)?.kind {
      NodeKind::Anchor => self
        .segment_nodes(node)
        .into_iter()
        .find_map(|inner| self.placement(inner)),
      NodeKind::Scope => self.children(node).find_map(|child| self.placement(child)),
      NodeKind::Box | NodeKind::Control | NodeKind::Run | NodeKind::Overlay => Some(node),
    }
  }

  /// The node the overlay `node` opened against, while one is.
  #[inline(always)]
  #[must_use]
  pub fn overlay_anchor(&self, node: NodeId) -> Option<NodeId> {
    self
      .record(node)
      .and_then(|record| record.overlay)
      .and_then(|overlay| overlay.anchor)
  }

  /// The overlays among `overlays`, open overlays in the order they opened, that a
  /// primary press at `at` dismisses, topmost first; `at` is `None` for a press on no
  /// node.
  ///
  /// From the top down, each overlay the press lands outside of—on neither the overlay
  /// nor, for a popup, the anchor it opened from—is dismissed, until the first it lands
  /// inside of or on the anchor of; a modal stops the descent, dismissed itself by a
  /// press outside it.
  #[must_use]
  pub fn dismissed_by_press(&self, overlays: &[NodeId], at: Option<NodeId>) -> Vec<NodeId> {
    let ancestry = at.map(|at| self.ancestry(at)).unwrap_or_default();

    self.overlay_stack(overlays).dismissed_by(&ancestry)
  }

  /// The stack `overlays`, overlays in the order they opened, make as plain data.
  pub(in crate::spine) fn overlay_stack(&self, overlays: &[NodeId]) -> OverlayStack {
    OverlayStack::new(overlays.iter().map(|&node| OpenOverlay {
      node,
      modal: self.is_modal(node),
      anchor: self.overlay_anchor(node),
    }))
  }

  /// The open overlays in the order they opened: the last stands on top.
  #[inline(always)]
  pub fn open_overlays(&self) -> &[NodeId] {
    &self.open_overlays
  }

  /// The open popups focus leaves as it moves to `next`: each non-modal overlay opened
  /// against an anchor, where neither the popup nor its anchor holds `next`.
  #[must_use]
  pub fn popups_left_by(&self, next: NodeId) -> Vec<NodeId> {
    self
      .overlay_stack(&self.open_overlays)
      .left_by(&self.ancestry(next))
  }

  /// The node the modal `modal` takes focus to as it opens, as a dialog's focusing steps
  /// choose: its first tab stop in `order`, the tab order, else the modal itself.
  pub fn modal_focus(&self, modal: NodeId, order: &[NodeId]) -> NodeId {
    order
      .iter()
      .copied()
      .find(|stop| self.ancestry(*stop).contains(&modal))
      .unwrap_or(modal)
  }

  /// Whether `node` is a modal overlay.
  #[inline(always)]
  #[must_use]
  pub fn is_modal(&self, node: NodeId) -> bool {
    self
      .record(node)
      .and_then(|record| record.overlay)
      .is_some_and(|overlay| overlay.modal)
  }

  /// The topmost open modal overlay among `overlays`, open overlays in the order they
  /// opened.
  #[must_use]
  pub fn topmost_modal(&self, overlays: &[NodeId]) -> Option<NodeId> {
    self.overlay_stack(overlays).topmost_modal()
  }

  /// Whether `node` stays reachable under the topmost modal among `overlays`, open
  /// overlays in the order they opened: inside that modal, or inside an overlay opened
  /// after it; every node is while none is open.
  #[must_use]
  pub fn reachable_under_modal(&self, overlays: &[NodeId], node: NodeId) -> bool {
    self.overlay_stack(overlays).reaches(|| self.ancestry(node))
  }
}

#[cfg(test)]
mod tests {
  use super::{OpenOverlay, OverlayStack};
  use crate::spine::{
    NodeId,
    NodeKind,
    Scene,
    testing::{TestBox, TestPlatform, TestScene},
  };

  /// A node id for plain data, by its index.
  fn node(index: usize) -> NodeId {
    NodeId::new(u32::try_from(index).unwrap(), 0)
  }

  /// An overlay anchored on a segment or a scope is placed against the first node
  /// inside it with a box of its own, an empty segment standing for none.
  #[test]
  fn a_segment_is_placed_against_its_first_box() {
    TestPlatform::reset();

    let (outer, _) = Scene::<TestPlatform>::create_segment();
    let (empty, _) = Scene::<TestPlatform>::create_segment();
    let scope = TestScene::with(|scene| scene.create(NodeKind::Scope, None));
    let inside = TestBox::node();
    let after = TestBox::node();

    TestScene::with(|scene| {
      scene.attach(scope, inside, None);
      scene.insert_in_segment(outer, empty, None);
      scene.insert_in_segment(outer, scope, None);
      scene.insert_in_segment(outer, after, None);
    });

    assert_eq!(
      TestScene::with(|scene| scene.placement(outer)),
      Some(inside)
    );
    assert_eq!(
      TestScene::with(|scene| scene.placement(scope)),
      Some(inside)
    );
    assert_eq!(TestScene::with(|scene| scene.placement(empty)), None);
    assert_eq!(TestScene::with(|scene| scene.placement(after)), Some(after));
  }

  /// A press dismisses, from the top down, each overlay it lands outside of, until one
  /// it lands inside of or on the anchor of; a modal ends the descent, dismissed itself,
  /// and keeps what opened beneath it.
  #[test]
  fn a_press_dismisses_the_overlays_it_lands_outside_of() {
    let [root, page, beneath, modal, menu, item, submenu] = std::array::from_fn(node);
    let stack = OverlayStack::new([
      OpenOverlay {
        node: beneath,
        modal: false,
        anchor: None,
      },
      OpenOverlay {
        node: modal,
        modal: true,
        anchor: None,
      },
      OpenOverlay {
        node: menu,
        modal: false,
        anchor: None,
      },
      OpenOverlay {
        node: submenu,
        modal: false,
        anchor: Some(item),
      },
    ]);

    assert_eq!(
      stack.dismissed_by(&[item, menu, modal, root]),
      [],
      "on the submenu's anchor"
    );
    assert_eq!(stack.dismissed_by(&[menu, modal, root]), [submenu]);
    assert_eq!(stack.dismissed_by(&[page, root]), [submenu, menu, modal]);
    assert_eq!(stack.dismissed_by(&[]), [submenu, menu, modal]);
  }

  /// The modal opened last is the topmost and contains what stays reachable with the
  /// overlays opened after it; with no modal open, everything is reachable, unasked.
  #[test]
  fn the_modal_opened_last_holds_what_stays_reachable() {
    let [root, page, earlier, later, popup, inside] = std::array::from_fn(node);
    let overlay = |node, modal| OpenOverlay {
      node,
      modal,
      anchor: None,
    };
    let stack = OverlayStack::new([
      overlay(earlier, true),
      overlay(later, true),
      overlay(popup, false),
    ]);

    assert_eq!(stack.topmost_modal(), Some(later));
    assert!(stack.reaches(|| [inside, later, root].into_iter().collect()));
    assert!(stack.reaches(|| [inside, popup, root].into_iter().collect()));
    assert!(!stack.reaches(|| [inside, earlier, root].into_iter().collect()));
    assert!(!stack.reaches(|| [page, root].into_iter().collect()));
    assert!(OverlayStack::new([overlay(popup, false)]).reaches(|| unreachable!()));
  }
}
