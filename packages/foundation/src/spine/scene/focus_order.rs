//! The tab order: each node's place in it, and the traversal from one stop to the next.

use rustc_hash::FxHashMap;

use super::{Mirror, Scene};
use crate::spine::{FocusOrder, NodeId, Platform};

impl<P: Platform> Scene<P> {
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
    let mut positioned = Vec::new();
    let mut ordered = Vec::new();
    let mut stack = vec![root];

    while let Some(current) = stack.pop() {
      let Some(record) = self.record(current)
      else {
        continue;
      };

      if record.overlay.is_some_and(|overlay| !overlay.open) || hidden(current) {
        continue;
      }

      match record.focus_order {
        Some(FocusOrder::Position(position)) => positioned.push((position, current)),
        Some(FocusOrder::Auto) => ordered.push(current),
        Some(FocusOrder::Skip) | None => {}
      }

      stack.extend(self.children_reversed(current));
    }

    positioned.sort_by_key(|(position, _)| *position);

    let mut order: Vec<NodeId> = positioned.into_iter().map(|(_, node)| node).collect();

    order.extend(ordered);
    order.retain(|node| self.reachable_under_modal(overlays, *node));

    order
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
  use crate::spine::{
    FocusOrder,
    Scene,
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
  /// order, skipping closed overlays and hidden subtrees; the modal opened last contains
  /// it with the overlays opened after it, whatever order they were built in.
  #[test]
  fn the_tab_order_follows_positions_documents_and_the_last_modal() {
    TestPlatform::reset();

    let root = TestBox::node();
    let [first, second, positioned, hidden, closed, earlier, later] =
      std::array::from_fn(|_| TestBox::node());
    let (inside_earlier, inside_later, inside_closed, inside_hidden) = (
      TestBox::node(),
      TestBox::node(),
      TestBox::node(),
      TestBox::node(),
    );

    TestScene::with(|scene| {
      for node in [first, second, positioned, hidden, closed, earlier, later] {
        scene.attach(root, node, None);
      }

      scene.attach(earlier, inside_earlier, None);
      scene.attach(later, inside_later, None);
      scene.attach(closed, inside_closed, None);
      scene.attach(hidden, inside_hidden, None);

      for node in [
        first,
        second,
        inside_earlier,
        inside_later,
        inside_closed,
        inside_hidden,
      ] {
        scene.note_focus_order(node, Some(FocusOrder::Auto));
      }

      scene.note_focus_order(positioned, Some(FocusOrder::Position(1)));

      for (overlay, modal) in [(closed, false), (earlier, true), (later, true)] {
        scene.note_overlay(overlay, modal);
      }
    });

    let order = || {
      TestScene::with(|scene| {
        let overlays = scene.open_overlays().to_vec();

        scene.tab_order(root, &overlays, |node| node == hidden)
      })
    };

    assert_eq!(order(), [positioned, first, second]);

    TestScene::with(|scene| {
      scene.set_overlay_open(later, true, None);
      scene.set_overlay_open(earlier, true, None);
    });

    assert_eq!(order(), [inside_earlier], "the modal opened last wins");

    TestScene::with(|scene| scene.set_overlay_open(earlier, false, None));

    assert_eq!(order(), [inside_later]);
  }
}
