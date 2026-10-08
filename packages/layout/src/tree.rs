//! A layout tree keyed by the scene's nodes, retained across layouts: each node's style,
//! context and children written where they changed, node by node or by a walk of the
//! scene, each node's part as its platform classifies it.

use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};
use skene_foundation::{NodeId, NodeMap, Platform, Point, Rect, Scene, Size};
use taffy::{
  TaffyTree,
  prelude::{AvailableSpace, length},
  style::{Display, FlexDirection, Overflow, Style},
  tree::{Layout, LayoutOutput},
};

/// What a leaf of the layout carries: what measures it and places what flows inside it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a leaf of the layout",
  note = "a leaf's context measures the leaf and places the nodes flowing inside it"
)]
pub trait Leaf {
  /// The size the leaf takes within its `known` dimensions and the space `available`.
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32>;

  /// Where each node flowing inside the leaf lays out, in the order they paint, the
  /// leaf's content box laid out at `content`; a node may take several rectangles, and
  /// a leaf places none by default.
  fn place(&self, content: Rect<f32>) -> Vec<(NodeId, Rect<f32>)> {
    let _ = content;

    Vec::new()
  }

  /// The nodes flowing inside the leaf as boxes, each laid out on its own at its
  /// content's size before the leaf measures and placed where the leaf places it; none
  /// by default.
  fn boxes(&self) -> Vec<NodeId> {
    Vec::new()
  }

  /// Gives the box `node` flowing inside the leaf the size it laid out at.
  fn size_box(&mut self, node: NodeId, size: Size<f32>) {
    let _ = (node, size);
  }
}

/// What a node is to the layout, with the platform's leaf context `C`.
#[derive(Debug)]
pub enum Part<C> {
  /// A box laid out with its own taffy style, its children laid out inside it and
  /// shifted back by its scroll offsets.
  Box(Style, Point<f32>),
  /// A leaf laid out with its own taffy style, measured from its context.
  Leaf(Style, C),
  /// An overlay: laid out with its own taffy style out of every flow, under the root,
  /// its children shifted back by its scroll offsets.
  Overlay(Style, Point<f32>),
  /// A node whose children lay out in its place, as an anchor's or a scope's do.
  Transparent,
  /// A node left out of the layout with everything beneath it.
  Absent,
}

/// Each node's rectangle from one layout, in the viewport's coordinates, what clips it,
/// and the order the nodes paint in.
///
/// A box clipping its overflow along an axis clips everything beneath it there to its
/// padding box, and a box's scroll offsets shift everything beneath it back by them. A
/// node a leaf places in several rectangles, as a run broken across lines, covers each
/// and spans them all.
#[derive(Debug, Default, Clone)]
pub struct Frames {
  rects: FxHashMap<NodeId, Rect<f32>>,
  clips: FxHashMap<NodeId, Bounds>,
  painted: Vec<NodeId>,
  pieces: Vec<(NodeId, Rect<f32>)>,
}

impl Frames {
  /// The rectangle `node` spans, its ancestors' scroll offsets applied; `None` for a
  /// node the layout left out or never laid out on its own.
  #[must_use]
  pub fn rect(&self, node: NodeId) -> Option<Rect<f32>> {
    self.rects.get(&node).copied()
  }

  /// The part of `node`'s rectangle the boxes clipping it leave showing, empty where
  /// they clip it away; `None` for a node the layout left out or never laid out on its
  /// own.
  #[must_use]
  pub fn visible(&self, node: NodeId) -> Option<Rect<f32>> {
    let rect = self.rect(node)?;

    Some(
      self
        .clips
        .get(&node)
        .map_or(rect, |clip| clip.intersect(rect)),
    )
  }

  /// The topmost node laid out at `point`: the last to paint a rectangle holding it
  /// where the boxes clipping it leave it showing, a child above its parent, a later
  /// sibling above an earlier one and an overlay above the flow.
  #[must_use]
  pub fn hit(&self, point: Point<f32>) -> Option<NodeId> {
    self
      .pieces
      .iter()
      .rev()
      .find(|(node, rect)| {
        Bounds::of(*rect).contains(point)
          && self.clips.get(node).is_none_or(|clip| clip.contains(point))
      })
      .map(|(node, _)| *node)
  }

  /// Adds every rectangle of `other`, painting above what this holds.
  pub fn extend(&mut self, other: Self) {
    self.rects.extend(other.rects);
    self.clips.extend(other.clips);
    self.painted.extend(other.painted);
    self.pieces.extend(other.pieces);
  }

