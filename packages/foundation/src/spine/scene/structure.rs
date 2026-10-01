//! The tree's structure: parents, children and segments, attaching, detaching and
//! destroying, each change mirrored into the platform.

use smallvec::SmallVec;

use super::{Mirror, NodeKind, NodeRecord, Scene};
use crate::spine::{NodeId, Part, Platform};

/// The end of a segment and, while the segment is detached, the content waiting
/// between its anchors.
pub(super) struct SegmentRecord {
  pub(super) end: NodeId,
  pub(super) pending: Vec<NodeId>,
}

/// How a node comes to stand under a parent: from nowhere, or from under another.
#[derive(Clone, Copy)]
enum Arrival {
  Attach,
  Relocate,
}

/// Why a node leaves its parent: to return while it stays live, or for good.
#[derive(Clone, Copy)]
enum Departure {
  Hide,
  Remove,
}

impl<P: Platform> Scene<P> {
  /// Mints a detached segment: a start and an end anchor, attached and detached as one
  /// with whatever is mounted between them. The platform realizes the anchors once the
  /// scene is released.
  ///
  /// Returns `(start, end)`; the start stands for the segment everywhere.
  pub fn create_segment() -> (NodeId, NodeId) {
    let (start, end) = Self::with(|scene| {
      let start = scene.create(NodeKind::Anchor, None);
      let end = scene.create(NodeKind::Anchor, None);

      if let Some(record) = scene.record_mut(start) {
        record.segment = Some(Box::new(SegmentRecord {
          end,
          pending: Vec::new(),
        }));
      }

      (start, end)
    });

    P::create_anchor(start);
    P::create_anchor(end);

    (start, end)
  }

  /// The end anchor of the segment `start` begins.
  #[inline(always)]
  #[must_use]
  pub fn segment_end(&self, start: NodeId) -> Option<NodeId> {
    self
      .record(start)
      .and_then(|record| record.segment.as_ref())
      .map(|segment| segment.end)
  }

  /// The parent of `node`.
  #[inline(always)]
  #[must_use]
  pub fn parent(&self, node: NodeId) -> Option<NodeId> {
    self.record(node).and_then(NodeRecord::parent)
  }

