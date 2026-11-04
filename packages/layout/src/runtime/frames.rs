//! The drawn runtime's index of window rectangles: every laid-out node's border box in
//! its window, the cover its paint damages, the reach a hit takes it at, the clips what
//! shows of it and of its content are cut to and whether it takes hits, in paint order,
//! each open overlay's subtree placed where the runtime places it; its walk in paint
//! order, which a platform's painter visits; the topmost node at a point, hit over it;
//! and the damage the next paint repaints, as nodes and the rectangles of the window
//! they cover.

use skene_foundation::{Inset, NodeId, NodeMap, Point, Rect, RunPlacement, Scalar, Scene, Size};
use smallvec::SmallVec;

use super::{Drawn, Kind, Runtime};
use crate::{Leaf, Placed};

/// One laid-out node's place in its window, as [`Frames`] indexes it.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Frame {
  /// The node.
  pub node: NodeId,
  /// Its border box in the window: its location summed down the laid-out nodes above
  /// it, each one's scroll offsets subtracted, an overlay's subtree moved to where the
  /// overlay is placed.
  pub rect: Rect<f32>,
  /// What its paint covers in the window, its border box grown by how far its paint
  /// reaches ([`Drawn::reach`]): what a repaint of it damages; `None` for a node of no
  /// extent.
  pub cover: Option<Rect<f32>>,
  /// Where a hit may take it or what it holds in the window: its border box, united with
  /// its scrollable overflow where it does not clip it.
  pub reach: Rect<f32>,
  /// What shows of it in the window: the padding boxes of every laid-out node above it in
  /// its layer that clips, intersected; `None` where none does.
  pub clip: Option<Rect<f32>>,
  /// What shows of its content and of what it holds in the window: its `clip`, cut to its
  /// padding box where it clips; `None` where neither cuts.
  pub content_clip: Option<Rect<f32>>,
  /// Whether it takes hits and paints: neither it nor a laid-out node above it in its
  /// layer is invisible.
  pub hits: bool,
  /// Whether it lays its runs out as one paragraph, a hit inside it resolved against its
  /// flow rather than its entries ([`Frames::hit`]).
  pub paragraph: bool,
  /// The index of its laid-out parent in its layer; `None` for the root and for an open
  /// overlay.
  pub parent: Option<usize>,
  /// The index past its subtree.
  pub end: usize,
}

/// An open overlay's layer in [`Frames`]: its subtree's entries, painted and hit above
/// the root's and every layer opened before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct Layer {
  /// The overlay.
  pub overlay: NodeId,
  /// Whether the overlay is modal.
  pub modal: bool,
  /// The index of the overlay's own entry, its subtree's first.
  pub start: usize,
  /// The index past its subtree.
  pub end: usize,
}

/// Whether a walk of [`Frames`] goes on into what an entry it entered holds
/// ([`FrameVisitor::enter`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Descend {
  /// Enter the entries beneath it, then leave it.
  Into,
  /// Pass over the entries beneath it, and do not leave it.
  Skip,
}

/// What walks [`Frames`] in paint order ([`Frames::visit`]): each entry entered before
/// what it holds and left after it, and each open overlay's layer announced before its
/// entries.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not visit `Frames` in paint order",
  label = "a painter walks `Frames` through `FrameVisitor`: entering each entry, leaving it after what it holds, and hearing each open overlay's layer before its entries"
)]
pub trait FrameVisitor {
  /// Enters the entry at `index`, `frame`, before every entry beneath it, answering
  /// whether the walk goes on into them.
  fn enter(&mut self, index: usize, frame: &Frame) -> Descend;

  /// Leaves the entry at `index`, `frame`, after every entry beneath it: one it entered
  /// answering [`Descend::Into`].
  fn leave(&mut self, index: usize, frame: &Frame);

  /// Announces the open overlay's layer `layer`, after the root's flow and every layer
  /// opened before it, before its entries.
  fn layer(&mut self, layer: &Layer);
}

/// Where the index keeps a node.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Slot {
  /// An entry, at its index.
  Entry(usize),
  /// A run flowing in a paragraph, which has no entry: placed by the flow of the
  /// paragraph whose entry is at `paragraph`.
  Run { paragraph: usize },
}

/// The drawn runtime's index of one window's rectangles ([`Runtime::frames`]).
///
/// It holds an entry ([`Frame`]) per laid-out node in paint order, the root's flow in the
/// scene's child order, overlays left out of it, then each open overlay's subtree in the
/// order they opened ([`Layer`]), with a lookup by node.
///
/// A run flowing in a paragraph has no entry: its paragraph's flow places it, in pieces
/// read from the flow as a query asks ([`Runtime::run_rects`], [`Runtime::rect`]). A
/// closed overlay and what it holds have no entry either.
#[derive(Debug, Default)]
#[must_use]
pub struct Frames {
  entries: Vec<Frame>,
  slots: NodeMap<Slot>,
  layers: Vec<Layer>,
  pending: Vec<Visit>,
}

impl Frames {
  /// Every entry, in paint order.
  #[inline(always)]
  #[must_use]
  pub const fn entries(&self) -> &[Frame] {
    self.entries.as_slice()
  }

  /// The open overlays' layers, in the order they opened; later ones stack on top.
  #[inline(always)]
  #[must_use]
  pub const fn layers(&self) -> &[Layer] {
    self.layers.as_slice()
  }

  /// How many of the entries, from the first, are the root's flow.
  #[must_use]
  pub const fn flow_len(&self) -> usize {
    match self.layers.as_slice().first() {
      Some(layer) => layer.start,
      None => self.entries.len(),
    }
  }

  /// `node`'s entry, where it has one.
  #[must_use]
  pub fn get(&self, node: NodeId) -> Option<&Frame> {
    match self.slots.get(node)? {
      Slot::Entry(index) => self.entries.get(*index),
      Slot::Run { .. } => None,
    }
  }

  /// `node`'s border box in the window, its entry's; `None` for a run flowing in a
  /// paragraph, which [`Runtime::rect`] places, and for a node the index does not hold.
  #[must_use]
  pub fn rect(&self, node: NodeId) -> Option<Rect<f32>> {
    self.get(node).map(|frame| frame.rect)
  }

  /// `node`'s origin in the window, as [`rect`](Self::rect) places it.
  #[must_use]
  pub fn origin(&self, node: NodeId) -> Option<Point<f32>> {
    self.rect(node).map(|rect| rect.origin)
  }

  /// The entry of the paragraph the run `run` flows in; `None` for a node that is no run
  /// flowing in a paragraph.
  fn paragraph_of(&self, run: NodeId) -> Option<&Frame> {
    match self.slots.get(run)? {
      Slot::Run { paragraph } => self.entries.get(*paragraph),
      Slot::Entry(_) => None,
    }
  }

  /// Where `node` lies in the window, as its border box and its origin: an entry's own;
  /// for a run flowing in a paragraph, the union of the pieces `pieces` places it in on
  /// its paragraph's lines, given the paragraph's entry, and the top left of its first,
  /// or the paragraph's origin with no size where it takes none; `None` for a node the
  /// index does not hold.
  fn locate(
    &self,
    node: NodeId,
    pieces: impl FnOnce(&Frame) -> Vec<Rect<f32>>,
  ) -> Option<(Rect<f32>, Point<f32>)> {
    match *self.slots.get(node)? {
      Slot::Entry(index) => {
        let rect = self.entries[index].rect;

        Some((rect, rect.origin))
      }
      Slot::Run { paragraph } => {
        let paragraph = &self.entries[paragraph];
        let pieces = pieces(paragraph);
        let Some(first) = pieces.first()
        else {
          return Some((
            Rect::new(paragraph.rect.origin, Size::ZERO),
            paragraph.rect.origin,
          ));
        };

        Some((
          pieces[1..]
            .iter()
            .fold(*first, |union, piece| union.union(*piece)),
          first.origin,
        ))
      }
    }
  }

  /// What `node`'s paint covers in the window ([`Frame::cover`]); `None` for a node of no
  /// extent or one without an entry.
  #[must_use]
  pub fn cover(&self, node: NodeId) -> Option<Rect<f32>> {
    self.get(node)?.cover
  }

  /// Walks every entry in paint order through `visitor`: the root's flow, then each open
  /// overlay's layer in the order they opened, announced before its entries
  /// ([`FrameVisitor::layer`]).
  ///
  /// Each entry is entered with its index before the entries beneath it, a paragraph's
  /// inline boxes after the paragraph, and left after them
  /// ([`FrameVisitor::enter`], [`FrameVisitor::leave`]); one entered answering
  /// [`Descend::Skip`] is left with none of them entered. An entry that takes no hits
  /// ([`Frame::hits`]), an invisible node and everything beneath it, is passed over,
  /// neither entered nor left. A run flowing in a paragraph has no entry, its paragraph
  /// standing for it.
  pub fn visit(&self, visitor: &mut impl FrameVisitor) {
    self.visit_range(0..self.flow_len(), visitor);

    for layer in &self.layers {
      visitor.layer(layer);
      self.visit_range(layer.start..layer.end, visitor);
    }
  }

