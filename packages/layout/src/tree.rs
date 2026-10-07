//! A layout tree keyed by the scene's nodes: retained across layouts, each node's style,
//! context and children written where they changed, or built afresh from the scene for
//! one layout, each node's part as its platform classifies it.

use rustc_hash::FxHashMap;
use skene_foundation::{NodeId, Platform, Point, Rect, Scene, Size};
use taffy::{
  TaffyTree,
  prelude::{AvailableSpace, length},
  style::{Display, FlexDirection, Style},
  tree::{Layout, LayoutOutput},
};

/// What a node is to the layout, with the platform's leaf context `C` a measure reads.
#[derive(Debug)]
pub enum Part<C> {
  /// A box laid out with its own taffy style, its children laid out inside it.
  Box(Style),
  /// A leaf laid out with its own taffy style, measured from its context.
  Leaf(Style, C),
  /// An overlay: laid out with its own taffy style out of every flow, under the root.
  Overlay(Style),
  /// A node whose children lay out in its place, as an anchor's or a scope's do.
  Transparent,
  /// A node left out of the layout with everything beneath it.
  Absent,
}

/// Each node's rectangle from one layout, in the viewport's coordinates, and the order
/// the nodes paint in.
#[derive(Debug, Default, Clone)]
pub struct Frames {
  rects: FxHashMap<NodeId, Rect<f32>>,
  painted: Vec<NodeId>,
}

impl Frames {
  /// The rectangle `node` covers; `None` for a node the layout left out or never laid
  /// out on its own.
  #[must_use]
  pub fn rect(&self, node: NodeId) -> Option<Rect<f32>> {
    self.rects.get(&node).copied()
  }

  /// The topmost node laid out at `point`: the last to paint whose rectangle holds it,
  /// a child above its parent, a later sibling above an earlier one and an overlay
  /// above the flow, clipping aside.
  #[must_use]
  pub fn hit(&self, point: Point<f32>) -> Option<NodeId> {
    self.painted.iter().rev().copied().find(|node| {
      self.rects.get(node).is_some_and(|rect| {
        point.x >= rect.origin.x
          && point.y >= rect.origin.y
          && point.x < rect.origin.x + rect.size.width
          && point.y < rect.origin.y + rect.size.height
      })
    })
  }

  /// Adds every rectangle of `other`, painting above what this holds.
  pub fn extend(&mut self, other: Self) {
    self.rects.extend(other.rects);
    self.painted.extend(other.painted);
  }

  /// Every node laid out, with its rectangle, in the order they paint.
  pub fn iter(&self) -> impl Iterator<Item = (NodeId, Rect<f32>)> + '_ {
    self
      .painted
      .iter()
      .filter_map(|node| Some((*node, *self.rects.get(node)?)))
  }
}

/// A layout tree of the scene's nodes, its leaves measured from contexts `C`.
///
/// A node takes part once inserted, as a leaf of the default style with no context
/// and no children, until it is removed. Writing a node's style, context or children
/// marks it for the next [`compute`](Self::compute) only where the value differs from
/// the one it holds.
#[must_use]
pub struct Tree<C> {
  taffy: TaffyTree<C>,
  nodes: FxHashMap<NodeId, Slot>,
}

/// A node's place in the taffy tree: its taffy node and the taffy children last set.
struct Slot {
  taffy: taffy::NodeId,
  children: Vec<taffy::NodeId>,
}

impl<C> std::fmt::Debug for Tree<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Tree")
      .field("nodes", &self.nodes.len())
      .finish_non_exhaustive()
  }
}

impl<C> Tree<C> {
  /// An empty tree, each layout rounded to whole units where `rounding` says so.
  pub fn new(rounding: bool) -> Self {
    let mut taffy = TaffyTree::new();

    if !rounding {
      taffy.disable_rounding();
    }

    Self {
      taffy,
      nodes: FxHashMap::default(),
    }
  }