  /// Every live node standing in no parent and no segment, in index order: each tree's
  /// root, and each node built and not yet attached; a node hidden while live is none.
  pub fn roots(&self) -> impl Iterator<Item = NodeId> + '_ {
    self
      .entries
      .iter()
      .enumerate()
      .filter_map(|(index, entry)| {
        let record = entry.record.as_ref()?;

        if record.parent.is_some() || record.enclosing.is_some() || record.hidden {
          return None;
        }

        Some(NodeId::new(u32::try_from(index).ok()?, entry.generation))
      })
  }

  /// The children of `node`, in order; empty for a leaf or a stale id.
  #[inline(always)]
  pub fn children(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let mut next = self.record(node).and_then(|record| record.first_child);

    std::iter::from_fn(move || {
      let current = next?;

      next = self.record(current).and_then(|record| record.next_sibling);

      Some(current)
    })
  }

  /// The children of `node`, last first; empty for a leaf or a stale id.
  #[inline(always)]
  pub(super) fn children_reversed(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let mut previous = self.record(node).and_then(|record| record.last_child);

    std::iter::from_fn(move || {
      let current = previous?;

      previous = self
        .record(current)
        .and_then(|record| record.previous_sibling);

      Some(current)
    })
  }

  /// The node after `node` among its parent's children, if any.
  #[inline(always)]
  #[must_use]
  pub fn next_sibling(&self, node: NodeId) -> Option<NodeId> {
    self.record(node).and_then(|record| record.next_sibling)
  }

  /// The node before `node` among its parent's children, if any.
  #[inline(always)]
  #[must_use]
  pub fn previous_sibling(&self, node: NodeId) -> Option<NodeId> {
    self.record(node).and_then(|record| record.previous_sibling)
  }

  /// `node` and the nodes an event at it climbs through, innermost first: the start of
  /// each segment it is mounted in, innermost first, then its parent, and so on.
  pub fn ancestry(&self, node: NodeId) -> SmallVec<[NodeId; 16]> {
    let mut chain = SmallVec::new();
    let mut current = Some(node);

    while let Some(node) = current {
      chain.push(node);
      current = self
        .record(node)
        .and_then(|record| record.enclosing.or(record.parent));
    }

    chain
  }

  /// The start of the segment `node` is mounted in, if it is.
  #[inline(always)]
  #[must_use]
  pub fn enclosing_segment(&self, node: NodeId) -> Option<NodeId> {
    self.record(node).and_then(|record| record.enclosing)
  }

  /// The nodes among `from` and its ancestors that are neither `to` nor among its
  /// ancestors, innermost first: those a move from `from` to `to` leaves.
  pub fn crossed(&self, from: Option<NodeId>, to: Option<NodeId>) -> SmallVec<[NodeId; 16]> {
    let kept = to.map(|to| self.ancestry(to)).unwrap_or_default();

    from
      .map(|from| self.ancestry(from))
      .unwrap_or_default()
      .into_iter()
      .filter(|node| !kept.contains(node))
      .collect()
  }

  /// Places `child` under `parent`, immediately before `before` when given, else last,
  /// moving it from any previous parent; nothing where it already stands there, or
  /// where either is gone.
  ///
  /// A segment start brings its content and its end anchor along; `child` is mounted in
  /// no segment after. A node detached earlier takes back the focus it held when it
  /// left, where focus has gone nowhere since.
  pub fn attach(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    if self.record(parent).is_none() {
      return;
    }

    if self.parent(child).is_none() {
      self.unlink(child, Departure::Hide);
    }

    if let Some(record) = self.record_mut(child) {
      record.enclosing = None;
    }

    self.place(parent, child, before);
  }

  /// Hides `child` from under `parent`, leaving it detached but live and mounted in no
  /// segment, to be attached again.
  ///
  /// A segment start takes its content and its end anchor with it. Focus held inside it
  /// leaves with it, kept for its return.
  pub fn detach(&mut self, parent: NodeId, child: NodeId) {
    if self.parent(child) != Some(parent) {
      return;
    }

    if let Some(record) = self.record_mut(child) {
      record.enclosing = None;
    }

    self.hide(child);
  }

  /// Destroys `node` and its whole subtree, detaching it first if attached; the
  /// platform releases the record of every node destroyed.
  pub fn destroy(&mut self, node: NodeId) {
    self.unlink(node, Departure::Remove);

    let mut stack = vec![node];

    while let Some(node) = stack.pop() {
      let children: SmallVec<[NodeId; 8]> = self.children(node).collect();
      let Some(record) = self.remove(node)
      else {
        continue;
      };

      stack.extend(children);

      if let Some(segment) = record.segment {
        stack.extend(segment.pending);
        stack.push(segment.end);
      }

      if let Some(handlers) = record.handlers {
        self.graveyard.push(*handlers);
      }

      if record.overlay.is_some_and(|overlay| overlay.open) {
        self.open_overlays.retain(|open| *open != node);
      }

      for held in [&mut self.hovered, &mut self.pressed, &mut self.focused] {
        if *held == Some(node) {
          *held = None;
          self.interaction_generation += 1;
        }
      }

      self.kept_focus.remove(&node);

      if self.drag.as_ref().is_some_and(|drag| drag.source == node) {
        self.drag = None;
        self.interaction_generation += 1;
      }

      if let Some(drag) = &mut self.drag
        && drag.target == Some(node)
      {
        drag.target = None;
        self.interaction_generation += 1;
      }

      self.mirrors.push(Mirror::Destroy(node));
    }

    self.definitions_generation += 1;
  }

  /// Mounts `node` in the segment `start` begins, before `before` when given, else
  /// last; a node already in the segment moves.
  pub fn insert_in_segment(&mut self, start: NodeId, node: NodeId, before: Option<NodeId>) {
    let Some((parent, end)) = self.segment_place(start)
    else {
      return;
    };

    if let Some(parent) = parent {
      self.place(parent, node, before.or(Some(end)));
    }
    else {
      if self.parent(node).is_some() {
        self.hide(node);
      }
      else {
        self.unlink(node, Departure::Hide);
      }

      let Some(segment) = self
        .record_mut(start)
        .and_then(|record| record.segment.as_mut())
      else {
        return;
      };

      segment.pending.retain(|pending| *pending != node);

      let index = before
        .and_then(|before| {
          segment
            .pending
            .iter()
            .position(|pending| *pending == before)
        })
        .unwrap_or(segment.pending.len());

      segment.pending.insert(index, node);
    }

    if let Some(record) = self.record_mut(node) {
      record.enclosing = Some(start);
    }
  }

  /// Hides `node`, mounted in the segment `start` begins or built for it, leaving it
  /// detached but live; nothing for a node mounted in another segment.
  pub fn remove_from_segment(&mut self, start: NodeId, node: NodeId) {
    let Some((parent, _)) = self.segment_place(start)
    else {
      return;
    };

    if self
      .record(node)
      .and_then(|record| record.enclosing)
      .is_some_and(|enclosing| enclosing != start)
    {
      return;
    }

    if let Some(parent) = parent
      && self.parent(node) == Some(parent)
    {
      self.detach(parent, node);

      return;
    }

    if let Some(segment) = self
      .record_mut(start)
      .and_then(|record| record.segment.as_mut())
    {
      segment.pending.retain(|pending| *pending != node);
    }

    if let Some(record) = self.record_mut(node) {
      record.enclosing = None;
      record.hidden = true;
    }
  }

  /// The nodes mounted in the segment `start` begins, in order; a nested segment counts
  /// as its start anchor, its own content and end excluded.
  #[must_use]
  pub fn segment_nodes(&self, start: NodeId) -> Vec<NodeId> {
    let Some((parent, end)) = self.segment_place(start)
    else {
      return Vec::new();
    };
    let Some(parent) = parent
    else {
      return self
        .record(start)
        .and_then(|record| record.segment.as_ref())
        .map(|segment| segment.pending.clone())
        .unwrap_or_default();
    };
    let mut nodes = Vec::new();
    let mut nested_end = None;

    for child in self
      .children(parent)
      .skip_while(|child| *child != start)
      .skip(1)
      .take_while(|child| *child != end)
    {
      if let Some(until) = nested_end {
        if child == until {
          nested_end = None;
        }

        continue;
      }

      nodes.push(child);
      nested_end = self.segment_end(child);
    }

    nodes
  }

  /// Destroys everything mounted in the segment `start` begins.
  pub fn clear_segment(&mut self, start: NodeId) {
    for node in self.segment_nodes(start) {
      self.destroy(node);
    }
  }

  /// Records that the control `control` keeps its caption, its `Label` part, beside it:
  /// placed before it among its siblings rather than inside it.
  pub fn keep_caption_beside(&mut self, control: NodeId) {
    if let Some(record) = self.record_mut(control) {
      record.caption_beside = true;
    }
  }

  /// The caption the control `control` keeps beside it: its child recorded as its
  /// `Label` part.
  #[must_use]
  pub fn caption_beside(&self, control: NodeId) -> Option<NodeId> {
    self
      .record(control)
      .filter(|record| record.caption_beside)?;
    self.label_child(control)
  }

  /// The child of `control` recorded as its `Label` part.
  fn label_child(&self, control: NodeId) -> Option<NodeId> {
    self
      .children(control)
      .find(|child| self.part(*child) == Some(Part::Label))
  }

  /// The children of `node` as a platform places them, in order: a caption kept beside
  /// its control stands before the control among its siblings, not inside it.
  pub fn placed_children(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let caption = self.caption_beside(node);
    let mut next = self.record(node).and_then(|record| record.first_child);
    let mut held = None;

    std::iter::from_fn(move || {
      if let Some(child) = held.take() {
        return Some(child);
      }

      loop {
        let current = next?;
        let record = self.record(current);

        next = record.and_then(|record| record.next_sibling);

        if Some(current) == caption {
          continue;
        }

        let beside = record
          .filter(|record| record.caption_beside)
          .and_then(|_| self.label_child(current));

        return Some(match beside {
          Some(label) => {
            held = Some(current);

            label
          }
          None => current,
        });
      }
    })
  }

  /// The node `node` is placed under: its parent, or its control's parent where it is
  /// the caption the control keeps beside it.
  #[must_use]
  pub fn placed_parent(&self, node: NodeId) -> Option<NodeId> {
    let parent = self.parent(node)?;

    if self.caption_beside(parent) == Some(node) {
      return self.parent(parent);
    }

    Some(parent)
  }

  /// The parent the segment's content sits under (`None` while detached) and its end.
  fn segment_place(&self, start: NodeId) -> Option<(Option<NodeId>, NodeId)> {
    let record = self.record(start)?;
    let segment = record.segment.as_ref()?;

    Some((record.parent, segment.end))
  }

  /// Places `child` under `parent`, immediately before `before` when given, else last,
  /// keeping the segment it is mounted in; a segment start brings its content and its
  /// end anchor along.
  fn place(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    if self.record(parent).is_none()
      || self.record(child).is_none()
      || self.stands_before(parent, child, before)
    {
      return;
    }

    if self.parent(child).is_some() {
      self.relocate(parent, child, before);

      return;
    }

    self.unlink(child, Departure::Hide);
    self.link(parent, child, before, Arrival::Attach);

    if let Some(record) = self.record_mut(child) {
      record.hidden = false;
    }

    let segment = self
      .record_mut(child)
      .and_then(|record| record.segment.as_mut())
      .map(|segment| (segment.end, std::mem::take(&mut segment.pending)));

    if let Some((end, pending)) = segment {
      for node in pending {
        self.place(parent, node, before);
      }

      self.link(parent, end, before, Arrival::Attach);
    }

    self.return_kept_focus(child);
    self.definitions_generation += 1;
  }

  /// Moves `child`, standing under a parent, under `parent` immediately before `before`,
  /// else last: a segment start with the run of its content and its end anchor, in
  /// order.
  fn relocate(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    let mut run: SmallVec<[NodeId; 4]> = SmallVec::new();

    run.push(child);

    if let Some(end) = self.segment_end(child) {
      let mut next = self.next_sibling(child);

      while let Some(node) = next {
        run.push(node);

        if node == end {
          break;
        }

        next = self.next_sibling(node);
      }
    }

    for node in run {
      if let Some(from) = self.parent(node) {
        self.sever(from, node);
      }

      self.link(parent, node, before, Arrival::Relocate);
    }

    self.definitions_generation += 1;
  }

  /// Takes `child`, standing under a parent, out of the tree live, keeping the focus
  /// held inside it for its return.
  fn hide(&mut self, child: NodeId) {
    if let Some(focused) = self.focused
      && self.ancestry(focused).contains(&child)
    {
      self.focused = None;
      self.kept_focus.insert(child, focused);
      self.interaction_generation += 1;
    }

    self.unlink(child, Departure::Hide);

    if let Some(record) = self.record_mut(child) {
      record.hidden = true;
    }

    self.definitions_generation += 1;
  }

  /// Gives the focus `child` kept when it left back to its node, where that node is live
  /// and focus has gone nowhere since.
  fn return_kept_focus(&mut self, child: NodeId) {
    if let Some(focused) = self.kept_focus.remove(&child)
      && self.focused.is_none()
      && self.contains(focused)
    {
      self.mirrors.push(Mirror::Focus(focused));
    }
  }

  /// Whether `child` stands under `parent` immediately before `before`, last when
  /// `None`: a segment start by its end anchor, its content between.
  fn stands_before(&self, parent: NodeId, child: NodeId, before: Option<NodeId>) -> bool {
    let Some(record) = self.record(child)
    else {
      return false;
    };

    if record.parent != Some(parent)
      || record
        .segment
        .as_ref()
        .is_some_and(|segment| !segment.pending.is_empty())
    {
      return false;
    }

    let last = record.segment.as_ref().map_or(child, |segment| segment.end);

    self
      .record(last)
      .is_some_and(|record| record.parent == Some(parent) && record.next_sibling == before)
  }

  /// Inserts `child` into `parent`'s children before `before` (else last) and mirrors
  /// its arrival; `child` must be detached.
  fn link(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>, arrival: Arrival) {
    if self.record(child).is_none() || self.record(parent).is_none() {
      return;
    }

    let before = before.filter(|before| {
      self
        .record(*before)
        .is_some_and(|record| record.parent == Some(parent))
    });
    let previous = match before {
      Some(before) => self.previous_sibling(before),
      None => self.record(parent).and_then(|record| record.last_child),
    };

    if let Some(record) = self.record_mut(child) {
      record.parent = Some(parent);
      record.previous_sibling = previous;
      record.next_sibling = before;
    }

    match previous {
      Some(previous) => {
        if let Some(record) = self.record_mut(previous) {
          record.next_sibling = Some(child);
        }
      }
      None => {
        if let Some(record) = self.record_mut(parent) {
          record.first_child = Some(child);
        }
      }
    }

    match before {
      Some(before) => {
        if let Some(record) = self.record_mut(before) {
          record.previous_sibling = Some(child);
        }
      }
      None => {
        if let Some(record) = self.record_mut(parent) {
          record.last_child = Some(child);
        }
      }
    }

    self.mirrors.push(match arrival {
      Arrival::Attach => Mirror::Attach {
        parent,
        child,
        before,
      },
      Arrival::Relocate => Mirror::Relocate {
        parent,
        child,
        before,
      },
    });
  }

  /// Removes `child` from under its parent, if any, and mirrors its departure, or from
  /// the pending content of the detached segment it waits in; a segment start pulls its
  /// content and end anchor back into the pending list.
  fn unlink(&mut self, child: NodeId, departure: Departure) {
    let Some(record) = self.record(child)
    else {
      return;
    };
    let Some(parent) = record.parent
    else {
      if let Some(segment) = record
        .enclosing
        .and_then(|start| self.record_mut(start))
        .and_then(|start| start.segment.as_mut())
      {
        segment.pending.retain(|pending| *pending != child);
      }

      return;
    };

    if record.is_segment_start() {
      self.unlink_segment(parent, child, departure);

      return;
    }

    self.unlink_one(parent, child, departure);
  }

  /// Removes one node from under `parent` and mirrors its departure.
  fn unlink_one(&mut self, parent: NodeId, child: NodeId, departure: Departure) {
    self.sever(parent, child);

    if self.contains(parent) && self.contains(child) {
      self.mirrors.push(match departure {
        Departure::Hide => Mirror::Hide { parent, child },
        Departure::Remove => Mirror::Remove { parent, child },
      });
    }
  }

  /// Removes one node from under `parent`'s children, mirroring nothing.
  fn sever(&mut self, parent: NodeId, child: NodeId) {
    let links = self
      .record(child)
      .filter(|record| record.parent == Some(parent))
      .map(|record| (record.previous_sibling, record.next_sibling));

    if let Some((previous, next)) = links {
      match previous {
        Some(previous) => {
          if let Some(record) = self.record_mut(previous) {
            record.next_sibling = next;
          }
        }
        None => {
          if let Some(record) = self.record_mut(parent) {
            record.first_child = next;
          }
        }
      }

      match next {
        Some(next) => {
          if let Some(record) = self.record_mut(next) {
            record.previous_sibling = previous;
          }
        }
        None => {
          if let Some(record) = self.record_mut(parent) {
            record.last_child = previous;
          }
        }
      }
    }

    if let Some(child_record) = self.record_mut(child) {
      child_record.parent = None;
      child_record.previous_sibling = None;
      child_record.next_sibling = None;
    }
  }

  /// Removes a segment start, its content and its end anchor from under `parent`, and
  /// keeps the content pending; a nested segment is unlinked as one, keeping its own
  /// content pending in turn.
  fn unlink_segment(&mut self, parent: NodeId, start: NodeId, departure: Departure) {
    let content = self.segment_nodes(start);
    let end = self.segment_end(start);

    self.unlink_one(parent, start, departure);

    for node in &content {
      self.unlink(*node, departure);

      if let Some(record) = self.record_mut(*node) {
        record.enclosing = Some(start);
      }
    }

    if let Some(end) = end {
      self.unlink_one(parent, end, departure);
    }

    if let Some(segment) = self
      .record_mut(start)
      .and_then(|record| record.segment.as_mut())
    {
      segment.pending = content;
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::spine::{
    NodeId,
    NodeKind,
    Part,
    Scene,
    testing::{Call, TestBox, TestPlatform, TestScene, children},
  };

  /// A node waiting in a detached segment and attached elsewhere leaves the segment: the
  /// segment's destruction takes it no more.
  #[test]
  fn attaching_a_waiting_node_takes_it_from_its_segment() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let node = TestBox::node();
    let (start, _) = Scene::<TestPlatform>::create_segment();

    TestScene::with(|scene| {
      scene.insert_in_segment(start, node, None);
      scene.attach(parent, node, None);
      scene.destroy(start);
    });

    assert!(TestScene::with(|scene| scene.record(node).is_some()));
    assert_eq!(children(parent), [node]);
  }

  /// Removing a node from a segment that does not hold it leaves it to the one that
  /// does, which gives it up once it is attached elsewhere.
  #[test]
  fn removing_from_another_segment_changes_nothing() {
    TestPlatform::reset();

    let elsewhere = TestBox::node();
    let node = TestBox::node();
    let (holding, _) = Scene::<TestPlatform>::create_segment();
    let (other, _) = Scene::<TestPlatform>::create_segment();

    TestScene::with(|scene| {
      scene.insert_in_segment(holding, node, None);
      scene.remove_from_segment(other, node);
      scene.attach(elsewhere, node, None);
      scene.destroy(holding);
    });

    assert!(TestScene::with(|scene| scene.record(node).is_some()));
    assert_eq!(children(elsewhere), [node]);
  }

  /// A node standing between a segment's anchors is its content: hidden with it, and
  /// taken from it when attached elsewhere.
  #[test]
  fn what_stands_between_the_anchors_is_the_segments() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let elsewhere = TestBox::node();
    let node = TestBox::node();
    let (start, end) = Scene::<TestPlatform>::create_segment();

    TestScene::with(|scene| {
      scene.attach(parent, start, None);
      scene.attach(parent, node, Some(end));
      scene.detach(parent, start);
    });

    assert_eq!(TestScene::with(|scene| scene.segment_nodes(start)), [node]);

    TestScene::with(|scene| {
      scene.attach(elsewhere, node, None);
      scene.destroy(start);
    });

    assert!(TestScene::with(|scene| scene.record(node).is_some()));
    assert_eq!(children(elsewhere), [node]);
  }

  /// Attaching to a destroyed parent, or attaching a destroyed child, changes nothing:
  /// the child stays where it stood.
  #[test]
  fn attach_needs_both_nodes_live() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let gone = TestBox::node();
    let child = TestBox::node();

    TestScene::with(|scene| {
      scene.attach(parent, child, None);
      scene.destroy(gone);
    });
    TestPlatform::take_log();
    TestScene::with(|scene| {
      scene.attach(gone, child, None);
      scene.attach(parent, gone, None);
    });

    assert_eq!(TestScene::with(|scene| scene.parent(child)), Some(parent));
    assert_eq!(children(parent), [child]);
    assert_eq!(TestPlatform::take_log(), []);
  }

  /// Attaching an attached node moves it from its previous parent, in the scene and in
  /// the platform.
  #[test]
  fn attach_unlinks_the_previous_parent() {
    TestPlatform::reset();

    let first = TestBox::node();
    let second = TestBox::node();
    let child = TestBox::node();

    TestScene::with(|scene| scene.attach(first, child, None));
    TestPlatform::take_log();
    TestScene::with(|scene| scene.attach(second, child, None));

    assert_eq!(TestScene::with(|scene| scene.parent(child)), Some(second));
    assert_eq!(children(first), []);
    assert_eq!(children(second), [child]);
    assert_eq!(
      TestPlatform::take_log(),
      [Call::Relocate(second, child, None)]
    );
  }

  /// `before` places a child ahead of a sibling, in the scene and in the platform.
  #[test]
  fn attach_before_orders_the_children() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let first = TestBox::node();
    let second = TestBox::node();

    TestScene::with(|scene| {
      scene.attach(parent, second, None);
      scene.attach(parent, first, Some(second));
    });

    assert_eq!(children(parent), [first, second]);
    assert_eq!(
      TestPlatform::take_log(),
      [
        Call::Attach(parent, second, None),
        Call::Attach(parent, first, Some(second)),
      ]
    );
  }

  /// Attaching a node where it already stands tells the platform nothing, and moving it
  /// does.
  #[test]
  fn attach_in_place_is_a_no_op() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let first = TestBox::node();
    let second = TestBox::node();

    TestScene::with(|scene| {
      scene.attach(parent, first, None);
      scene.attach(parent, second, None);
    });
    TestPlatform::take_log();
    TestScene::with(|scene| {
      scene.attach(parent, first, Some(second));
      scene.attach(parent, second, None);
    });

    assert_eq!(TestPlatform::take_log(), []);

    TestScene::with(|scene| scene.attach(parent, first, None));

    assert_eq!(children(parent), [second, first]);
    assert_eq!(
      TestPlatform::take_log(),
      [Call::Relocate(parent, first, None)]
    );
  }

  /// Detaching a node hides it, no root while hidden, and attaching it again gives it
  /// back from nowhere.
  #[test]
  fn detach_hides_and_attach_gives_back() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let child = TestBox::node();

    TestScene::with(|scene| scene.attach(parent, child, None));
    TestPlatform::take_log();
    TestScene::with(|scene| scene.detach(parent, child));

    assert_eq!(TestPlatform::take_log(), [Call::Hide(parent, child)]);
    assert!(
      !TestScene::with(|scene| scene.roots().any(|root| root == child)),
      "a hidden node is no root"
    );

    TestScene::with(|scene| scene.attach(parent, child, None));

    assert_eq!(
      TestPlatform::take_log(),
      [Call::Attach(parent, child, None)]
    );
  }

  /// Moving a segment relocates its start, its content and its end as one run, in
  /// order.
  #[test]
  fn moving_a_segment_relocates_its_run_in_order() {
    TestPlatform::reset();

    let first = TestBox::node();
    let second = TestBox::node();
    let (start, end) = Scene::<TestPlatform>::create_segment();
    let content = [TestBox::node(), TestBox::node()];

    TestScene::with(|scene| {
      scene.attach(first, start, None);

      for node in content {
        scene.insert_in_segment(start, node, None);
      }
    });
    TestPlatform::take_log();
    TestScene::with(|scene| scene.attach(second, start, None));

    assert_eq!(children(first), []);
    assert_eq!(children(second), [start, content[0], content[1], end]);
    assert_eq!(
      TestPlatform::take_log(),
      [
        Call::Relocate(second, start, None),
        Call::Relocate(second, content[0], None),
        Call::Relocate(second, content[1], None),
        Call::Relocate(second, end, None),
      ]
    );
  }

  /// Hiding a subtree holding focus clears it, and attaching the subtree again gives it
  /// back where focus has gone nowhere since.
  #[test]
  fn a_hidden_subtree_keeps_its_focus_for_its_return() {
    TestPlatform::reset();

    let root = TestBox::node();
    let panel = TestBox::node();
    let field = TestBox::node();
    let other = TestBox::node();

    TestScene::with(|scene| {
      scene.attach(root, panel, None);
      scene.attach(panel, field, None);
      scene.attach(root, other, None);
      let _ = scene.move_focus(Some(field));
      scene.detach(root, panel);
    });

    assert_eq!(TestScene::with(|scene| scene.focused()), None);

    TestPlatform::take_log();
    TestScene::with(|scene| scene.attach(root, panel, None));

    assert_eq!(
      TestPlatform::take_log(),
      [Call::Attach(root, panel, None), Call::Focus(field)]
    );

    TestScene::with(|scene| {
      let _ = scene.move_focus(Some(field));
      scene.detach(root, panel);
      let _ = scene.move_focus(Some(other));
    });
    TestPlatform::take_log();
    TestScene::with(|scene| scene.attach(root, panel, None));

    assert_eq!(TestPlatform::take_log(), [Call::Attach(root, panel, None)]);
  }

  /// A caption kept beside its control is placed before the control among its
  /// siblings, not inside it.
  #[test]
  fn a_caption_kept_beside_is_placed_before_its_control() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let control = TestBox::node();
    let caption = TestBox::node();
    let track = TestBox::node();
    let sibling = TestBox::node();

    TestScene::with(|scene| {
      scene.attach(parent, control, None);
      scene.attach(parent, sibling, None);
      scene.attach(control, caption, None);
      scene.attach(control, track, None);
      scene.set_part(caption, Part::Label);
    });

    assert_eq!(
      TestScene::with(|scene| scene.placed_children(parent).collect::<Vec<_>>()),
      [control, sibling],
      "a caption kept inside is placed where it stands"
    );

    TestScene::with(|scene| scene.keep_caption_beside(control));

    assert_eq!(
      TestScene::with(|scene| scene.placed_children(parent).collect::<Vec<_>>()),
      [caption, control, sibling]
    );
    assert_eq!(
      TestScene::with(|scene| scene.placed_children(control).collect::<Vec<_>>()),
      [track]
    );
    assert_eq!(
      TestScene::with(|scene| scene.placed_parent(caption)),
      Some(parent)
    );
    assert_eq!(
      TestScene::with(|scene| scene.placed_parent(track)),
      Some(control)
    );
  }

  /// Destroying a node takes its whole subtree, releases every record on the platform,
  /// and retires every id.
  #[test]
  fn destroy_takes_the_subtree_and_releases_every_record() {
    TestPlatform::reset();

    let root = TestBox::node();
    let parent = TestBox::node();
    let child = TestBox::node();
    let grandchild = TestBox::node();
    let nodes = [parent, child, grandchild];

    TestScene::with(|scene| {
      scene.attach(root, parent, None);
      scene.attach(parent, child, None);
      scene.attach(child, grandchild, None);
    });
    TestPlatform::take_log();
    TestScene::with(|scene| scene.destroy(parent));

    let log = TestPlatform::take_log();
    let mut destroyed: Vec<NodeId> = log
      .iter()
      .filter_map(|call| match call {
        Call::Destroy(node) => Some(*node),
        _ => None,
      })
      .collect();

    destroyed.sort_unstable();

    assert_eq!(log[0], Call::Remove(root, nodes[0]));
    assert_eq!(destroyed, nodes);
    assert_eq!(children(root), []);
    assert!(TestScene::with(|scene| scene.record(child).is_none()));

    let reused = TestBox::node();

    assert_eq!(reused.index(), grandchild.index());
    assert_ne!(reused, grandchild);
    assert!(TestScene::with(|scene| scene.record(grandchild).is_none()));
  }

  /// A node destroyed while it waits in a detached segment leaves the segment.
  #[test]
  fn a_destroyed_node_leaves_the_detached_segment_it_waits_in() {
    TestPlatform::reset();

    let (start, _) = Scene::<TestPlatform>::create_segment();
    let first = TestBox::node();
    let second = TestBox::node();

    TestScene::with(|scene| {
      scene.insert_in_segment(start, first, None);
      scene.insert_in_segment(start, second, None);
      scene.destroy(first);
    });

    assert_eq!(
      TestScene::with(|scene| scene.segment_nodes(start)),
      [second]
    );
  }

  /// A segment attaches and detaches with its content and its end anchor.
  #[test]
  fn segments_travel_with_their_content() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let other = TestBox::node();
    let content = TestBox::node();
    let (start, end) = Scene::<TestPlatform>::create_segment();

    TestScene::with(|scene| scene.insert_in_segment(start, content, None));

    assert_eq!(
      TestScene::with(|scene| scene.segment_nodes(start)),
      [content]
    );
    assert_eq!(TestScene::with(|scene| scene.parent(content)), None);

    TestScene::with(|scene| scene.attach(parent, start, None));

    assert_eq!(children(parent), [start, content, end]);
    assert_eq!(
      TestScene::with(|scene| scene.record(start).unwrap().kind()),
      NodeKind::Anchor
    );

    TestScene::with(|scene| scene.attach(other, start, None));

    assert_eq!(children(parent), []);
    assert_eq!(children(other), [start, content, end]);

    TestScene::with(|scene| scene.detach(other, start));

    assert_eq!(children(other), []);
    assert_eq!(
      TestScene::with(|scene| scene.segment_nodes(start)),
      [content]
    );

    TestPlatform::take_log();
    TestScene::with(|scene| scene.destroy(start));

    let mut destroyed: Vec<NodeId> = TestPlatform::take_log()
      .iter()
      .filter_map(|call| match call {
        Call::Destroy(node) => Some(*node),
        _ => None,
      })
      .collect();

    destroyed.sort_unstable();

    assert_eq!(destroyed.len(), 3);
    assert!(TestScene::with(|scene| scene.record(content).is_none()));
    assert!(TestScene::with(|scene| scene.record(end).is_none()));
  }

  /// A segment nested in a detached segment keeps its own content pending, so a
  /// removal from it while detached holds when the outer segment is reattached.
  #[test]
  fn nested_segments_keep_their_own_content_while_detached() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let before = TestBox::node();
    let inner_content = TestBox::node();
    let (outer, outer_end) = Scene::<TestPlatform>::create_segment();
    let (inner, inner_end) = Scene::<TestPlatform>::create_segment();

    TestScene::with(|scene| {
      scene.attach(parent, outer, None);
      scene.insert_in_segment(outer, before, None);
      scene.insert_in_segment(outer, inner, None);
      scene.insert_in_segment(inner, inner_content, None);
    });

    assert_eq!(
      children(parent),
      [outer, before, inner, inner_content, inner_end, outer_end]
    );
    assert_eq!(
      TestScene::with(|scene| scene.segment_nodes(outer)),
      [before, inner]
    );

    TestScene::with(|scene| scene.detach(parent, outer));

    assert_eq!(
      TestScene::with(|scene| scene.segment_nodes(outer)),
      [before, inner]
    );
    assert_eq!(
      TestScene::with(|scene| scene.segment_nodes(inner)),
      [inner_content]
    );

    TestScene::with(|scene| scene.remove_from_segment(inner, inner_content));
    TestScene::with(|scene| scene.attach(parent, outer, None));

    assert_eq!(
      children(parent),
      [outer, before, inner, inner_end, outer_end]
    );
    assert!(TestScene::with(|scene| scene
      .record(inner_content)
      .is_some()));

    TestScene::with(|scene| scene.insert_in_segment(inner, inner_content, None));

    assert_eq!(
      children(parent),
      [outer, before, inner, inner_content, inner_end, outer_end]
    );
  }

  /// One structural change a platform's views make to the scene, over indices into a
  /// pool of boxes and segments.
  #[cfg(not(target_arch = "wasm32"))]
  #[derive(Debug, Clone)]
  enum Change {
    Attach(usize, usize, Option<usize>),
    Detach(usize, usize),
    Insert(usize, usize, Option<usize>),
    Remove(usize, usize),
    Destroy(usize),
  }

  /// The most links a walk of the pool follows before it is taken for a cycle.
  #[cfg(not(target_arch = "wasm32"))]
  const WALK: usize = 64;

  /// The children of `node`, at most [`WALK`] of them.
  #[cfg(not(target_arch = "wasm32"))]
  fn bounded_children(scene: &Scene<TestPlatform>, node: NodeId) -> Vec<NodeId> {
    scene.children(node).take(WALK).collect()
  }

  /// Whether `ancestor` is `node` or stands above it, at most [`WALK`] links up.
  #[cfg(not(target_arch = "wasm32"))]
  fn stands_above(scene: &Scene<TestPlatform>, ancestor: NodeId, node: NodeId) -> bool {
    let mut current = Some(node);

    for _ in 0..WALK {
      let Some(visited) = current
      else {
        return false;
      };

      if visited == ancestor {
        return true;
      }

      current = scene.parent(visited);
    }

    true
  }

  /// The nodes that move with `node`: a segment start's content and end anchor with it,
  /// and a pending content's nested ones.
  #[cfg(not(target_arch = "wasm32"))]
  fn moving(scene: &Scene<TestPlatform>, node: NodeId) -> Vec<NodeId> {
    moving_within(scene, node, WALK)
  }

  /// [`moving`], following at most `depth` pending segments down.
  #[cfg(not(target_arch = "wasm32"))]
  fn moving_within(scene: &Scene<TestPlatform>, node: NodeId, depth: usize) -> Vec<NodeId> {
    let Some(end) = scene.segment_end(node).filter(|_| depth > 0)
    else {
      return vec![node];
    };

    if let Some(parent) = scene.parent(node) {
      return bounded_children(scene, parent)
        .into_iter()
        .skip_while(|child| *child != node)
        .take_while(|child| *child != end)
        .chain([end])
        .collect();
    }

    let pending = scene
      .record(node)
      .and_then(|record| record.segment.as_ref())
      .map(|segment| segment.pending.clone())
      .unwrap_or_default();

    [node]
      .into_iter()
      .chain(
        pending
          .into_iter()
          .flat_map(|pending| moving_within(scene, pending, depth - 1)),
      )
      .chain([end])
      .collect()
  }

  /// Every link the scene keeps agreeing with every other: each live node stands once
  /// among its parent's children, the sibling links run both ways to the ends the
  /// parent names, no node stands under itself, an attached segment's end stands after
  /// its start under the same parent, and a destroyed node is named by nothing.
  #[cfg(not(target_arch = "wasm32"))]
  fn assert_whole(scene: &Scene<TestPlatform>, nodes: &[NodeId]) {
    for &node in nodes {
      let Some(record) = scene.record(node)
      else {
        continue;
      };
      let mut previous = None;
      let mut next = record.first_child;
      let mut count = 0;

      while let Some(child) = next {
        let child_record = scene.record(child).expect("a child is live");

        assert_eq!(child_record.parent, Some(node), "a child names its parent");
        assert_eq!(
          child_record.previous_sibling, previous,
          "siblings link both ways"
        );
        previous = Some(child);
        next = child_record.next_sibling;
        count += 1;

        assert!(count <= nodes.len(), "the children end");
      }

      assert_eq!(
        record.last_child, previous,
        "the last child is the parent's last"
      );

      if let Some(parent) = record.parent {
        assert_eq!(
          bounded_children(scene, parent)
            .into_iter()
            .filter(|child| *child == node)
            .count(),
          1,
          "a node stands once among its parent's children"
        );
      }

      let mut ancestor = record.parent;
      let mut depth = 0;

      while let Some(visited) = ancestor {
        assert_ne!(visited, node, "no node stands under itself");
        depth += 1;

        assert!(depth <= nodes.len(), "the ancestry ends");

        ancestor = scene.parent(visited);
      }

      if let Some(segment) = &record.segment {
        let end = segment.end;

        assert!(scene.record(end).is_some(), "a live start has a live end");
        assert_eq!(
          scene.parent(end),
          record.parent,
          "an end stands with its start"
        );

        if let Some(parent) = record.parent {
          let order = bounded_children(scene, parent);
          let start = order.iter().position(|child| *child == node);
          let end = order.iter().position(|child| *child == end);

          assert!(start < end, "an end stands after its start");
        }
      }
    }
  }

  #[cfg(not(target_arch = "wasm32"))]
  use proptest::prelude::*;

  #[cfg(not(target_arch = "wasm32"))]
  proptest! {
    /// Any run of attaching, detaching, mounting in and out of segments and destroying
    /// keeps the scene's links whole.
    #[test]
    fn any_run_of_changes_keeps_the_scene_whole(
      changes in proptest::collection::vec(
        prop_oneof![
          4 => (0..9_usize, 0..9_usize, proptest::option::of(0..9_usize))
            .prop_map(|(parent, child, before)| Change::Attach(parent, child, before)),
          2 => (0..9_usize, 0..9_usize).prop_map(|(parent, child)| Change::Detach(parent, child)),
          3 => (0..2_usize, 0..9_usize, proptest::option::of(0..9_usize))
            .prop_map(|(segment, node, before)| Change::Insert(segment, node, before)),
          2 => (0..2_usize, 0..9_usize).prop_map(|(segment, node)| Change::Remove(segment, node)),
          1 => (0..9_usize).prop_map(Change::Destroy),
        ],
        0..30,
      ),
    ) {
      TestPlatform::reset();

      let boxes: Vec<NodeId> = (0..5).map(|_| TestBox::node()).collect();
      let segments = [
        Scene::<TestPlatform>::create_segment(),
        Scene::<TestPlatform>::create_segment(),
      ];
      let pool: Vec<NodeId> = boxes
        .iter()
        .copied()
        .chain(segments.iter().map(|(start, _)| *start))
        .chain(segments.iter().map(|(_, end)| *end))
        .collect();

      for change in changes {
        TestScene::with(|scene| {
          let holds = |moved: NodeId, node: NodeId| {
            moving(scene, moved)
              .into_iter()
              .any(|ancestor| stands_above(scene, ancestor, node))
          };

          match change {
            Change::Attach(parent, child, before) => {
              let (parent, child) = (boxes[parent % boxes.len()], pool[child % 7]);
              let before = before.map(|before| pool[before]);

              if !holds(child, parent)
                && before.is_none_or(|before| !moving(scene, child).contains(&before))
              {
                scene.attach(parent, child, before);
              }
            }
            Change::Detach(parent, child) => {
              scene.detach(boxes[parent % boxes.len()], pool[child % 7]);
            }
            Change::Insert(segment, node, before) => {
              let start = segments[segment].0;
              let node = pool[node % 7];
              let parent = scene.parent(start);

              let before = before.map(|before| pool[before]);

              if !moving(scene, node).contains(&start)
                && parent.is_none_or(|parent| !holds(node, parent))
                && before.is_none_or(|before| !moving(scene, node).contains(&before))
              {
                scene.insert_in_segment(start, node, before);
              }
            }
            Change::Remove(segment, node) => {
              scene.remove_from_segment(segments[segment].0, pool[node % 7]);
            }
            Change::Destroy(node) => scene.destroy(pool[node % 7]),
          }

          assert_whole(scene, &pool);
        });
      }
    }
  }
}