  /// Walks the entries of `range`, a run of whole subtrees topped by no entry outside it,
  /// as [`visit`](Self::visit) walks them; the entries open are those down the parents
  /// from the deepest, left as the walk passes their ends.
  fn visit_range(&self, range: std::ops::Range<usize>, visitor: &mut impl FrameVisitor) {
    let mut open = None;
    let mut index = range.start;

    while index < range.end {
      self.leave_ended(&mut open, index, visitor);

      let frame = &self.entries[index];

      if !frame.hits {
        index = frame.end;

        continue;
      }

      debug_assert_eq!(open, frame.parent, "an entry is entered beneath its parent");

      match visitor.enter(index, frame) {
        Descend::Into => {
          open = Some(index);
          index += 1;
        }
        Descend::Skip => index = frame.end,
      }
    }

    self.leave_ended(&mut open, range.end, visitor);
  }

  /// Leaves every open entry, from `open`, the deepest, up its parents, that ends at or
  /// before `index`.
  fn leave_ended(&self, open: &mut Option<usize>, index: usize, visitor: &mut impl FrameVisitor) {
    while let Some(top) = *open {
      let frame = &self.entries[top];

      if frame.end > index {
        break;
      }

      visitor.leave(top, frame);
      *open = frame.parent;
    }
  }

  /// The topmost node at `point` in the window, `None` where the point hits nothing:
  /// each open overlay's layer from the last opened, nothing beneath the topmost modal,
  /// then the root's flow.
  ///
  /// Within its layer the node is the deepest of the topmost subtree, later children above
  /// earlier ones and above their parent: an entry that takes hits and whose border box
  /// holds the point within its clip, a box that does not clip passing hits on wherever
  /// what it holds reaches. A paragraph's entry stands for its runs and inline boxes:
  /// `within` answers the node the point hits inside it, topmost first, and the paragraph
  /// takes the hit itself where `within` answers `None` and its border box holds the
  /// point.
  pub fn hit(
    &self,
    point: Point<f32>,
    mut within: impl FnMut(&Frame) -> Option<NodeId>,
  ) -> Option<NodeId> {
    for layer in self.layers.iter().rev() {
      if let Some(node) = self.scan(layer.start..layer.end, point, &mut within) {
        return Some(node);
      }

      if layer.modal {
        return None;
      }
    }

    self.scan(0..self.flow_len(), point, &mut within)
  }

  /// The topmost node at `point` among `node` and the subtree beneath it, as
  /// [`hit`](Self::hit) finds it in a layer; `None` where the point hits none of it or
  /// `node` has no entry.
  pub fn hit_subtree(
    &self,
    node: NodeId,
    point: Point<f32>,
    mut within: impl FnMut(&Frame) -> Option<NodeId>,
  ) -> Option<NodeId> {
    let Some(Slot::Entry(index)) = self.slots.get(node)
    else {
      return None;
    };

    self.scan(*index..self.entries[*index].end, point, &mut within)
  }

  /// The topmost node at `point` among the entries of `range`, a subtree or a run of
  /// sibling subtrees: the last entry in paint order that takes the point, a subtree
  /// passed over where its reach or its clip misses the point, and each paragraph after it
  /// asked of `within`, the last first.
  fn scan(
    &self,
    range: std::ops::Range<usize>,
    point: Point<f32>,
    within: &mut impl FnMut(&Frame) -> Option<NodeId>,
  ) -> Option<NodeId> {
    let mut topmost = None;
    let mut paragraphs = SmallVec::<[usize; 4]>::new();
    let mut index = range.start;

    while index < range.end {
      let frame = &self.entries[index];

      if !frame.hits
        || !frame.reach.contains(point)
        || frame.clip.is_some_and(|clip| !clip.contains(point))
      {
        index = frame.end;

        continue;
      }

      if frame.paragraph {
        paragraphs.push(index);
        index = frame.end;

        continue;
      }

      if frame.rect.contains(point) {
        topmost = Some(index);
        paragraphs.clear();
      }

      index += 1;
    }

    paragraphs
      .into_iter()
      .rev()
      .find_map(|index| {
        let frame = &self.entries[index];

        within(frame).or_else(|| frame.rect.contains(point).then_some(frame.node))
      })
      .or_else(|| topmost.map(|index| self.entries[index].node))
  }

  /// The index of `source`'s window, built into `frames`' storage: the root's flow, then
  /// each open overlay's subtree in the order they opened, every overlay then placed
  /// against the entries already built, one whose anchor lies in a layer not yet placed
  /// placing that layer first.
  fn build(source: &impl FrameSource, mut frames: Self) -> Self {
    frames.entries.clear();
    frames.slots.clear();
    frames.layers.clear();
    frames.walk(source, source.root());

    for (overlay, modal) in source.open_overlays() {
      let start = frames.entries.len();

      if frames.walk(source, overlay) {
        frames.layers.push(Layer {
          overlay,
          modal,
          start,
          end: frames.entries.len(),
        });
      }
    }

    let mut placing = vec![Placing::Pending; frames.layers.len()];

    for layer in 0..frames.layers.len() {
      frames.place(source, layer, &mut placing);
    }

    frames
  }

  /// Indexes `top` and the subtree beneath it in the scene's child order, its location
  /// from the window's origin, overlays beneath it left out; `false` where `top` is no
  /// laid-out node.
  fn walk(&mut self, source: &impl FrameSource, top: NodeId) -> bool {
    let first = self.entries.len();
    let mut pending = std::mem::take(&mut self.pending);

    pending.clear();

    pending.push(Visit {
      node: top,
      base: Point::ZERO,
      clip: None,
      hits: true,
      parent: None,
      flows: false,
    });

    while let Some(visit) = pending.pop() {
      let Some(laid) = source.laid(visit.node)
      else {
        if visit.node != top {
          let children = source.children(visit.node);

          pending.extend(children.into_iter().rev().map(|child| Visit {
            node: child,
            ..visit
          }));
        }

        continue;
      };

      if laid.overlay && visit.node != top {
        continue;
      }

      let flowing = laid.inline == Inline::Run && visit.flows;
      let location = if flowing {
        Point::ZERO
      }
      else {
        laid.placed.rect.origin
      };
      let rect = Rect::new(
        Point::new(visit.base.x + location.x, visit.base.y + location.y),
        laid.placed.rect.size,
      );
      let hits = visit.hits && !laid.invisible;
      let children = source.children(visit.node);
      let paragraph = !children.is_empty() && source.is_paragraph(visit.node);
      let clip = laid.children_clip(rect, visit.clip);
      let parent = if flowing {
        self.flows(visit);

        visit.parent
      }
      else {
        Some(self.push(visit, &laid, (rect, clip), (hits, paragraph)))
      };

      if children.is_empty() {
        continue;
      }

      let scroll = source.scroll(visit.node);
      let base = Point::new(rect.origin.x - scroll.x, rect.origin.y - scroll.y);
      let flows = paragraph || (laid.inline != Inline::None && visit.flows);

      pending.extend(children.into_iter().rev().map(|node| Visit {
        node,
        base,
        clip,
        hits,
        parent,
        flows,
      }));
    }

    self.pending = pending;

    for index in (first..self.entries.len()).rev() {
      if let Some(parent) = self.entries[index].parent {
        let end = self.entries[index].end;
        let held = &mut self.entries[parent].end;

        *held = (*held).max(end);
      }
    }

    self.entries.len() > first
  }

  /// Indexes the run `visit` reaches as flowing in the paragraph whose entry it flows
  /// beneath, with no entry of its own.
  fn flows(&mut self, visit: Visit) {
    if let Some(paragraph) = visit.parent {
      let _replaced = self.slots.insert(visit.node, Slot::Run { paragraph });
    }
  }

  /// Pushes the entry of the node `visit` reaches, laid out as `laid` at `rect`, what
  /// shows of its content `content_clip`, taking hits and laying its runs out as a
  /// paragraph where `hits` and `paragraph` say, returning its index.
  fn push(
    &mut self,
    visit: Visit,
    laid: &Laid,
    (rect, content_clip): (Rect<f32>, Option<Rect<f32>>),
    (hits, paragraph): (bool, bool),
  ) -> usize {
    let index = self.entries.len();
    let overflow = laid.placed.overflow.translate(Point::new(
      rect.origin.x + laid.placed.border.start,
      rect.origin.y + laid.placed.border.top,
    ));

    self.entries.push(Frame {
      node: visit.node,
      rect,
      cover: rect.has_extent().then(|| rect.inflate(laid.reach)),
      reach: if laid.clips {
        rect
      }
      else {
        rect.union(overflow)
      },
      clip: visit.clip,
      content_clip,
      hits,
      paragraph,
      parent: visit.parent,
      end: index + 1,
    });

    let _replaced = self.slots.insert(visit.node, Slot::Entry(index));

    index
  }

  /// Places the layer at `layer` where `source` places its overlay, against its anchor's
  /// rectangle, placing first the layer that anchor lies in where it is not yet placed,
  /// and moves its subtree there.
  fn place(&mut self, source: &impl FrameSource, layer: usize, placing: &mut [Placing]) {
    if placing[layer] != Placing::Pending {
      return;
    }

    placing[layer] = Placing::Underway;

    let Layer {
      overlay,
      start,
      end,
      ..
    } = self.layers[layer];
    let origin = source.place(overlay, |anchor| self.anchor_rect(source, anchor, placing));
    let at = self.entries[start].rect.origin;
    let by = Point::new(origin.x - at.x, origin.y - at.y);

    for entry in &mut self.entries[start..end] {
      entry.rect = entry.rect.translate(by);
      entry.cover = entry.cover.map(|cover| cover.translate(by));
      entry.reach = entry.reach.translate(by);
      entry.clip = entry.clip.map(|clip| clip.translate(by));
      entry.content_clip = entry.content_clip.map(|clip| clip.translate(by));
    }

    placing[layer] = Placing::Done;
  }

