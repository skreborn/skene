//! The drawn runtime's index of window rectangles: every laid-out node's border box in
//! its window, the cover its paint damages, the reach a hit takes it at, the clip what
//! shows of it is cut to and whether it takes hits, in paint order, each open overlay's
//! subtree placed where the runtime places it.

use skene_foundation::{Inset, NodeId, NodeMap, Point, Rect, RunPlacement, Scene, Size};
use smallvec::SmallVec;

use super::{Drawn, Kind, Runtime};
use crate::Placed;

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
  /// Where a hit may take it or what it holds in the window: its border box united with
  /// its scrollable overflow.
  pub reach: Rect<f32>,
  /// What shows of it in the window: the padding boxes of every laid-out node above it in
  /// its layer that clips, intersected; `None` where none does.
  pub clip: Option<Rect<f32>>,
  /// Whether it takes hits: neither it nor a laid-out node above it in its layer is
  /// invisible.
  pub hits: bool,
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

/// Where the index keeps a node.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Slot {
  /// An entry, at its index.
  Entry(usize),
  /// A run flowing in a paragraph, which has no entry: placed at `offset` from the entry
  /// at `owner`, the laid-out node it flows under.
  Run { owner: usize, offset: Point<f32> },
}

/// The drawn runtime's index of one window's rectangles ([`Runtime::frames`]).
///
/// It holds an entry ([`Frame`]) per laid-out node in paint order, the root's flow in the
/// scene's child order, overlays left out of it, then each open overlay's subtree in the
/// order they opened ([`Layer`]), with a lookup by node.
///
/// A run flowing in a paragraph has no entry: it answers its paragraph's origin, with
/// no size. A closed overlay and what it holds have none either.
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

  /// `node`'s border box in the window: its entry's, or its paragraph's origin with no
  /// size for a run flowing in one; `None` for a node the index does not hold.
  #[must_use]
  pub fn rect(&self, node: NodeId) -> Option<Rect<f32>> {
    match *self.slots.get(node)? {
      Slot::Entry(index) => Some(self.entries[index].rect),
      Slot::Run { owner, offset } => {
        let origin = self.entries[owner].rect.origin;

        Some(Rect::new(
          Point::new(origin.x + offset.x, origin.y + offset.y),
          Size::ZERO,
        ))
      }
    }
  }

  /// `node`'s origin in the window, as [`rect`](Self::rect) places it.
  #[must_use]
  pub fn origin(&self, node: NodeId) -> Option<Point<f32>> {
    self.rect(node).map(|rect| rect.origin)
  }

  /// What `node`'s paint covers in the window ([`Frame::cover`]); `None` for a node of no
  /// extent or one without an entry.
  #[must_use]
  pub fn cover(&self, node: NodeId) -> Option<Rect<f32>> {
    self.get(node)?.cover
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
      let parent = if flowing {
        self.flows(visit, rect.origin);

        visit.parent
      }
      else {
        Some(self.push(visit, &laid, rect, hits))
      };

      let children = source.children(visit.node);

      if children.is_empty() {
        continue;
      }

      let clip = if laid.clips {
        let border = laid.placed.border;
        let padding = Rect::new(
          Point::new(rect.origin.x + border.start, rect.origin.y + border.top),
          Size::new(
            (rect.size.width - border.start - border.end).max(0.),
            (rect.size.height - border.top - border.bottom).max(0.),
          ),
        );

        Some(visit.clip.map_or(padding, |clip| clip.intersect(padding)))
      }
      else {
        visit.clip
      };
      let scroll = source.scroll(visit.node);
      let base = Point::new(rect.origin.x - scroll.x, rect.origin.y - scroll.y);
      let flows = source.is_paragraph(visit.node) || (laid.inline != Inline::None && visit.flows);

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

  /// Indexes the run `visit` reaches as flowing at `origin` under the entry it flows
  /// beneath, with no entry of its own.
  fn flows(&mut self, visit: Visit, origin: Point<f32>) {
    if let Some(owner) = visit.parent {
      let at = self.entries[owner].rect.origin;
      let _replaced = self.slots.insert(
        visit.node,
        Slot::Run {
          owner,
          offset: Point::new(origin.x - at.x, origin.y - at.y),
        },
      );
    }
  }

  /// Pushes the entry of the node `visit` reaches, laid out as `laid` at `rect` and
  /// taking hits where `hits` says, returning its index.
  fn push(&mut self, visit: Visit, laid: &Laid, rect: Rect<f32>, hits: bool) -> usize {
    let index = self.entries.len();
    let extent = rect.size.width > 0. && rect.size.height > 0.;
    let overflow = laid.placed.overflow.translate(Point::new(
      rect.origin.x + laid.placed.border.start,
      rect.origin.y + laid.placed.border.top,
    ));

    self.entries.push(Frame {
      node: visit.node,
      rect,
      cover: extent.then(|| rect.inflate(laid.reach)),
      reach: rect.union(overflow),
      clip: visit.clip,
      hits,
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
    }

    placing[layer] = Placing::Done;
  }

  /// The window rectangle of the anchor `anchor`, its layer placed first where it lies
  /// in one not yet placed; empty at the window's origin for a node the index does not
  /// hold.
  fn anchor_rect(
    &mut self,
    source: &impl FrameSource,
    anchor: NodeId,
    placing: &mut [Placing],
  ) -> Rect<f32> {
    let index = match self.slots.get(anchor) {
      Some(Slot::Entry(index)) => *index,
      Some(Slot::Run { owner, .. }) => *owner,
      None => return Rect::ZERO,
    };

    if let Some(layer) = self
      .layers
      .iter()
      .position(|layer| (layer.start..layer.end).contains(&index))
    {
      self.place(source, layer, placing);
    }

    self.rect(anchor).unwrap_or(Rect::ZERO)
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

/// What [`Frames`] is built from: a window's scene, its records and its layout.
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

  fn scroll(&self, node: NodeId) -> Point<f32> {
    let offset = self.scroll_offset(node);

    if P::ROUNDED {
      Point::new(offset.x.round(), offset.y.round())
    }
    else {
      offset
    }
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
}

impl<P: Drawn> Runtime<P> {
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
}

#[cfg(test)]
mod tests {
  use rustc_hash::FxHashMap;
  use skene_foundation::{Inset, NodeId, Point, Rect, Size};
  use smallvec::SmallVec;

  use super::{FrameSource, Frames, Inline, Laid};
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

  /// A canned window: its nodes, its open overlays in order and where each is placed.
  #[derive(Default)]
  struct Canned {
    nodes: FxHashMap<NodeId, Node>,
    open: Vec<(NodeId, bool)>,
    places: FxHashMap<NodeId, Place>,
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

  /// A run flowing in a paragraph has no entry and answers its paragraph's origin with no
  /// size, through any group around it; an inline box is placed in the paragraph.
  #[test]
  fn a_flowing_run_answers_its_paragraphs_origin() {
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

    let frames = Frames::build(&canned, Frames::default());

    assert_eq!(frames.get(node(3)), None);
    assert_eq!(frames.rect(node(2)), Some(rect(2., 4., 0., 0.)));
    assert_eq!(frames.rect(node(3)), Some(rect(2., 4., 0., 0.)));
    assert_eq!(frames.rect(node(4)), Some(rect(5., 4., 2., 1.)));
    assert_eq!(frames.get(node(4)).and_then(|frame| frame.parent), Some(1));
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
}