  /// Records that `node` covers `rect` where `clip` leaves it showing, painting above
  /// everything recorded before.
  fn add(&mut self, node: NodeId, rect: Rect<f32>, clip: Bounds) {
    if let Some(spanned) = self.rects.get_mut(&node) {
      *spanned = Bounds::of(*spanned).union(rect);
    }
    else {
      self.rects.insert(node, rect);
      self.painted.push(node);
    }

    if clip != Bounds::UNBOUNDED {
      self.clips.insert(node, clip);
    }

    self.pieces.push((node, rect));
  }

  /// Every node laid out, with its rectangle, in the order they paint.
  pub fn iter(&self) -> impl Iterator<Item = (NodeId, Rect<f32>)> + '_ {
    self
      .painted
      .iter()
      .filter_map(|node| Some((*node, *self.rects.get(node)?)))
  }
}

/// The region a clip leaves showing, unbounded along an axis nothing clips.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
  min: Point<f32>,
  max: Point<f32>,
}

impl Bounds {
  /// The region nothing clips.
  const UNBOUNDED: Self = Self {
    min: Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
    max: Point::new(f32::INFINITY, f32::INFINITY),
  };

  /// The region `rect` covers.
  const fn of(rect: Rect<f32>) -> Self {
    Self {
      min: rect.origin,
      max: Point::new(
        rect.origin.x + rect.size.width,
        rect.origin.y + rect.size.height,
      ),
    }
  }

  /// Whether `point` lies inside, the far edges outside.
  fn contains(self, point: Point<f32>) -> bool {
    point.x >= self.min.x && point.y >= self.min.y && point.x < self.max.x && point.y < self.max.y
  }

  /// The smallest rectangle holding this region and `rect`.
  fn union(self, rect: Rect<f32>) -> Rect<f32> {
    let covered = Self::of(rect);
    let min = Point::new(self.min.x.min(covered.min.x), self.min.y.min(covered.min.y));
    let max = Point::new(self.max.x.max(covered.max.x), self.max.y.max(covered.max.y));

    Rect {
      origin: min,
      size: Size::new(max.x - min.x, max.y - min.y),
    }
  }

  /// The part of `rect` inside, empty where none is.
  fn intersect(self, rect: Rect<f32>) -> Rect<f32> {
    let covered = Self::of(rect);
    let min = Point::new(self.min.x.max(covered.min.x), self.min.y.max(covered.min.y));
    let max = Point::new(self.max.x.min(covered.max.x), self.max.y.min(covered.max.y));

    Rect {
      origin: min,
      size: Size::new((max.x - min.x).max(0.), (max.y - min.y).max(0.)),
    }
  }
}

/// A layout tree of the scene's nodes, its leaves measured from their contexts `C`.
///
/// A node takes part once inserted, as a leaf of the default style with no context
/// and no children, until it is removed, or while a walk of the scene
/// ([`lay_out`](Self::lay_out)) reaches it. Writing a node's style, context or children
/// marks it for the next layout only where the value differs from the one it holds.
#[must_use]
pub struct Tree<C> {
  taffy: TaffyTree<C>,
  nodes: NodeMap<Slot>,
  tops: Vec<taffy::NodeId>,
}

/// What one walk of the scene carries: what answers each node's part, the overlays set
/// aside for the root with their parts, and every node it reached.
struct Walk<F, C> {
  part: F,
  overlays: VecDeque<(NodeId, Part<C>)>,
  reached: FxHashSet<NodeId>,
}

/// A node's place in the taffy tree: its taffy node, the taffy children last set and
/// the offsets it scrolls its children by.
struct Slot {
  taffy: taffy::NodeId,
  children: Vec<taffy::NodeId>,
  scroll: Point<f32>,
}

impl<C> std::fmt::Debug for Tree<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Tree")
      .field("nodes", &self.nodes.values().count())
      .finish_non_exhaustive()
  }
}

impl<C: Leaf> Tree<C> {
  /// An empty tree, each layout rounded to whole units where `rounding` says so.
  pub fn new(rounding: bool) -> Self {
    let mut taffy = TaffyTree::new();

    if !rounding {
      taffy.disable_rounding();
    }

    Self {
      taffy,
      nodes: NodeMap::new(),
      tops: Vec::new(),
    }
  }