  /// The window rectangle of the anchor `anchor`, its layer placed first where it lies
  /// in one not yet placed: its border box, or a run's pieces' union as its paragraph's
  /// flow places them ([`locate`](Self::locate)); empty at the window's origin for a node
  /// the index does not hold.
  fn anchor_rect(
    &mut self,
    source: &impl FrameSource,
    anchor: NodeId,
    placing: &mut [Placing],
  ) -> Rect<f32> {
    let index = match self.slots.get(anchor) {
      Some(Slot::Entry(index)) => *index,
      Some(Slot::Run { paragraph }) => *paragraph,
      None => return Rect::ZERO,
    };

    if let Some(layer) = self
      .layers
      .iter()
      .position(|layer| (layer.start..layer.end).contains(&index))
    {
      self.place(source, layer, placing);
    }

    self
      .locate(anchor, |paragraph| source.pieces(anchor, paragraph))
      .map_or(Rect::ZERO, |(rect, _)| rect)
  }
}

/// How far a layer's placement has come while the index is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placing {
  Pending,
  Underway,
  Done,
}

/// A node the index is yet to visit, with what its walk carries down to it.
#[derive(Debug, Clone, Copy)]
struct Visit {
  node: NodeId,
  base: Point<f32>,
  clip: Option<Rect<f32>>,
  hits: bool,
  parent: Option<usize>,
  flows: bool,
}

/// Whether a laid-out node is a run inline in a paragraph's flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Inline {
  /// It is not: a box, or a run laid out on its own line.
  None,
  /// A run of text or a group, flowing in its paragraph where one encloses it.
  Run,
  /// An inline box, laid out as a box among its paragraph's runs.
  Box,
}

/// What the index reads of a laid-out node as its walk reaches it.
#[derive(Debug, Clone, Copy)]
struct Laid {
  placed: Placed,
  reach: Inset<f32>,
  clips: bool,
  invisible: bool,
  overlay: bool,
  inline: Inline,
}

impl Laid {
  /// What shows of the children of the node `self` reads, its border box at `rect` and
  /// what shows of it `clip`: `clip` cut to its padding box where it clips.
  fn children_clip(&self, rect: Rect<f32>, clip: Option<Rect<f32>>) -> Option<Rect<f32>> {
    if !self.clips {
      return clip;
    }

    let padding = rect.deflate(self.placed.border);

    Some(clip.map_or(padding, |clip| clip.intersect(padding)))
  }
}

/// What [`Frames`] is built from: a window's scene, its records and its layout.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a window `Frames` can index",
  label = "`Frames` indexes a window's scene, records and layout, read through the runtime or a canned window"
)]
trait FrameSource {
  /// The node filling the window.
  fn root(&self) -> NodeId;

  /// The children of `node` as the scene places them, in order.
  fn children(&self, node: NodeId) -> SmallVec<[NodeId; 8]>;

  /// What the index reads of `node`, `None` for a node that is not laid out: an anchor
  /// or a scope, its content laid out in its place.
  fn laid(&self, node: NodeId) -> Option<Laid>;

  /// The offsets `node`'s children are scrolled by.
  fn scroll(&self, node: NodeId) -> Point<f32>;

  /// Whether `node` lays its runs out as one paragraph.
  fn is_paragraph(&self, node: NodeId) -> bool;

  /// Every open overlay in the order they opened, with whether it is modal.
  fn open_overlays(&self) -> Vec<(NodeId, bool)>;

  /// The window origin of the overlay `overlay`, against the window rectangle
  /// `anchor_rect` gives for the node it opens against, where it opens against one.
  fn place(&self, overlay: NodeId, anchor_rect: impl FnOnce(NodeId) -> Rect<f32>) -> Point<f32>;

  /// The window rectangles of the pieces the run `run` takes on the lines of the
  /// paragraph whose entry is `paragraph`, in the order its flow places them, laid out in
  /// the paragraph's content box, scrolled with it.
  fn pieces(&self, run: NodeId, paragraph: &Frame) -> Vec<Rect<f32>>;
}

impl<P: Drawn> FrameSource for Runtime<P> {
  #[inline(always)]
  fn root(&self) -> NodeId {
    self.root
  }

  #[inline(always)]
  fn children(&self, node: NodeId) -> SmallVec<[NodeId; 8]> {
    Self::children(self, node)
  }

  fn laid(&self, node: NodeId) -> Option<Laid> {
    let record = self.record(node)?;
    let style = record.kind.box_style();
    let inline = match &record.kind {
      Kind::Run { facts, .. } => match facts.placement {
        RunPlacement::Line => Inline::None,
        RunPlacement::Embedded => Inline::Box,
        RunPlacement::Inline => Inline::Run,
      },
      _ => Inline::None,
    };

    Some(Laid {
      placed: self.tree.layout(node).unwrap_or_default(),
      reach: style.map_or(Inset::ZERO, P::reach),
      clips: self.clips(node),
      invisible: record.is_invisible(),
      overlay: record.is_overlay(),
      inline,
    })
  }

  #[inline(always)]
  fn scroll(&self, node: NodeId) -> Point<f32> {
    self.scroll_shift(node)
  }

  #[inline(always)]
  fn is_paragraph(&self, node: NodeId) -> bool {
    Self::is_paragraph(self, node)
  }

  fn open_overlays(&self) -> Vec<(NodeId, bool)> {
    let open = Self::open_overlays(self);

    Scene::<P>::read(|scene| {
      open
        .into_iter()
        .map(|overlay| (overlay, scene.is_modal(overlay)))
        .collect()
    })
  }

  #[inline(always)]
  fn place(&self, overlay: NodeId, anchor_rect: impl FnOnce(NodeId) -> Rect<f32>) -> Point<f32> {
    self.overlay_origin(overlay, anchor_rect)
  }

  fn pieces(&self, run: NodeId, paragraph: &Frame) -> Vec<Rect<f32>> {
    let Some(flow) = self.flow_of(paragraph.node)
    else {
      return Vec::new();
    };
    let content = self.flow_content(paragraph.node);
    let origin = paragraph.rect.origin;

    flow
      .place(content.translate(origin))
      .into_iter()
      .filter_map(|(node, piece)| (node == run).then_some(piece))
      .collect()
  }
}

impl<P: Drawn> Runtime<P> {
  /// The path to the topmost node at `point` in the window ([`Frames::hit`]), each node
  /// with its window origin ([`Frames::origin`]): the laid-out nodes from the root, or
  /// from the open overlay the node lies in, as a top-layer element's chain ends at it,
  /// down to the node; empty where the point hits nothing.
  ///
  /// A paragraph resolves a point within what shows of its content (its padding box,
  /// where it clips) to the run its flow draws there, laid out in its content box and
  /// scrolled with it ([`flow_content`](Self::flow_content)), else to the first of its
  /// inline boxes whose border box holds it; any other point to what its inline boxes
  /// hold wherever that reaches and shows, the last box first; the groups around a run or a box
  /// join the path, and a run or a group takes the origin its flow places it at
  /// ([`origin`](Self::origin)).
  #[must_use]
  pub fn hit(&self, point: Point<f32>) -> Vec<(NodeId, Point<f32>)> {
    let frames = self.frames();
    let Some(deepest) = frames.hit(point, |paragraph| {
      self.hit_paragraph(frames, paragraph, point)
    })
    else {
      return Vec::new();
    };
    let mut path = Vec::new();
    let mut node = Some(deepest);

    while let Some(current) = node {
      path.push((current, self.origin(current).unwrap_or(Point::ZERO)));

      node = match frames.get(current) {
        Some(frame) if !self.is_inline_box(current) => {
          frame.parent.map(|parent| frames.entries()[parent].node)
        }
        _ => self.layout_parent(current),
      };
    }

    path.reverse();

    path
  }

  /// The node `point` hits inside the paragraph `paragraph` of `frames`, as
  /// [`hit`](Self::hit) resolves it; `None` where it hits no run and nothing an inline
  /// box holds.
  fn hit_paragraph(&self, frames: &Frames, paragraph: &Frame, point: Point<f32>) -> Option<NodeId> {
    let node = paragraph.node;
    let flow = self.flow_of(node)?;
    let boxes = self.inline_boxes(node);
    let origin = paragraph.rect.origin;
    let local = Point::new(point.x - origin.x, point.y - origin.y);
    let clip = paragraph.content_clip;
    let inside = |boxed: NodeId| {
      frames.hit_subtree(boxed, point, |inner| {
        self.hit_paragraph(frames, inner, point)
      })
    };

    if local.x >= 0. && local.y >= 0. && clip.is_none_or(|clip| clip.contains(point)) {
      if let Some(run) = flow.run_at(self.flow_content(node), local) {
        return Some(run.node);
      }

      if let Some(boxed) = boxes.iter().copied().find(|boxed| {
        frames
          .get(*boxed)
          .is_some_and(|frame| frame.rect.contains(point))
      }) {
        return inside(boxed).or_else(|| self.layout_parent(boxed));
      }
    }

    boxes.into_iter().rev().find_map(inside)
  }