  /// Adds `node` as a leaf of the default style with no context and no children; a
  /// node already in the tree stays as it is.
  ///
  /// # Panics
  ///
  /// Panics where taffy cannot allocate the node.
  pub fn insert(&mut self, node: NodeId) {
    if self.nodes.contains_key(&node) {
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
      },
    );
  }

  /// Takes `node` out of the tree, and out of its parent's children.
  pub fn remove(&mut self, node: NodeId) {
    if let Some(slot) = self.nodes.remove(&node) {
      let _ = self.taffy.remove(slot.taffy);
    }
  }

  /// Whether `node` takes part in the layout.
  #[must_use]
  pub fn contains(&self, node: NodeId) -> bool {
    self.nodes.contains_key(&node)
  }

  /// Gives `node` the taffy style `style`; `true` where it differs from the one it
  /// held, `false` where it is the same or `node` is outside the tree.
  pub fn set_style(&mut self, node: NodeId, style: Style) -> bool {
    let Some(slot) = self.nodes.get(&node)
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
    let Some(slot) = self.nodes.get(&node)
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
    let slot = self.nodes.get(&node)?;

    self.taffy.get_node_context_mut(slot.taffy)
  }

  /// Marks `node` for the next [`compute`](Self::compute) to measure and lay out
  /// again, with every ancestor.
  pub fn mark_dirty(&mut self, node: NodeId) {
    if let Some(slot) = self.nodes.get(&node) {
      let _ = self.taffy.mark_dirty(slot.taffy);
    }
  }

  /// Gives `node` the children `children`, in order, leaving out each outside the tree;
  /// `true` where they differ from the ones it held.
  pub fn set_children(&mut self, node: NodeId, children: impl IntoIterator<Item = NodeId>) -> bool {
    let children: Vec<taffy::NodeId> = children
      .into_iter()
      .filter_map(|child| self.nodes.get(&child).map(|slot| slot.taffy))
      .collect();
    let Some(slot) = self.nodes.get_mut(&node)
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
  /// measured by `measure` from its known dimensions, the space available and its
  /// context, and each without one sized to its known dimensions or nothing.
  pub fn compute(
    &mut self,
    root: NodeId,
    available: taffy::geometry::Size<AvailableSpace>,
    measure: impl FnMut(
      taffy::geometry::Size<Option<f32>>,
      taffy::geometry::Size<AvailableSpace>,
      &C,
    ) -> Size<f32>,
  ) {
    if let Some(slot) = self.nodes.get(&root) {
      let _ = self.compute_at(slot.taffy, available, measure);
    }
  }

  /// `node`'s layout in its parent, as the last [`compute`](Self::compute) left it,
  /// rounded where the tree rounds; `None` for a node outside the tree.
  #[must_use]
  pub fn layout(&self, node: NodeId) -> Option<&Layout> {
    self.taffy.layout(self.nodes.get(&node)?.taffy).ok()
  }

  /// `node`'s layout in its parent, as the last [`compute`](Self::compute) left it
  /// before any rounding; `None` for a node outside the tree.
  #[must_use]
  pub fn unrounded_layout(&self, node: NodeId) -> Option<&Layout> {
    Some(self.taffy.unrounded_layout(self.nodes.get(&node)?.taffy))
  }

  /// The rectangle of every node laid out at or beneath `root` in the last
  /// [`compute`](Self::compute), from `root`'s origin, and the order they paint in.
  #[must_use]
  pub fn frames(&self, root: NodeId) -> Frames {
    let mut frames = Frames::default();

    if let Some(slot) = self.nodes.get(&root) {
      self.collect(slot.taffy, &mut frames);
    }

    frames
  }

  /// Lays out the scene of the platform `P` under `root` at `viewport` on a tree built
  /// for the one layout, each node's part as `part` answers, each leaf measured by
  /// `measure` from its known dimensions, the space available and its context; the
  /// layout rounded to whole units where `rounding` says so.
  ///
  /// The root lays out in the viewport as a block's child does, stretched across it,
  /// and the open overlays beneath it are hoisted beside it, out of every flow.
  pub fn lay_out<P: Platform>(
    root: NodeId,
    viewport: Size<f32>,
    rounding: bool,
    mut part: impl FnMut(NodeId) -> Part<C>,
    measure: impl FnMut(
      taffy::geometry::Size<Option<f32>>,
      taffy::geometry::Size<AvailableSpace>,
      &C,
    ) -> Size<f32>,
  ) -> Frames {
    let mut tree = Self::new(rounding);
    let mut overlays = Vec::new();
    let mut children: Vec<taffy::NodeId> = match part(root) {
      Part::Transparent => tree.children_of::<P>(root, &mut part, &mut overlays),
      _ => tree
        .add::<P>(root, &mut part, &mut overlays)
        .into_iter()
        .collect(),
    };
    let mut hoisted = 0;

    while hoisted < overlays.len() {
      let overlay = overlays[hoisted];

      hoisted += 1;
      children.extend(tree.add::<P>(overlay, &mut part, &mut overlays));
    }

    let viewport_style = Style {
      display: Display::Flex,
      flex_direction: FlexDirection::Column,
      size: taffy::geometry::Size {
        width: length(viewport.width),
        height: length(viewport.height),
      },
      ..Style::default()
    };
    let Ok(top) = tree.taffy.new_with_children(viewport_style, &children)
    else {
      return Frames::default();
    };
    let mut frames = Frames::default();

    let available = taffy::geometry::Size {
      width: AvailableSpace::Definite(viewport.width),
      height: AvailableSpace::Definite(viewport.height),
    };

    if tree.compute_at(top, available, measure).is_ok() {
      tree.collect(top, &mut frames);
    }

    frames
  }

  /// Lays out the taffy tree under `top` in the space `available`, measuring each leaf
  /// with a context by `measure`.
  fn compute_at(
    &mut self,
    top: taffy::NodeId,
    available: taffy::geometry::Size<AvailableSpace>,
    mut measure: impl FnMut(
      taffy::geometry::Size<Option<f32>>,
      taffy::geometry::Size<AvailableSpace>,
      &C,
    ) -> Size<f32>,
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
          |context| measure(known, input.available_space, context),
        );

        LayoutOutput::from_outer_size(taffy::geometry::Size {
          width: size.width,
          height: size.height,
        })
      })
  }

  /// Adds `node` with everything laid out beneath it, returning its taffy node; `None`
  /// where it is left out.
  fn add<P: Platform>(
    &mut self,
    node: NodeId,
    part: &mut impl FnMut(NodeId) -> Part<C>,
    overlays: &mut Vec<NodeId>,
  ) -> Option<taffy::NodeId> {
    let (taffy, children) = match part(node) {
      Part::Box(style) | Part::Overlay(style) => {
        let children = self.children_of::<P>(node, part, overlays);

        (
          self.taffy.new_with_children(style, &children).ok()?,
          children,
        )
      }
      Part::Leaf(style, context) => (
        self.taffy.new_leaf_with_context(style, context).ok()?,
        Vec::new(),
      ),
      Part::Transparent | Part::Absent => return None,
    };

    self.nodes.insert(node, Slot { taffy, children });

    Some(taffy)
  }

  /// The taffy nodes laid out as `node`'s children: its placed children, a transparent
  /// one's own in its place, overlays set aside for the root.
  fn children_of<P: Platform>(
    &mut self,
    node: NodeId,
    part: &mut impl FnMut(NodeId) -> Part<C>,
    overlays: &mut Vec<NodeId>,
  ) -> Vec<taffy::NodeId> {
    let placed: Vec<NodeId> = Scene::<P>::read(|scene| scene.placed_children(node).collect());
    let mut children = Vec::new();

    for child in placed {
      match part(child) {
        Part::Transparent => children.extend(self.children_of::<P>(child, part, overlays)),
        Part::Overlay(_) => overlays.push(child),
        Part::Absent => {}
        Part::Box(_) | Part::Leaf(..) => children.extend(self.add::<P>(child, part, overlays)),
      }
    }

    children
  }

  /// Records the rectangle of every node laid out at or beneath `top`, in the order
  /// they paint: each before its children, the children in order.
  fn collect(&self, top: taffy::NodeId, frames: &mut Frames) {
    let by_taffy: FxHashMap<taffy::NodeId, NodeId> = self
      .nodes
      .iter()
      .map(|(node, slot)| (slot.taffy, *node))
      .collect();
    let mut stack = vec![(top, Point::new(0., 0.), true)];

    while let Some((current, parent, top)) = stack.pop() {
      let Ok(layout) = self.taffy.layout(current)
      else {
        continue;
      };
      let origin = if top {
        Point::new(0., 0.)
      }
      else {
        Point::new(parent.x + layout.location.x, parent.y + layout.location.y)
      };

      if let Some(node) = by_taffy.get(&current) {
        frames.rects.insert(
          *node,
          Rect {
            origin,
            size: Size::new(layout.size.width, layout.size.height),
          },
        );
        frames.painted.push(*node);
      }

      for child in self
        .taffy
        .children(current)
        .unwrap_or_default()
        .into_iter()
        .rev()
      {
        stack.push((child, origin, false));
      }
    }
  }
}
