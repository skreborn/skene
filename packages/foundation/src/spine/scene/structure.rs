//! The tree's structure: parents, children and segments, attaching, detaching and
//! destroying, each change mirrored into the platform.

use smallvec::SmallVec;

use super::{
  Mirror,
  NodeRecord,
  Scene,
  placement::{Placing, Structural},
};
use crate::spine::{NodeId, NodeSpec, Part, Realization, dispatch::Crossing};

/// The end of a segment and, while the segment is detached, the content waiting
/// between its anchors.
pub(super) struct SegmentRecord {
  pub(super) end: NodeId,
  pub(super) pending: Vec<NodeId>,
}

/// Why a node leaves its parent: to return while it stays live, or for good.
#[derive(Clone, Copy)]
enum Departure {
  Hide,
  Remove,
}

impl<P: Realization> Scene<P> {
  /// Mints a detached segment: a start and an end anchor, attached and detached as one
  /// with whatever is mounted between them, each created by the platform as
  /// [`realize`](Self::realize) has it.
  ///
  /// Returns `(start, end)`; the start stands for the segment everywhere.
  pub fn create_segment() -> (NodeId, NodeId) {
    let spec = NodeSpec::anchor();
    let start = Self::realize(&spec, None);
    let end = Self::realize(&spec, None);

    Self::with(|scene| {
      if let Some(record) = scene.record_mut(start) {
        record.segment = Some(Box::new(SegmentRecord {
          end,
          pending: Vec::new(),
        }));
      }
    });

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
    self.nodes.iter().filter_map(|(node, record)| {
      (record.parent.is_none() && record.enclosing.is_none() && !record.hidden).then_some(node)
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
    let ancestry = |node: Option<NodeId>| node.map(|node| self.ancestry(node)).unwrap_or_default();

    Crossing::leaving(&ancestry(from), &ancestry(to))
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
  /// platform releases the record of every node destroyed it made.
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

      if let Some(entry) = record.editing {
        self.retired.push(entry);
      }

      if record.overlay.is_some_and(|overlay| overlay.open) {
        self.open_overlays.retain(|open| *open != node);
      }

      self.held.forget(node);

      self.kept_focus.remove(&node);
      self.shortcuts.remove(&node);

      if self.drag.as_ref().is_some_and(|drag| drag.source == node) {
        self.drag = None;
        self.held.touch();
      }

      if let Some(drag) = &mut self.drag
        && drag.target == Some(node)
      {
        drag.target = None;
        self.held.touch();
      }

      if record.realized {
        self.mirrors.push(Mirror::Destroy(node));
      }
    }

    self.tokens.touch();
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
  ///
  /// Called as the control is created, before its caption is attached: a platform
  /// places a caption as it is attached, so a later call moves none.
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
    self.link(parent, child, before, None);

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

      self.link(parent, end, before, None);
    }

    self.return_kept_focus(child);
    self.tokens.touch();
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
      let from = self.parent(node);

      if let Some(from) = from {
        self.sever(from, node);
      }

      let held = from.and_then(|from| self.realized_ancestor(from));

      self.link(parent, node, before, held);
    }

    self.tokens.touch();
  }

  /// Takes `child`, standing under a parent, out of the tree live, keeping the focus
  /// held inside it for its return.
  fn hide(&mut self, child: NodeId) {
    if let Some(focused) = self.focused()
      && self.ancestry(focused).contains(&child)
    {
      self.held.focus(None);
      self.kept_focus.insert(child, focused);
    }

    self.unlink(child, Departure::Hide);

    if let Some(record) = self.record_mut(child) {
      record.hidden = true;
    }

    self.tokens.touch();
  }

  /// Gives the focus `child` kept when it left back to its node, where that node is live
  /// and focus has gone nowhere since.
  fn return_kept_focus(&mut self, child: NodeId) {
    if let Some(focused) = self.kept_focus.remove(&child)
      && self.focused().is_none()
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
  /// its arrival, `held` the node the platform held it under before, if any; `child`
  /// must be detached.
  fn link(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>, held: Option<NodeId>) {
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

    let changes = Structural::arrival(self, child, held);

    self
      .mirrors
      .extend(changes.into_iter().map(Mirror::Structure));
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
      let removed = matches!(departure, Departure::Remove);
      let changes = Structural::departure(self, parent, child, removed);

      self
        .mirrors
        .extend(changes.into_iter().map(Mirror::Structure));
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