  /// The window rectangles of the pieces the run `run` takes where it flows in a
  /// paragraph, one per line it reaches, in the order the paragraph's flow places them,
  /// laid out in the paragraph's content box ([`flow_content`](Self::flow_content)); a
  /// group's are those of the runs and boxes it holds. Empty for a node that is no run
  /// flowing in a paragraph.
  #[must_use]
  pub fn run_rects(&self, run: NodeId) -> Vec<Rect<f32>> {
    self
      .frames()
      .paragraph_of(run)
      .map_or_else(Vec::new, |paragraph| self.pieces(run, paragraph))
  }

  /// `node`'s border box in the window ([`Frames::rect`]); for a run flowing in a
  /// paragraph, the union of the pieces it takes ([`run_rects`](Self::run_rects)), or its
  /// paragraph's origin with no size where it takes none. `None` for a node the index
  /// does not hold.
  #[must_use]
  pub fn rect(&self, node: NodeId) -> Option<Rect<f32>> {
    self
      .frames()
      .locate(node, |paragraph| self.pieces(node, paragraph))
      .map(|(rect, _)| rect)
  }

  /// `node`'s origin in the window: its border box's corner ([`Frames::origin`]); for a
  /// run flowing in a paragraph, the top left of the first piece it takes, as an inline's
  /// offset is measured from its first fragment, or its paragraph's origin where it takes
  /// none. `None` for a node the index does not hold.
  #[must_use]
  pub fn origin(&self, node: NodeId) -> Option<Point<f32>> {
    self
      .frames()
      .locate(node, |paragraph| self.pieces(node, paragraph))
      .map(|(_, origin)| origin)
  }

  /// What shows of `node` in the window: its border box ([`Frames::rect`]) within its
  /// clip ([`Frame::clip`]) and the window; for a run flowing in a paragraph, of the first
  /// piece it takes ([`run_rects`](Self::run_rects)) of which anything shows, within its
  /// paragraph's clip and the padding box of a paragraph that clips. `None` where nothing
  /// shows, or for a node the index does not hold.
  #[must_use]
  pub fn shown(&self, node: NodeId) -> Option<Rect<f32>> {
    let viewport = self.window().reported().viewport;
    let window = Rect::new(
      Point::ZERO,
      Size::new(viewport.width.to_f32(), viewport.height.to_f32()),
    );
    let frames = self.frames();
    let showing = |rect: Rect<f32>, clip: Option<Rect<f32>>| {
      let shown = clip
        .map_or(rect, |clip| rect.intersect(clip))
        .intersect(window);

      shown.has_extent().then_some(shown)
    };

    if let Some(frame) = frames.get(node) {
      return showing(frame.rect, frame.clip);
    }

    let paragraph = frames.paragraph_of(node)?;

    self
      .pieces(node, paragraph)
      .into_iter()
      .find_map(|piece| showing(piece, paragraph.content_clip))
  }

  /// The index of the window's rectangles ([`Frames`]) as the scene, the records and the
  /// last layout place them now: built afresh at the first read after a layout, a scroll
  /// offset, a box's reach or visibility, an overlay's opening or placement, or the
  /// scene's structure moved.
  #[must_use]
  pub fn frames(&self) -> &Frames {
    self
      .frames
      .get_or_init(|| Frames::build(self, self.spare_frames.take()))
  }

  /// Drops the index, built afresh at its next read into the storage it held: what it was
  /// built from moved.
  pub(super) fn invalidate_frames(&mut self) {
    if let Some(stale) = self.frames.take() {
      *self.spare_frames.get_mut() = stale;
    }
  }

  /// Drops the index and notes that the covers may have moved since the platform last
  /// painted ([`Runtime::covers_moved`]).
  pub(super) fn move_geometry(&mut self) {
    self.invalidate_frames();
    self.covers_moved = true;
  }

  /// Marks `node` for repaint: what its paint owner ([`paint_owner`](Self::paint_owner))
  /// covered at the last paint and what it covers now are painted again at the next
  /// frame.
  #[inline(always)]
  pub fn damage(&mut self, node: NodeId) {
    self.damaged.push(node);
  }

  /// Whether anything is marked for repaint: a damaged node, or what a destroyed node
  /// covered at the last paint.
  #[inline(always)]
  #[must_use]
  pub const fn has_damage(&self) -> bool {
    !self.damaged.is_empty() || !self.damaged_rects.is_empty()
  }

  /// The node whose paint covers `node`: the paragraph a run flows in or the inline box
  /// whose content it is, through any groups around it; else `node` itself.
  pub fn paint_owner(&self, node: NodeId) -> NodeId {
    let mut owner = node;

    while let Some(parent) = self.layout_parent(owner) {
      if !(self.is_paragraph(parent) || self.is_inline_run(parent)) {
        break;
      }

      owner = parent;
    }

    owner
  }

  /// Takes what the next paint repaints, as rectangles of the window: what each damaged
  /// node's paint owner covered at the last paint and covers now, what every destroyed
  /// node covered at the last paint, and, where the covers moved since
  /// ([`covers_moved`](Self::covers_moved)), what each node whose cover changed covered
  /// and covers now, every node's cover then recorded as the one it covered at this
  /// paint ([`Record::painted`](super::Record::painted)).
  pub fn take_damage(&mut self) -> Vec<Rect<f32>> {
    let mut rects = std::mem::take(&mut self.damaged_rects);
    let damaged = std::mem::take(&mut self.damaged);
    let moved = std::mem::take(&mut self.covers_moved);
    let frames = self
      .frames
      .get_or_init(|| Frames::build(self, self.spare_frames.take()));

    for node in damaged {
      let owner = self.paint_owner(node);

      if let Some(record) = self.records.get(owner) {
        rects.extend(record.painted);
        rects.extend(frames.cover(owner));
      }
    }

    if moved {
      for (node, record) in self.records.iter_mut() {
        let cover = frames.cover(node);

        if record.painted != cover {
          rects.extend(record.painted);
          rects.extend(cover);
          record.painted = cover;
        }
      }
    }

    rects
  }
}

#[cfg(test)]
mod tests {
  use rustc_hash::FxHashMap;
  use skene_foundation::{Inset, NodeId, Point, Rect, Size};
  use smallvec::SmallVec;

  use super::{Descend, Frame, FrameSource, FrameVisitor, Frames, Inline, Laid, Layer};
  use crate::Placed;

  /// Where a canned overlay is placed.
  #[derive(Clone, Copy)]
  enum Place {
    /// Below its anchor, at the anchor's start.
    Below(NodeId),
    /// At a point of the window.
    At(Point<f32>),
  }

  /// A canned node: what the index reads of it.
  #[derive(Clone, Default)]
  struct Node {
    children: Vec<NodeId>,
    laid: Option<Laid>,
    scroll: Point<f32>,
    paragraph: bool,
  }

  /// A canned window: its nodes, its open overlays in order, where each is placed and the
  /// pieces each flowing run takes from its paragraph's corner.
  #[derive(Default)]
  struct Canned {
    nodes: FxHashMap<NodeId, Node>,
    open: Vec<(NodeId, bool)>,
    places: FxHashMap<NodeId, Place>,
    pieces: FxHashMap<NodeId, Vec<Rect<f32>>>,
  }

  /// The canned root, a 40 by 20 box at the window's origin.
  const ROOT: NodeId = NodeId::new(0, 0);

  /// A laid-out box at `(x, y)` in its parent, `width` by `height`, its content reaching
  /// no farther.
  fn laid(x: f32, y: f32, width: f32, height: f32) -> Laid {
    Laid {
      placed: Placed {
        rect: Rect::new(Point::new(x, y), Size::new(width, height)),
        border: Inset::ZERO,
        padding: Inset::ZERO,
        overflow: Rect::new(Point::ZERO, Size::new(width, height)),
        scroll_limit: Size::ZERO,
      },
      reach: Inset::ZERO,
      clips: false,
      invisible: false,
      overlay: false,
      inline: Inline::None,
    }
  }

  impl Canned {
    /// A window holding the root alone.
    fn new() -> Self {
      let mut canned = Self::default();

      canned.nodes.insert(
        ROOT,
        Node {
          laid: Some(laid(0., 0., 40., 20.)),
          ..Node::default()
        },
      );

      canned
    }

    /// Adds `node`, laid out as `laid`, as the last child of `parent`.
    fn add(&mut self, parent: NodeId, node: NodeId, laid: Option<Laid>) -> &mut Node {
      self.nodes.entry(parent).or_default().children.push(node);
      self.nodes.entry(node).or_default().laid = laid;

      self.nodes.get_mut(&node).expect("the node just added")
    }

    /// Adds the overlay `node`, laid out as `laid`, under `parent`, opened after every
    /// overlay opened before it and placed at `place`.
    fn add_open_overlay(&mut self, parent: NodeId, node: NodeId, mut laid: Laid, place: Place) {
      laid.overlay = true;
      self.add(parent, node, Some(laid));
      self.open.push((node, false));
      self.places.insert(node, place);
    }

    /// Makes the open overlay `node` modal.
    fn make_modal(&mut self, node: NodeId) {
      for (overlay, modal) in &mut self.open {
        *modal |= *overlay == node;
      }
    }
  }

  impl FrameSource for Canned {
    fn root(&self) -> NodeId {
      ROOT
    }

    fn children(&self, node: NodeId) -> SmallVec<[NodeId; 8]> {
      self
        .nodes
        .get(&node)
        .map(|node| node.children.iter().copied().collect())
        .unwrap_or_default()
    }

    fn laid(&self, node: NodeId) -> Option<Laid> {
      self.nodes.get(&node)?.laid
    }

