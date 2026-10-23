//! The tab order: each node's place in it, and the traversal from one stop to the next.

use rustc_hash::FxHashMap;

use super::{Mirror, Scene};
use crate::spine::{FocusOrder, NodeId, Realization};

/// A node the tab order weighs: its place in focus traversal and its position in
/// document order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::spine) struct TabStop {
  /// The node.
  pub(in crate::spine) node: NodeId,
  /// Its place in focus traversal.
  pub(in crate::spine) order: FocusOrder,
  /// Its position in document order.
  pub(in crate::spine) document: usize,
}

impl TabStop {
  /// The tab order of `stops`: positioned ones first by position, then the rest in
  /// document order, each tie in document order; a stop that skips, or that `reachable`
  /// rejects, as the topmost modal does a node outside it, is left out.
  pub(in crate::spine) fn order(
    mut stops: Vec<Self>,
    reachable: impl Fn(NodeId) -> bool,
  ) -> Vec<NodeId> {
    stops.retain(|stop| stop.order != FocusOrder::Skip);
    stops.sort_by_key(|stop| {
      (
        match stop.order {
          FocusOrder::Position(position) => (0, position),
          FocusOrder::Auto | FocusOrder::Skip => (1, 0),
        },
        stop.document,
      )
    });

    stops
      .into_iter()
      .map(|stop| stop.node)
      .filter(|node| reachable(*node))
      .collect()
  }
}

impl<P: Realization> Scene<P> {
  /// Records `node`'s place in focus traversal as the platform was given it at
  /// creation; `None` keeps it out of focus.
  pub fn note_focus_order(&mut self, node: NodeId, order: Option<FocusOrder>) {
    if let Some(record) = self.record_mut(node) {
      record.focus_order = order;
    }
  }

  /// Replaces `node`'s place in focus traversal and mirrors it; `None` takes it out of
  /// focus.
  pub fn set_focus_order(&mut self, node: NodeId, order: Option<FocusOrder>) {
    let Some(record) = self.record_mut(node)
    else {
      return;
    };

    record.focus_order = order;
    self.mirrors.push(Mirror::FocusOrder { node, order });
  }

  /// `node`'s place in focus traversal; `None` for a node focus never reaches.
  #[inline(always)]
  #[must_use]
  pub fn focus_order(&self, node: NodeId) -> Option<FocusOrder> {
    self.record(node).and_then(|record| record.focus_order)
  }

  /// The node that takes focus for the subtree `node` roots: `node` itself where it
  /// has a place in focus traversal, else the first node beneath it with one, in
  /// document order, a segment's content counting as beneath its start; `node` where
  /// none has.
  pub fn focus_holder_within(&self, node: NodeId) -> NodeId {
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
      let Some(record) = self.record(current)
      else {
        continue;
      };

      if record.focus_order.is_some() {
        return current;
      }

      if record.is_segment_start() {
        stack.extend(self.segment_nodes(current).into_iter().rev());
      }
      else {
        stack.extend(self.children_reversed(current));
      }
    }

    node
  }

  /// The tab stops under `root` in traversal order, with `overlays` the open overlays
  /// in the order they opened: positioned ones first by position, then the rest in
  /// document order; closed overlays and the subtrees `hidden` answers for skipped, and
  /// the topmost modal containing the order with the overlays opened after it.
  #[must_use]
  pub fn tab_order(
    &self,
    root: NodeId,
    overlays: &[NodeId],
    hidden: impl Fn(NodeId) -> bool,
  ) -> Vec<NodeId> {
    let mut stops = Vec::new();
    let mut stack = vec![root];
    let mut document = 0;

    while let Some(current) = stack.pop() {
      let Some(record) = self.record(current)
      else {
        continue;
      };

      if record.overlay.is_some_and(|overlay| !overlay.open) || hidden(current) {
        continue;
      }

      if let Some(order) = record.focus_order {
        stops.push(TabStop {
          node: current,
          order,
          document,
        });
      }

      document += 1;
      stack.extend(self.children_reversed(current));
    }

    let overlays = self.overlay_stack(overlays);

    TabStop::order(stops, |node| overlays.reaches(|| self.ancestry(node)))
  }

  /// The stop `step` places from `from` along `order`, the tab stops under `root` in
  /// traversal order, wrapping at both ends.
  ///
  /// From a node that is no stop, such as one disabled while it held focus, the nearest
  /// stop after it in document order is taken, or before it for a negative `step`; from
  /// no node, the first stop, or the last for a negative `step`.
  #[must_use]
  pub fn tab_from(
    &self,
    root: NodeId,
    order: &[NodeId],
    from: Option<NodeId>,
    step: isize,
  ) -> Option<NodeId> {
    let (first, last) = (*order.first()?, *order.last()?);

    if let Some(index) = from.and_then(|from| order.iter().position(|stop| *stop == from)) {
      let count = isize::try_from(order.len()).ok()?;
      let index = isize::try_from(index).ok()?;

      return usize::try_from((index + step).rem_euclid(count))
        .ok()
        .and_then(|index| order.get(index).copied());
    }

    let positions = self.document_positions(root);
    let position = |node: NodeId| positions.get(&node).copied();
    let at = from.and_then(position);
    let beside = at.and_then(|at| {
      if step >= 0 {
        order
          .iter()
          .copied()
          .find(|stop| position(*stop).is_some_and(|stop| stop > at))
      }
      else {
        order
          .iter()
          .rev()
          .copied()
          .find(|stop| position(*stop).is_some_and(|stop| stop < at))
      }
    });

    Some(beside.unwrap_or(if step >= 0 { first } else { last }))
  }

  /// Every node under `root` with its position in document order.
  fn document_positions(&self, root: NodeId) -> FxHashMap<NodeId, usize> {
    let mut positions = FxHashMap::default();
    let mut stack = vec![root];

    while let Some(current) = stack.pop() {
      if !self.contains(current) {
        continue;
      }

      positions.insert(current, positions.len());
      stack.extend(self.children_reversed(current));
    }

    positions
  }
}