  /// Adds `node` as a leaf of the default style with no context and no children; a
  /// node already in the tree stays as it is.
  ///
  /// # Panics
  ///
  /// Panics where taffy cannot allocate the node.
  pub fn insert(&mut self, node: NodeId) {
    if self.nodes.contains(node) {
      return;
    }

    let taffy = self
      .taffy
      .new_leaf(Style::default())
      .expect("allocate a layout node");

    self.nodes.insert(
      node,
      Slot {
        taffy,
        children: Vec::new(),
        scroll: Point::new(0., 0.),
      },
    );
  }

  /// Takes `node` out of the tree, and out of its parent's children.
  pub fn remove(&mut self, node: NodeId) {
    if let Some(slot) = self.nodes.remove(node) {
      let _ = self.taffy.remove(slot.taffy);
    }
  }

  /// Whether `node` takes part in the layout.
  #[must_use]
  pub fn contains(&self, node: NodeId) -> bool {
    self.nodes.contains(node)
  }

  /// Gives `node` the taffy style `style`; `true` where it differs from the one it
  /// held, `false` where it is the same or `node` is outside the tree.
  pub fn set_style(&mut self, node: NodeId, style: Style) -> bool {
    let Some(slot) = self.nodes.get(node)
    else {
      return false;
    };

    if self
      .taffy
      .style(slot.taffy)
      .is_ok_and(|held| *held == style)
    {
      return false;
    }

    self.taffy.set_style(slot.taffy, style).is_ok()
  }

  /// Gives `node` the context its measure reads, or none; `true` where it differs from
  /// the one it held, `false` where it is the same or `node` is outside the tree.
  pub fn set_context(&mut self, node: NodeId, context: Option<C>) -> bool
  where
    C: PartialEq,
  {
    let Some(slot) = self.nodes.get(node)
    else {
      return false;
    };

    if self.taffy.get_node_context(slot.taffy) == context.as_ref() {
      return false;
    }

    self.taffy.set_node_context(slot.taffy, context).is_ok()
  }

  /// The context `node`'s measure reads, to change in place; a change to what it
  /// measures to takes [`mark_dirty`](Self::mark_dirty) as well.
  pub fn context_mut(&mut self, node: NodeId) -> Option<&mut C> {
    let slot = self.nodes.get(node)?;

    self.taffy.get_node_context_mut(slot.taffy)
  }

  /// Marks `node` for the next [`compute`](Self::compute) to measure and lay out
  /// again, with every ancestor.
  pub fn mark_dirty(&mut self, node: NodeId) {
    if let Some(slot) = self.nodes.get(node) {
      let _ = self.taffy.mark_dirty(slot.taffy);
    }
  }

  /// Gives `node` the children `children`, in order, leaving out each outside the tree;
  /// `true` where they differ from the ones it held.
  pub fn set_children(&mut self, node: NodeId, children: impl IntoIterator<Item = NodeId>) -> bool {
    let children: Vec<taffy::NodeId> = children
      .into_iter()
      .filter_map(|child| self.nodes.get(child).map(|slot| slot.taffy))
      .collect();
    let Some(slot) = self.nodes.get_mut(node)
    else {
      return false;
    };

    if slot.children == children {
      return false;
    }

    if self.taffy.set_children(slot.taffy, &children).is_err() {
      return false;
    }

    slot.children = children;

    true
  }

  /// Lays out the tree under `root` in the space `available`, each leaf with a context
  /// measured by it and each without one sized to its known dimensions or nothing.
  pub fn compute(&mut self, root: NodeId, available: taffy::geometry::Size<AvailableSpace>) {
    if let Some(slot) = self.nodes.get(root) {
      let _ = self.compute_at(slot.taffy, available);
    }
  }

  /// `node`'s layout in its parent, as the last [`compute`](Self::compute) left it,
  /// rounded where the tree rounds; `None` for a node outside the tree.
  #[must_use]
  pub fn layout(&self, node: NodeId) -> Option<&Layout> {
    self.taffy.layout(self.nodes.get(node)?.taffy).ok()
  }

  /// `node`'s layout in its parent, as the last [`compute`](Self::compute) left it
  /// before any rounding; `None` for a node outside the tree.
  #[must_use]
  pub fn unrounded_layout(&self, node: NodeId) -> Option<&Layout> {
    Some(self.taffy.unrounded_layout(self.nodes.get(node)?.taffy))
  }

  /// The rectangle of every node laid out at or beneath `root` in the last
  /// [`compute`](Self::compute), from `root`'s origin, each node its leaves place among
  /// them, and the order they paint in.
  #[must_use]
  pub fn frames(&self, root: NodeId) -> Frames {
    let mut frames = Frames::default();

    if let Some(slot) = self.nodes.get(root) {
      self.collect(slot.taffy, &mut frames);
    }

    frames
  }