    fn scroll(&self, node: NodeId) -> Point<f32> {
      self
        .nodes
        .get(&node)
        .map_or(Point::ZERO, |node| node.scroll)
    }

    fn is_paragraph(&self, node: NodeId) -> bool {
      self.nodes.get(&node).is_some_and(|node| node.paragraph)
    }

    fn open_overlays(&self) -> Vec<(NodeId, bool)> {
      self.open.clone()
    }

    fn place(&self, overlay: NodeId, anchor_rect: impl FnOnce(NodeId) -> Rect<f32>) -> Point<f32> {
      match self.places[&overlay] {
        Place::Below(anchor) => {
          let rect = anchor_rect(anchor);

          Point::new(rect.origin.x, rect.origin.y + rect.size.height)
        }
        Place::At(point) => point,
      }
    }

    fn pieces(&self, run: NodeId, paragraph: &Frame) -> Vec<Rect<f32>> {
      self.pieces.get(&run).map_or_else(Vec::new, |pieces| {
        pieces
          .iter()
          .map(|piece| piece.translate(paragraph.rect.origin))
          .collect()
      })
    }
  }

  impl Frames {
    /// Where `node` lies as [`Frames::locate`] places it over `canned`'s pieces.
    fn located(&self, canned: &Canned, node: NodeId) -> Option<(Rect<f32>, Point<f32>)> {
      self.locate(node, |paragraph| canned.pieces(node, paragraph))
    }
  }

  /// The node of the canned id `index`.
  const fn node(index: u32) -> NodeId {
    NodeId::new(index, 0)
  }

  /// A rectangle at `(x, y)`, `width` by `height`.
  const fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect<f32> {
    Rect::new(Point::new(x, y), Size::new(width, height))
  }