#[cfg(test)]
mod tests {
  use rustc_hash::FxHashMap;

  use super::TabStop;
  use crate::spine::{
    FocusOrder,
    NodeId,
    Scene,
    scene::overlays::{OpenOverlay, OverlayStack},
    testing::{TestBox, TestPlatform, TestScene},
  };

  /// The node taking focus for a segment is in the segment's content.
  #[test]
  fn the_focus_holder_of_a_segment_is_in_its_content() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let (start, _) = Scene::<TestPlatform>::create_segment();
    let plain = TestBox::node();
    let focusable = TestBox::node();

    TestScene::with(|scene| {
      scene.attach(parent, start, None);
      scene.insert_in_segment(start, plain, None);
      scene.insert_in_segment(start, focusable, None);
      scene.note_focus_order(focusable, Some(FocusOrder::Auto));
    });

    assert_eq!(
      TestScene::with(|scene| scene.focus_holder_within(start)),
      focusable
    );
    assert_eq!(
      TestScene::with(|scene| scene.enclosing_segment(focusable)),
      Some(start)
    );
  }

  /// The tab order puts positioned stops first by position, then the rest in document
  /// order, leaving out the stops that skip and those the last modal does not contain
  /// with the overlays opened after it, whatever order they were built in.
  #[test]
  fn the_tab_order_follows_positions_documents_and_the_last_modal() {
    let [
      root,
      first,
      second,
      positioned,
      skipped,
      earlier,
      later,
      inside_earlier,
      inside_later,
    ] = std::array::from_fn(|index| NodeId::new(u32::try_from(index).unwrap(), 0));
    let parents: FxHashMap<NodeId, NodeId> = [
      (first, root),
      (second, root),
      (positioned, root),
      (skipped, root),
      (earlier, root),
      (later, root),
      (inside_earlier, earlier),
      (inside_later, later),
    ]
    .into_iter()
    .collect();
    let ancestry =
      |node| std::iter::successors(Some(node), |node| parents.get(node).copied()).collect();
    let stop = |node, order, document| TabStop {
      node,
      order,
      document,
    };
    let stops = vec![
      stop(inside_later, FocusOrder::Auto, 8),
      stop(first, FocusOrder::Auto, 1),
      stop(positioned, FocusOrder::Position(2), 3),
      stop(second, FocusOrder::Auto, 2),
      stop(skipped, FocusOrder::Skip, 4),
      stop(earlier, FocusOrder::Position(1), 6),
      stop(inside_earlier, FocusOrder::Auto, 5),
    ];
    let order = |open: &[(NodeId, bool)]| {
      let overlays = OverlayStack::new(open.iter().map(|&(node, modal)| OpenOverlay {
        node,
        modal,
        anchor: None,
      }));

      TabStop::order(stops.clone(), |node| overlays.reaches(|| ancestry(node)))
    };

    assert_eq!(
      order(&[]),
      [
        earlier,
        positioned,
        first,
        second,
        inside_earlier,
        inside_later
      ]
    );
    assert_eq!(
      order(&[(later, true), (earlier, true)]),
      [earlier, inside_earlier],
      "the modal opened last wins"
    );
    assert_eq!(order(&[(later, true)]), [inside_later]);
    assert_eq!(
      order(&[(earlier, true), (later, false)]),
      [earlier, inside_earlier, inside_later],
      "an overlay opened after the modal stays reachable"
    );
  }
}