  /// Lays out the scene of the platform `P` under each of `roots` at `viewport`, first
  /// bringing the tree in line with it: each node's part as `part` answers, its style,
  /// context and children written where they changed, and each node no longer reached
  /// taken out. Each leaf is measured by its context, the nodes it places recorded
  /// beneath it.
  ///
  /// A root lays out in the viewport as a block's child does, stretched across it, and
  /// the open overlays beneath it are hoisted beside it, out of every flow.
  pub fn lay_out<P: Platform>(
    &mut self,
    roots: &[NodeId],
    viewport: Size<f32>,
    part: impl FnMut(NodeId) -> Part<C>,
  ) -> Frames
  where
    C: PartialEq,
  {
    let mut walk = Walk {
      part,
      overlays: VecDeque::new(),
      reached: FxHashSet::default(),
    };
    let viewport_style = Style {
      display: Display::Flex,
      flex_direction: FlexDirection::Column,
      size: taffy::geometry::Size {
        width: length(viewport.width),
        height: length(viewport.height),
      },
      ..Style::default()
    };
    let available = taffy::geometry::Size {
      width: AvailableSpace::Definite(viewport.width),
      height: AvailableSpace::Definite(viewport.height),
    };
    let mut frames = Frames::default();

    for (index, root) in roots.iter().copied().enumerate() {
      let mut children = match (walk.part)(root) {
        Part::Transparent => self.sync_children::<P, _>(root, &mut walk),
        its => self
          .sync::<P, _>(root, its, &mut walk)
          .into_iter()
          .collect(),
      };

      while let Some((overlay, its)) = walk.overlays.pop_front() {
        children.extend(self.sync::<P, _>(overlay, its, &mut walk));
      }

      if index == self.tops.len() {
        let Ok(top) = self.taffy.new_leaf(viewport_style.clone())
        else {
          continue;
        };

        self.tops.push(top);
      }

      let top = self.tops[index];

      if self
        .taffy
        .style(top)
        .is_ok_and(|held| *held != viewport_style)
      {
        let _ = self.taffy.set_style(top, viewport_style.clone());
      }

      if self.taffy.children(top).is_ok_and(|held| held != children) {
        let _ = self.taffy.set_children(top, &children);
      }

      if self.compute_at(top, available).is_ok() {
        self.collect(top, &mut frames);
      }
    }

    for top in self.tops.drain(roots.len().min(self.tops.len())..) {
      let _ = self.taffy.remove(top);
    }

    let stale: Vec<NodeId> = self
      .nodes
      .iter()
      .map(|(node, _)| node)
      .filter(|node| !walk.reached.contains(node))
      .collect();

    for node in stale {
      self.remove(node);
    }

    frames
  }

  /// Lays out the taffy tree under `top` in the space `available`, each leaf with a
  /// context measured by it.
  fn compute_at(
    &mut self,
    top: taffy::NodeId,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Result<(), taffy::TaffyError> {
    self
      .taffy
      .compute_layout_with_measure(top, available, |input, _, context, _| {
        let known = input.known_dimensions;
        let size = context.map_or(
          Size::new(
            known.width.unwrap_or_default(),
            known.height.unwrap_or_default(),
          ),
          |context| context.measure(known, input.available_space),
        );

        LayoutOutput::from_outer_size(taffy::geometry::Size {
          width: size.width,
          height: size.height,
        })
      })
  }

  /// Brings `node`, whose part is `its`, in line with the scene with everything laid out
  /// beneath it, returning its taffy node; `None` where it is left out.
  fn sync<P: Platform, F: FnMut(NodeId) -> Part<C>>(
    &mut self,
    node: NodeId,
    its: Part<C>,
    walk: &mut Walk<F, C>,
  ) -> Option<taffy::NodeId>
  where
    C: PartialEq,
  {
    let (style, context, children, scroll) = match its {
      Part::Box(style, scroll) | Part::Overlay(style, scroll) => {
        let children = self.sync_children::<P, _>(node, walk);

        (style, None, children, scroll)
      }
      Part::Leaf(style, mut context) => {
        for inline in context.boxes() {
          let its = (walk.part)(inline);
          let Some(laid_out) = self.sync::<P, _>(inline, its, walk)
          else {
            continue;
          };
          let max_content = taffy::geometry::Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
          };

          if self.compute_at(laid_out, max_content).is_ok()
            && let Ok(layout) = self.taffy.layout(laid_out)
          {
            context.size_box(inline, Size::new(layout.size.width, layout.size.height));
          }
        }

        (style, Some(context), Vec::new(), Point::new(0., 0.))
      }
      Part::Transparent | Part::Absent => return None,
    };

    self.insert(node);
    self.set_style(node, style);
    self.set_context(node, context);

    let slot = self.nodes.get_mut(node)?;

    slot.scroll = scroll;

    if slot.children != children && self.taffy.set_children(slot.taffy, &children).is_ok() {
      slot.children = children;
    }

    walk.reached.insert(node);

    Some(slot.taffy)
  }