  /// A node's rectangle sums the locations down its laid-out ancestors, an anchor or a
  /// scope taking no room, and each subtree's entries end where it does.
  #[test]
  fn a_rectangle_sums_the_locations_above_it() {
    let mut canned = Canned::new();

    canned.add(ROOT, node(1), Some(laid(2., 3., 20., 10.)));
    canned.add(node(1), node(2), None);
    canned.add(node(2), node(3), Some(laid(1., 1., 4., 2.)));
    canned.add(ROOT, node(4), Some(laid(0., 13., 5., 1.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.rect(node(3)), Some(rect(3., 4., 4., 2.)));
    assert_eq!(frames.rect(node(2)), None);
    assert_eq!(
      frames
        .entries()
        .iter()
        .map(|frame| (frame.node, frame.parent, frame.end))
        .collect::<Vec<_>>(),
      [
        (ROOT, None, 4),
        (node(1), Some(0), 3),
        (node(3), Some(1), 3),
        (node(4), Some(0), 4),
      ]
    );
  }

  /// A scrolled box shifts what it holds by its offsets, and keeps its own place.
  #[test]
  fn a_scroll_offset_shifts_the_children() {
    let mut canned = Canned::new();

    canned
      .add(ROOT, node(1), Some(laid(0., 2., 20., 5.)))
      .scroll = Point::new(1., 3.);
    canned.add(node(1), node(2), Some(laid(0., 4., 20., 1.)));
    canned.add(node(2), node(3), Some(laid(2., 0., 3., 1.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.rect(node(1)), Some(rect(0., 2., 20., 5.)));
    assert_eq!(frames.rect(node(2)), Some(rect(-1., 3., 20., 1.)));
    assert_eq!(frames.rect(node(3)), Some(rect(1., 3., 3., 1.)));
  }

  /// A clipping box clips what it holds at its padding box, inside its border, and the
  /// clips of nested boxes intersect; a box that does not clip leaves its children's
  /// clip as it found it.
  #[test]
  fn clips_intersect_at_each_clipping_padding_box() {
    let mut canned = Canned::new();
    let mut outer = laid(1., 1., 10., 6.);
    let mut inner = laid(4., 2., 10., 10.);

    outer.clips = true;
    outer.placed.border = Inset::all(1.);
    inner.clips = true;
    canned.add(ROOT, node(1), Some(outer));
    canned.add(node(1), node(2), Some(inner));
    canned.add(node(2), node(3), Some(laid(0., 0., 2., 2.)));
    canned.add(node(1), node(4), Some(laid(0., 0., 2., 2.)));

    let frames = Frames::build(&canned, Frames::default());
    let clip = |index| frames.get(node(index)).and_then(|frame| frame.clip);

    assert_eq!(clip(1), None);
    assert_eq!(clip(2), Some(rect(2., 2., 8., 4.)));
    assert_eq!(clip(3), Some(rect(5., 3., 5., 3.)));
    assert_eq!(clip(4), Some(rect(2., 2., 8., 4.)));
  }

  /// A box's cover grows by its paint's reach, a box of no extent covers nothing, and its
  /// hit reach holds its overflow; an invisible box keeps its rectangle and takes no
  /// hit, nor does anything beneath it.
  #[test]
  fn covers_reaches_and_hits() {
    let mut canned = Canned::new();
    let mut shadowed = laid(1., 1., 4., 2.);
    let mut hidden = laid(0., 5., 4., 2.);

    shadowed.reach = Inset::new(0, 0, 1, 1);
    shadowed.placed.overflow = rect(-1., 0., 8., 3.);
    hidden.invisible = true;
    canned.add(ROOT, node(1), Some(shadowed));
    canned.add(ROOT, node(2), Some(laid(0., 4., 0., 1.)));
    canned.add(ROOT, node(3), Some(hidden));
    canned.add(node(3), node(4), Some(laid(0., 0., 1., 1.)));

    let frames = Frames::build(&canned, Frames::default());
    let frame = |index| *frames.get(node(index)).expect("an entry");

    assert_eq!(frame(1).cover, Some(rect(1., 1., 5., 3.)));
    assert_eq!(frame(1).reach, rect(0., 1., 8., 3.));
    assert_eq!(frame(2).cover, None);
    assert_eq!(frame(3).rect, rect(0., 5., 4., 2.));
    assert!(frame(1).hits && !frame(3).hits && !frame(4).hits);
  }

  /// A run flowing in a paragraph has no entry, through any group around it, and lies
  /// where its paragraph's flow places its pieces: their union, from the first's corner,
  /// or the paragraph's origin with no size where it takes none; an inline box is placed
  /// in the paragraph.
  #[test]
  fn a_flowing_run_lies_where_its_pieces_flow() {
    let mut canned = Canned::new();
    let mut run = laid(7., 0., 5., 1.);
    let mut boxed = laid(3., 0., 2., 1.);

    run.inline = Inline::Run;
    boxed.inline = Inline::Box;
    canned
      .add(ROOT, node(1), Some(laid(2., 4., 20., 2.)))
      .paragraph = true;
    canned.add(node(1), node(2), Some(run));
    canned.add(node(2), node(3), Some(run));
    canned.add(node(2), node(4), Some(boxed));
    canned.add(node(1), node(5), Some(run));
    canned
      .pieces
      .insert(node(3), vec![rect(6., 0., 4., 1.), rect(0., 1., 2., 1.)]);

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!((frames.get(node(3)), frames.rect(node(3))), (None, None));
    assert_eq!(
      frames.located(&canned, node(3)),
      Some((rect(2., 4., 10., 2.), Point::new(8., 4.)))
    );
    assert_eq!(
      frames.located(&canned, node(5)),
      Some((rect(2., 4., 0., 0.), Point::new(2., 4.)))
    );
    assert_eq!(frames.rect(node(4)), Some(rect(5., 4., 2., 1.)));
    assert_eq!(frames.get(node(4)).and_then(|frame| frame.parent), Some(1));
  }

  /// An overlay anchored on a run flowing in a paragraph opens against the run's pieces,
  /// where its paragraph's flow places them, not against the paragraph.
  #[test]
  fn an_overlay_anchors_on_a_flowing_run_where_it_flows() {
    let mut canned = Canned::new();
    let mut run = laid(0., 0., 0., 0.);

    run.inline = Inline::Run;
    canned
      .add(ROOT, node(1), Some(laid(1., 2., 20., 3.)))
      .paragraph = true;
    canned.add(node(1), node(2), Some(run));
    canned.pieces.insert(node(2), vec![rect(5., 1., 3., 1.)]);
    canned.add_open_overlay(ROOT, node(3), laid(0., 0., 4., 2.), Place::Below(node(2)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.rect(node(3)), Some(rect(6., 4., 4., 2.)));
  }

  /// An open overlay's subtree is left out of the flow it sits in and indexed after it,
  /// moved to where the overlay is placed, unclipped by the box it sits in and topped by
  /// no laid-out parent.
  #[test]
  fn an_overlay_moves_its_subtree_where_it_is_placed() {
    let mut canned = Canned::new();
    let mut clipping = laid(0., 0., 10., 10.);

    clipping.clips = true;
    canned.add(ROOT, node(1), Some(clipping)).scroll = Point::new(0., 2.);
    canned.add_open_overlay(
      node(1),
      node(2),
      laid(3., 1., 6., 4.),
      Place::At(Point::new(20., 5.)),
    );
    canned.add(node(2), node(3), Some(laid(1., 1., 2., 1.)));
    canned.add(ROOT, node(4), Some(laid(0., 10., 4., 1.)));

    let frames = Frames::build(&canned, Frames::default());
    let overlay = *frames.get(node(2)).expect("the overlay's entry");

    assert_eq!(frames.flow_len(), 3);
    assert_eq!(frames.layers()[0].start, 3);
    assert_eq!(overlay.rect, rect(20., 5., 6., 4.));
    assert_eq!((overlay.parent, overlay.clip), (None, None));
    assert_eq!(frames.rect(node(3)), Some(rect(21., 6., 2., 1.)));
    assert_eq!(frames.get(node(3)).and_then(|frame| frame.clip), None);
  }

  /// An overlay anchored inside an overlay that opened after it is placed once the later
  /// one is, against where its anchor shows.
  #[test]
  fn an_anchor_in_a_later_overlay_is_placed_first() {
    let mut canned = Canned::new();

    canned.add(ROOT, node(1), Some(laid(0., 0., 4., 1.)));
    canned.add_open_overlay(ROOT, node(2), laid(0., 0., 3., 2.), Place::Below(node(5)));
    canned.add_open_overlay(ROOT, node(4), laid(0., 0., 8., 3.), Place::Below(node(1)));
    canned.add(node(4), node(5), Some(laid(2., 1., 3., 1.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.rect(node(4)), Some(rect(0., 1., 8., 3.)));
    assert_eq!(frames.rect(node(5)), Some(rect(2., 2., 3., 1.)));
    assert_eq!(frames.rect(node(2)), Some(rect(2., 3., 3., 2.)));
    assert_eq!(
      frames
        .layers()
        .iter()
        .map(|layer| layer.overlay)
        .collect::<Vec<_>>(),
      [node(2), node(4)]
    );
  }

  impl Frames {
    /// The topmost node at `point` with no paragraph resolving anything.
    fn topmost(&self, point: Point<f32>) -> Option<NodeId> {
      self.hit(point, |_| None)
    }
  }

  /// The point `(x, y)`.
  const fn at(x: f32, y: f32) -> Point<f32> {
    Point::new(x, y)
  }

  /// Of two overlapping siblings the later takes the hit where they overlap, the earlier
  /// where it shows alone, and the root where neither is.
  #[test]
  fn the_topmost_of_overlapping_siblings_takes_the_hit() {
    let mut canned = Canned::new();

    canned.add(ROOT, node(1), Some(laid(0., 0., 10., 5.)));
    canned.add(ROOT, node(2), Some(laid(5., 2., 10., 5.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.topmost(at(6.5, 3.5)), Some(node(2)));
    assert_eq!(frames.topmost(at(1.5, 1.5)), Some(node(1)));
    assert_eq!(frames.topmost(at(14.5, 6.5)), Some(node(2)));
    assert_eq!(frames.topmost(at(30.5, 15.5)), Some(ROOT));
    assert_eq!(frames.topmost(at(40.5, 15.5)), None);
  }

  /// A child takes the hit above its parent, and the parent where the child is not.
  #[test]
  fn a_child_takes_the_hit_above_its_parent() {
    let mut canned = Canned::new();

    canned.add(ROOT, node(1), Some(laid(0., 0., 10., 10.)));
    canned.add(node(1), node(2), Some(laid(2., 2., 3., 3.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.topmost(at(3.5, 3.5)), Some(node(2)));
    assert_eq!(frames.topmost(at(8.5, 8.5)), Some(node(1)));
  }

  /// A clipping box clips what it holds at its padding box: a child is missed past it and
  /// under its border, where the box itself takes the hit.
  #[test]
  fn a_clipped_child_is_missed_outside_the_clip() {
    let mut canned = Canned::new();
    let mut clipping = laid(0., 0., 10., 5.);

    clipping.clips = true;
    clipping.placed.border = Inset::all(1.);
    canned.add(ROOT, node(1), Some(clipping));
    canned.add(node(1), node(2), Some(laid(0., 0., 20., 3.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.topmost(at(5.5, 1.5)), Some(node(2)));
    assert_eq!(frames.topmost(at(0.5, 0.5)), Some(node(1)));
    assert_eq!(frames.topmost(at(12.5, 1.5)), Some(ROOT));
  }

  /// What a box holds takes a hit wherever it reaches past a box that does not clip, and
  /// nowhere past one that does.
  #[test]
  fn overflow_takes_a_hit_past_a_box_that_does_not_clip_it() {
    for clips in [false, true] {
      let mut canned = Canned::new();
      let mut parent = laid(0., 0., 10., 5.);

      parent.clips = clips;
      parent.placed.overflow = rect(0., 0., 18., 5.);
      canned.add(ROOT, node(1), Some(parent));
      canned.add(node(1), node(2), Some(laid(8., 1., 10., 2.)));

      let frames = Frames::build(&canned, Frames::default());
      let expected = if clips { ROOT } else { node(2) };

      assert_eq!(frames.topmost(at(15.5, 2.5)), Some(expected));
      assert_eq!(frames.topmost(at(9.5, 2.5)), Some(node(2)));
    }
  }

  /// An invisible box and everything beneath it are passed over for what lies under them.
  #[test]
  fn an_invisible_subtree_is_passed_over() {
    let mut canned = Canned::new();
    let mut hidden = laid(0., 0., 10., 10.);

    hidden.invisible = true;
    canned.add(ROOT, node(1), Some(laid(0., 0., 4., 4.)));
    canned.add(ROOT, node(2), Some(hidden));
    canned.add(node(2), node(3), Some(laid(1., 1., 5., 5.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.topmost(at(2.5, 2.5)), Some(node(1)));
    assert_eq!(frames.topmost(at(5.5, 5.5)), Some(ROOT));
  }

  /// Open overlays take hits above the flow, the last opened first.
  #[test]
  fn overlays_take_hits_topmost_first() {
    let mut canned = Canned::new();

    canned.add(ROOT, node(1), Some(laid(0., 0., 20., 10.)));
    canned.add_open_overlay(
      node(1),
      node(2),
      laid(0., 0., 6., 4.),
      Place::At(at(2., 2.)),
    );
    canned.add_open_overlay(ROOT, node(3), laid(0., 0., 6., 4.), Place::At(at(5., 3.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.topmost(at(6.5, 4.5)), Some(node(3)));
    assert_eq!(frames.topmost(at(2.5, 2.5)), Some(node(2)));
    assert_eq!(frames.topmost(at(15.5, 8.5)), Some(node(1)));
  }

  /// Nothing beneath the topmost modal takes a hit, the flow and the overlays opened before
  /// it included, while the modal and an overlay opened above it still do.
  #[test]
  fn nothing_beneath_a_modal_takes_a_hit() {
    let mut canned = Canned::new();

    canned.add(ROOT, node(1), Some(laid(0., 0., 20., 10.)));
    canned.add_open_overlay(ROOT, node(2), laid(0., 0., 4., 4.), Place::At(at(0., 0.)));
    canned.add_open_overlay(ROOT, node(3), laid(0., 0., 6., 4.), Place::At(at(5., 3.)));
    canned.add_open_overlay(ROOT, node(4), laid(0., 0., 3., 3.), Place::At(at(10., 6.)));
    canned.make_modal(node(3));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.topmost(at(6.5, 4.5)), Some(node(3)));
    assert_eq!(frames.topmost(at(11.5, 7.5)), Some(node(4)));
    assert_eq!(frames.topmost(at(1.5, 1.5)), None);
    assert_eq!(frames.topmost(at(15.5, 1.5)), None);
  }

  /// A hit inside an overlay climbs its laid-out parents to the overlay and no farther,
  /// and a subtree's hit looks in that subtree alone.
  #[test]
  fn the_path_is_cut_at_the_overlay() {
    let mut canned = Canned::new();

    canned.add(ROOT, node(1), Some(laid(0., 0., 20., 10.)));
    canned.add_open_overlay(
      node(1),
      node(2),
      laid(0., 0., 6., 4.),
      Place::At(at(2., 2.)),
    );
    canned.add(node(2), node(3), Some(laid(1., 1., 2., 2.)));

    let frames = Frames::build(&canned, Frames::default());
    let deepest = frames.topmost(at(3.5, 3.5)).expect("a hit");
    let mut path = vec![deepest];

    while let Some(parent) = frames
      .get(path[path.len() - 1])
      .and_then(|frame| frame.parent)
    {
      path.push(frames.entries()[parent].node);
    }

    assert_eq!(path, [node(3), node(2)]);
    assert_eq!(
      frames.hit_subtree(node(1), at(3.5, 3.5), |_| None),
      Some(node(1))
    );
    assert_eq!(
      frames.hit_subtree(node(2), at(3.5, 3.5), |_| None),
      Some(node(3))
    );
  }

  /// A paragraph's entry stands for what it holds: the scan enters none of it, the node
  /// its flow resolves the point to takes the hit, else the paragraph where its box holds
  /// the point, else what lies beneath it.
  #[test]
  fn a_paragraph_answers_through_its_flow() {
    let mut canned = Canned::new();
    let mut boxed = laid(2., 0., 3., 1.);
    let mut paragraph = laid(0., 0., 10., 2.);

    boxed.inline = Inline::Box;
    paragraph.placed.overflow = rect(0., 0., 12., 2.);
    canned.add(ROOT, node(1), Some(laid(0., 0., 20., 1.)));
    canned.add(ROOT, node(2), Some(paragraph)).paragraph = true;
    canned.add(node(2), node(3), Some(boxed));

    let frames = Frames::build(&canned, Frames::default());
    let resolved = |answer: Option<NodeId>, point| {
      frames.hit(point, |frame| {
        assert_eq!(frame.node, node(2));

        answer
      })
    };

    assert_eq!(resolved(Some(node(9)), at(3.5, 0.5)), Some(node(9)));
    assert_eq!(resolved(None, at(3.5, 0.5)), Some(node(2)));
    assert_eq!(resolved(None, at(11.5, 0.5)), Some(node(1)));
    assert_eq!(resolved(Some(node(9)), at(11.5, 0.5)), Some(node(9)));
  }

  /// A paragraph that clips clips its inline boxes' hits at its padding box: a point in a
  /// box past the clip misses it, where the paragraph takes the hit inside its own box and
  /// what lies beneath it outside.
  #[test]
  fn a_clipping_paragraph_clips_its_inline_boxes() {
    let mut canned = Canned::new();
    let mut paragraph = laid(0., 0., 10., 4.);
    let mut boxed = laid(6., 1., 5., 2.);

    paragraph.clips = true;
    paragraph.placed.border = Inset::all(1.);
    boxed.inline = Inline::Box;
    canned.add(ROOT, node(1), Some(paragraph)).paragraph = true;
    canned.add(node(1), node(2), Some(boxed));

    let frames = Frames::build(&canned, Frames::default());
    let hit = |point| frames.hit(point, |_| frames.hit_subtree(node(2), point, |_| None));

    assert_eq!(
      frames.get(node(2)).and_then(|frame| frame.clip),
      Some(rect(1., 1., 8., 2.))
    );
    assert_eq!(
      frames.hit_subtree(node(2), at(7.5, 1.5), |_| None),
      Some(node(2))
    );
    assert_eq!(frames.hit_subtree(node(2), at(9.5, 1.5), |_| None), None);
    assert_eq!(hit(at(7.5, 2.5)), Some(node(2)));
    assert_eq!(hit(at(9.5, 2.5)), Some(node(1)));
    assert_eq!(hit(at(10.5, 2.5)), Some(ROOT));
  }

  /// A scrolled paragraph moves its inline boxes by its offsets, as it moves its flow, and
  /// keeps its own place.
  #[test]
  fn a_scrolled_paragraph_moves_its_inline_boxes() {
    let mut canned = Canned::new();
    let mut paragraph = laid(0., 2., 10., 2.);
    let mut boxed = laid(3., 2., 2., 1.);

    paragraph.clips = true;
    boxed.inline = Inline::Box;

    let added = canned.add(ROOT, node(1), Some(paragraph));

    added.paragraph = true;
    added.scroll = Point::new(0., 2.);
    canned.add(node(1), node(2), Some(boxed));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.rect(node(1)), Some(rect(0., 2., 10., 2.)));
    assert_eq!(frames.rect(node(2)), Some(rect(3., 2., 2., 1.)));
  }

  /// One step of a walk, as a recording visitor hears it.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  enum Step {
    /// An entry entered, at its index.
    Enter(usize, NodeId),
    /// An entry left, at its index.
    Leave(usize, NodeId),
    /// An open overlay's layer announced.
    Layer(NodeId),
  }

  /// A visitor recording each step it hears and each entry's frame as it is entered,
  /// passing over what the nodes of `skip` hold.
  #[derive(Default)]
  struct Recorder {
    skip: Vec<NodeId>,
    steps: Vec<Step>,
    entered: Vec<(usize, Frame)>,
  }

  impl FrameVisitor for Recorder {
    fn enter(&mut self, index: usize, frame: &Frame) -> Descend {
      self.steps.push(Step::Enter(index, frame.node));
      self.entered.push((index, *frame));

      if self.skip.contains(&frame.node) {
        Descend::Skip
      }
      else {
        Descend::Into
      }
    }

    fn leave(&mut self, index: usize, frame: &Frame) {
      self.steps.push(Step::Leave(index, frame.node));
    }

    fn layer(&mut self, layer: &Layer) {
      self.steps.push(Step::Layer(layer.overlay));
    }
  }

  impl Frames {
    /// What a walk passing over what the nodes of `skip` hold hears, every entry's frame
    /// held to the entry at the index it is handed with.
    fn walked(&self, skip: &[NodeId]) -> Recorder {
      let mut recorder = Recorder {
        skip: skip.to_vec(),
        ..Recorder::default()
      };

      self.visit(&mut recorder);

      for (index, frame) in &recorder.entered {
        assert_eq!(&self.entries()[*index], frame);
      }

      recorder
    }
  }

  /// A canned window of nested boxes, an anchor among them: the root holding the box 1
  /// and then the box 5, the box 1 holding the box 3 through the anchor 2 and then the
  /// box 4.
  fn nested() -> Canned {
    let mut canned = Canned::new();

    canned.add(ROOT, node(1), Some(laid(0., 0., 20., 10.)));
    canned.add(node(1), node(2), None);
    canned.add(node(2), node(3), Some(laid(1., 1., 4., 2.)));
    canned.add(node(1), node(4), Some(laid(1., 4., 4., 2.)));
    canned.add(ROOT, node(5), Some(laid(0., 12., 4., 2.)));

    canned
  }

  /// The walk enters each entry before what it holds and leaves it after, in nesting
  /// order, an anchor neither entered nor left and what it holds entered in its place.
  #[test]
  fn the_walk_leaves_an_entry_after_what_it_holds() {
    let frames = Frames::build(&nested(), Frames::default());

    assert_eq!(
      frames.walked(&[]).steps,
      [
        Step::Enter(0, ROOT),
        Step::Enter(1, node(1)),
        Step::Enter(2, node(3)),
        Step::Leave(2, node(3)),
        Step::Enter(3, node(4)),
        Step::Leave(3, node(4)),
        Step::Leave(1, node(1)),
        Step::Enter(4, node(5)),
        Step::Leave(4, node(5)),
        Step::Leave(0, ROOT),
      ]
    );
  }

  /// An entry entered answering `Skip` is neither left nor has anything beneath it
  /// entered, and the walk goes on past it.
  #[test]
  fn a_skipped_entry_passes_over_its_subtree() {
    let frames = Frames::build(&nested(), Frames::default());

    assert_eq!(
      frames.walked(&[node(1)]).steps,
      [
        Step::Enter(0, ROOT),
        Step::Enter(1, node(1)),
        Step::Enter(4, node(5)),
        Step::Leave(4, node(5)),
        Step::Leave(0, ROOT),
      ]
    );
  }

  /// An invisible box and everything beneath it are neither entered nor left.
  #[test]
  fn the_walk_never_enters_an_invisible_subtree() {
    let mut canned = Canned::new();
    let mut hidden = laid(0., 0., 10., 10.);

    hidden.invisible = true;
    canned.add(ROOT, node(1), Some(hidden));
    canned.add(node(1), node(2), Some(laid(1., 1., 2., 2.)));
    canned.add(ROOT, node(3), Some(laid(0., 12., 4., 2.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(
      frames.walked(&[]).steps,
      [
        Step::Enter(0, ROOT),
        Step::Enter(3, node(3)),
        Step::Leave(3, node(3)),
        Step::Leave(0, ROOT),
      ]
    );
  }

  /// Each open overlay's layer comes after the root's flow, in the order they opened,
  /// announced before its entries and holding them alone, each entry handed at its index.
  #[test]
  fn layers_follow_the_flow_in_open_order() {
    let mut canned = Canned::new();

    canned.add(ROOT, node(1), Some(laid(0., 0., 20., 10.)));
    canned.add_open_overlay(
      node(1),
      node(2),
      laid(0., 0., 6., 4.),
      Place::At(at(2., 2.)),
    );
    canned.add(node(2), node(3), Some(laid(1., 1., 2., 2.)));
    canned.add_open_overlay(ROOT, node(4), laid(0., 0., 3., 3.), Place::At(at(9., 5.)));
    canned.add(ROOT, node(5), Some(laid(0., 12., 4., 2.)));

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(
      frames.walked(&[]).steps,
      [
        Step::Enter(0, ROOT),
        Step::Enter(1, node(1)),
        Step::Leave(1, node(1)),
        Step::Enter(2, node(5)),
        Step::Leave(2, node(5)),
        Step::Leave(0, ROOT),
        Step::Layer(node(2)),
        Step::Enter(3, node(2)),
        Step::Enter(4, node(3)),
        Step::Leave(4, node(3)),
        Step::Leave(3, node(2)),
        Step::Layer(node(4)),
        Step::Enter(5, node(4)),
        Step::Leave(5, node(4)),
      ]
    );
  }

  /// A paragraph's inline boxes are entered after it and before it is left, a run flowing
  /// in it having no entry, each box handed with what shows of the paragraph's content as
  /// its clip.
  #[test]
  fn a_paragraphs_inline_boxes_are_entered_within_its_content_clip() {
    let mut canned = Canned::new();
    let mut paragraph = laid(0., 0., 12., 4.);
    let mut run = laid(0., 0., 5., 1.);
    let mut boxed = laid(6., 1., 5., 2.);

    paragraph.clips = true;
    paragraph.placed.border = Inset::new(1, 1, 2, 0);
    run.inline = Inline::Run;
    boxed.inline = Inline::Box;
    canned.add(ROOT, node(1), Some(paragraph)).paragraph = true;
    canned.add(node(1), node(2), Some(run));
    canned.add(node(1), node(3), Some(boxed));

    let frames = Frames::build(&canned, Frames::default());
    let walked = frames.walked(&[]);
    let content_clip = frames.get(node(1)).and_then(|frame| frame.content_clip);

    assert_eq!(
      walked.steps,
      [
        Step::Enter(0, ROOT),
        Step::Enter(1, node(1)),
        Step::Enter(2, node(3)),
        Step::Leave(2, node(3)),
        Step::Leave(1, node(1)),
        Step::Leave(0, ROOT),
      ]
    );
    assert_eq!(content_clip, Some(rect(1., 1., 9., 3.)));
    assert_eq!(walked.entered[2].1.clip, content_clip);
  }

  /// What shows of a box's content is what shows of it where it does not clip, and that
  /// cut to its padding box, inside borders of any widths, where it does.
  #[test]
  fn the_content_clip_cuts_at_a_clipping_padding_box() {
    let mut canned = Canned::new();
    let mut outer = laid(1., 1., 20., 10.);
    let mut inner = laid(10., 1., 8., 6.);
    let mut overflowing = laid(18., 7., 6., 6.);

    outer.clips = true;
    inner.clips = true;
    inner.placed.border = Inset::new(1, 2, 3, 0);
    overflowing.clips = true;
    overflowing.placed.border = Inset::all(1.);
    canned.add(ROOT, node(1), Some(outer));
    canned.add(node(1), node(2), Some(laid(2., 2., 6., 4.)));
    canned.add(node(1), node(3), Some(inner));
    canned.add(node(1), node(4), Some(overflowing));

    let frames = Frames::build(&canned, Frames::default());
    let clips = |index| {
      frames
        .get(node(index))
        .map(|frame| (frame.clip, frame.content_clip))
    };

    assert_eq!(clips(0), Some((None, None)));
    assert_eq!(clips(1), Some((None, Some(rect(1., 1., 20., 10.)))));
    assert_eq!(
      clips(2),
      Some((Some(rect(1., 1., 20., 10.)), Some(rect(1., 1., 20., 10.))))
    );
    assert_eq!(
      clips(3),
      Some((Some(rect(1., 1., 20., 10.)), Some(rect(12., 4., 4., 4.))))
    );
    assert_eq!(
      clips(4),
      Some((Some(rect(1., 1., 20., 10.)), Some(rect(20., 9., 1., 2.))))
    );
  }

  /// The pruned hit held to looking at every entry, over random windows.
  #[cfg(not(target_arch = "wasm32"))]
  mod pruned {
    use proptest::{
      collection::vec,
      prelude::*,
      test_runner::{Config as ProptestConfig, RngSeed},
    };
    use skene_foundation::{Inset, NodeId, Point, Rect};

    use super::{Canned, Frame, Frames, Laid, Place, ROOT, at, laid, node, rect};

    impl Frames {
      /// The topmost node at `point` found by looking at every entry: the last in paint
      /// order that takes hits and whose border box holds the point within its clip.
      fn topmost_unpruned(&self, point: Point<f32>) -> Option<NodeId> {
        let takes = |frame: &&Frame| {
          frame.hits
            && frame.rect.contains(point)
            && frame.clip.is_none_or(|clip| clip.contains(point))
        };

        for layer in self.layers().iter().rev() {
          if let Some(frame) = self.entries()[layer.start..layer.end]
            .iter()
            .rev()
            .find(takes)
          {
            return Some(frame.node);
          }

          if layer.modal {
            return None;
          }
        }

        self.entries()[..self.flow_len()]
          .iter()
          .rev()
          .find(takes)
          .map(|frame| frame.node)
      }
    }

    /// A random laid-out box: its place in its parent, its size, whether it clips, has a
    /// border and is invisible, and the offsets it is scrolled by where it clips.
    #[derive(Debug, Clone, Copy)]
    struct Laying {
      x: f32,
      y: f32,
      width: f32,
      height: f32,
      clips: bool,
      bordered: bool,
      invisible: bool,
      scroll: Point<f32>,
    }

    /// A random subtree: a laid-out box, or an anchor or a scope where `laying` is `None`, and
    /// the subtrees it holds.
    #[derive(Debug, Clone)]
    struct Shape {
      laying: Option<Laying>,
      children: Vec<Self>,
    }

    /// A random open overlay: its subtree, whether it is modal and where it is placed.
    #[derive(Debug, Clone)]
    struct Opened {
      shape: Shape,
      modal: bool,
      at: Point<f32>,
    }

    /// Any laid-out box: a quarter clip, half of those behind a border, and one in ten is
    /// invisible.
    fn laying() -> impl Strategy<Value = Laying> {
      (
        (0..30_u8, 0..20_u8, 1..13_u8, 1..9_u8),
        (0..4_u8, any::<bool>(), 0..10_u8),
        (0..3_u8, 0..3_u8),
      )
        .prop_map(
          |((x, y, width, height), (clip, bordered, shown), (left, top))| {
            let clips = clip == 0;

            Laying {
              x: f32::from(x) - 5.,
              y: f32::from(y) - 5.,
              width: f32::from(width),
              height: f32::from(height),
              clips,
              bordered: clips && bordered,
              invisible: shown == 0,
              scroll: if clips {
                Point::new(f32::from(left), f32::from(top))
              }
              else {
                Point::ZERO
              },
            }
          },
        )
    }

    /// Any subtree `depth` deep: below the top, one in eight an anchor or a scope, and each
    /// holding up to three subtrees a level shallower.
    fn shape(depth: u32) -> BoxedStrategy<Shape> {
      if depth == 0 {
        return laying()
          .prop_map(|laying| Shape {
            laying: Some(laying),
            children: Vec::new(),
          })
          .boxed();
      }

      (0..8_u8, laying(), vec(shape(depth - 1), 0..4))
        .prop_map(|(anchor, laying, children)| Shape {
          laying: (anchor != 0).then_some(laying),
          children,
        })
        .boxed()
    }

    /// Any open overlay two deep, one in three modal, placed in the window.
    fn opened() -> impl Strategy<Value = Opened> {
      (shape(2), 0..3_u8, 0..30_u8, 0..16_u8).prop_map(|(shape, modal, x, y)| Opened {
        shape,
        modal: modal == 0,
        at: at(f32::from(x), f32::from(y)),
      })
    }

    impl Canned {
      /// The laid-out description of `node`, mutably.
      fn laid_mut(&mut self, node: NodeId) -> &mut Laid {
        self
          .nodes
          .get_mut(&node)
          .and_then(|node| node.laid.as_mut())
          .expect("a laid-out node")
      }

      /// Adds `shape` beneath `parent`, numbering its nodes from `next`, each box's overflow
      /// reaching what it holds where it does not clip; the rectangle its laid-out nodes
      /// reach from the corner of the laid-out box it sits in.
      fn plant(&mut self, shape: &Shape, parent: NodeId, next: &mut u32) -> Option<Rect<f32>> {
        let id = node(*next);

        *next += 1;

        let Some(laying) = shape.laying
        else {
          self.add(parent, id, None);

          return shape
            .children
            .iter()
            .filter_map(|child| self.plant(child, id, next))
            .reduce(Rect::union);
        };
        let mut box_laid = laid(laying.x, laying.y, laying.width, laying.height);

        box_laid.clips = laying.clips;
        box_laid.invisible = laying.invisible;

        if laying.bordered {
          box_laid.placed.border = Inset::all(1.);
        }

        let own = Rect::new(Point::ZERO, box_laid.placed.rect.size);
        let location = box_laid.placed.rect.origin;

        self.add(parent, id, Some(box_laid)).scroll = laying.scroll;

        let held = shape
          .children
          .iter()
          .filter_map(|child| self.plant(child, id, next))
          .fold(own, Rect::union);
        let reach = if laying.clips { own } else { held };
        let border = box_laid.placed.border;

        self.laid_mut(id).placed.overflow = reach.translate(Point::new(-border.start, -border.top));

        Some(reach.translate(location))
      }
    }

    proptest! {
      // Every run tries the same cases, and a failure is kept by no file.
      #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x00f4_a3e5),
        failure_persistence: None,
        ..ProptestConfig::default()
      })]

      /// A hit that passes over every subtree its reach or its clip misses finds what looking
      /// at every entry finds, at every point of any tree, beneath any open overlays, modal
      /// or not.
      #[test]
      fn a_pruned_hit_equals_an_unpruned_one(
        flow in vec(shape(3), 0..5),
        overlays in vec(opened(), 0..3),
      ) {
        let mut canned = Canned::new();
        let mut next = 1;
        let mut held = flow
          .iter()
          .filter_map(|shape| canned.plant(shape, ROOT, &mut next))
          .fold(rect(0., 0., 40., 20.), Rect::union);

        for overlay in &overlays {
          let top = node(next);

          if let Some(reach) = canned.plant(&overlay.shape, ROOT, &mut next) {
            held = held.union(reach);
          }

          if overlay.shape.laying.is_none() {
            continue;
          }

          canned.laid_mut(top).overlay = true;
          canned.open.push((top, overlay.modal));
          canned.places.insert(top, Place::At(overlay.at));
        }

        canned.laid_mut(ROOT).placed.overflow = held;

        let frames = Frames::build(&canned, Frames::default());

        for y in -12_i16..60 {
          for x in -12_i16..90 {
            let point = at(f32::from(x) / 2. + 0.25, f32::from(y) / 2. + 0.25);

            prop_assert_eq!(
              frames.topmost(point),
              frames.topmost_unpruned(point),
              "at {:?}",
              point
            );
          }
        }
      }
    }
  }
}