  /// Brings the children `node` places in line with the scene, returning their taffy
  /// nodes in order: a transparent one's own in its place, overlays set aside for the
  /// root.
  fn sync_children<P: Platform, F: FnMut(NodeId) -> Part<C>>(
    &mut self,
    node: NodeId,
    walk: &mut Walk<F, C>,
  ) -> Vec<taffy::NodeId>
  where
    C: PartialEq,
  {
    let placed: Vec<NodeId> = Scene::<P>::read(|scene| scene.placed_children(node).collect());
    let mut children = Vec::new();

    for child in placed {
      match (walk.part)(child) {
        Part::Transparent => children.extend(self.sync_children::<P, _>(child, walk)),
        its @ Part::Overlay(..) => walk.overlays.push_back((child, its)),
        Part::Absent => {}
        its => children.extend(self.sync::<P, _>(child, its, walk)),
      }
    }

    children
  }

  /// Records the rectangle and the clip of every node laid out at or beneath `top`, in
  /// the order they paint: each before what it places and its children, the children
  /// in order, and a box a leaf places at the origin it places it at.
  fn collect(&self, top: taffy::NodeId, frames: &mut Frames) {
    let by_taffy: FxHashMap<taffy::NodeId, (NodeId, Point<f32>)> = self
      .nodes
      .iter()
      .map(|(node, slot)| (slot.taffy, (node, slot.scroll)))
      .collect();
    let mut stack = vec![(top, Point::new(0., 0.), Bounds::UNBOUNDED, true)];

    while let Some((current, content, clip, placed)) = stack.pop() {
      let (Ok(layout), Ok(style)) = (self.taffy.layout(current), self.taffy.style(current))
      else {
        continue;
      };
      let origin = if placed {
        content
      }
      else {
        Point::new(content.x + layout.location.x, content.y + layout.location.y)
      };
      let size = Size::new(layout.size.width, layout.size.height);
      let mut scroll = Point::new(0., 0.);
      let mut own = None;

      if let Some((node, offsets)) = by_taffy.get(&current) {
        frames.add(*node, Rect { origin, size }, clip);
        scroll = *offsets;
        own = Some(*node);
      }

      let mut inner = clip;

      if style.overflow.x != Overflow::Visible {
        inner.min.x = inner.min.x.max(origin.x + layout.border.left);
        inner.max.x = inner
          .max
          .x
          .min(origin.x + layout.size.width - layout.border.right);
      }

      if style.overflow.y != Overflow::Visible {
        inner.min.y = inner.min.y.max(origin.y + layout.border.top);
        inner.max.y = inner
          .max
          .y
          .min(origin.y + layout.size.height - layout.border.bottom);
      }

      let content = Point::new(origin.x - scroll.x, origin.y - scroll.y);

      if let Some(context) = self.taffy.get_node_context(current) {
        let start = layout.border.left + layout.padding.left;
        let top = layout.border.top + layout.padding.top;
        let inside = Rect {
          origin: Point::new(content.x + start, content.y + top),
          size: Size::new(
            size.width - start - layout.border.right - layout.padding.right,
            size.height - top - layout.border.bottom - layout.padding.bottom,
          ),
        };

        let mut boxes = Vec::new();

        for (node, rect) in context.place(inside) {
          match self.nodes.get(node) {
            _ if Some(node) == own => {}
            Some(inline) => boxes.push((inline.taffy, rect.origin, inner, true)),
            None => frames.add(node, rect, inner),
          }
        }

        stack.extend(boxes.into_iter().rev());
      }

      for child in self
        .taffy
        .children(current)
        .unwrap_or_default()
        .into_iter()
        .rev()
      {
        stack.push((child, content, inner, false));
      }
    }
  }
}
