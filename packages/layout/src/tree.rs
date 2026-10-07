//! A tree laid out from the scene: each node's part in the layout as its platform
//! classifies it, laid out on a taffy tree built for the one computation.

use rustc_hash::FxHashMap;
use skene_foundation::{NodeId, Platform, Point, Rect, Scene, Size};
use taffy::{
  TaffyTree,
  prelude::{AvailableSpace, length},
  style::{Display, FlexDirection, Style},
  tree::LayoutOutput,
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

/// Each node's rectangle from one layout, in the viewport's coordinates.
#[derive(Debug, Default, Clone)]
pub struct Frames(FxHashMap<NodeId, Rect<f32>>);

impl Frames {
  /// The rectangle `node` covers; `None` for a node the layout left out or never laid
  /// out on its own.
  #[must_use]
  pub fn rect(&self, node: NodeId) -> Option<Rect<f32>> {
    self.0.get(&node).copied()
  }

  /// Adds every rectangle of `other`, replacing what this holds for its nodes.
  pub fn extend(&mut self, other: Self) {
    self.0.extend(other.0);
  }

  /// Every node laid out, with its rectangle, in no order.
  pub fn iter(&self) -> impl Iterator<Item = (NodeId, Rect<f32>)> + '_ {
    self.0.iter().map(|(node, rect)| (*node, *rect))
  }
}

/// A layout of the scene of the platform `P`, its leaves measured from contexts `C`.
pub struct Tree<P: Platform, C> {
  taffy: TaffyTree<C>,
  nodes: FxHashMap<NodeId, taffy::NodeId>,
  overlays: Vec<NodeId>,
  platform: std::marker::PhantomData<P>,
}

impl<P: Platform, C> std::fmt::Debug for Tree<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Tree")
      .field("nodes", &self.nodes.len())
      .finish_non_exhaustive()
  }
}

impl<P: Platform, C> Tree<P, C> {
  /// Lays out the tree under `root` at `viewport`, each node's part as `part` answers,
  /// each leaf measured by `measure` from its known dimensions, the space available and
  /// its context; the layout rounded to whole units where `rounding` says so.
  ///
  /// The root lays out in the viewport as a block's child does, stretched across it,
  /// and the open overlays beneath it are hoisted beside it, out of every flow.
  pub fn lay_out(
    root: NodeId,
    viewport: Size<f32>,
    rounding: bool,
    mut part: impl FnMut(NodeId) -> Part<C>,
    mut measure: impl FnMut(
      taffy::geometry::Size<Option<f32>>,
      taffy::geometry::Size<AvailableSpace>,
      &C,
    ) -> Size<f32>,
  ) -> Frames {
    let mut tree = Self {
      taffy: TaffyTree::new(),
      nodes: FxHashMap::default(),
      overlays: Vec::new(),
      platform: std::marker::PhantomData,
    };

    if !rounding {
      tree.taffy.disable_rounding();
    }

    let mut children: Vec<taffy::NodeId> = match part(root) {
      Part::Transparent => tree.children_of(root, &mut part),
      _ => tree.add(root, &mut part).into_iter().collect(),
    };
    let mut hoisted = 0;

    while hoisted < tree.overlays.len() {
      let overlay = tree.overlays[hoisted];

      hoisted += 1;
      children.extend(tree.add(overlay, &mut part));
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

    let available = taffy::geometry::Size {
      width: AvailableSpace::Definite(viewport.width),
      height: AvailableSpace::Definite(viewport.height),
    };
    let computed =
      tree
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
        });

    if computed.is_err() {
      return Frames::default();
    }

    let mut frames = FxHashMap::default();

    tree.collect(top, Point::new(0., 0.), &mut frames);

    Frames(frames)
  }

  /// Adds `node` with everything laid out beneath it, returning its taffy node; `None`
  /// where it is left out.
  fn add(
    &mut self,
    node: NodeId,
    part: &mut impl FnMut(NodeId) -> Part<C>,
  ) -> Option<taffy::NodeId> {
    let added = match part(node) {
      Part::Box(style) | Part::Overlay(style) => {
        let children = self.children_of(node, part);

        self.taffy.new_with_children(style, &children).ok()?
      }
      Part::Leaf(style, context) => self.taffy.new_leaf_with_context(style, context).ok()?,
      Part::Transparent | Part::Absent => return None,
    };

    self.nodes.insert(node, added);

    Some(added)
  }

  /// The taffy nodes laid out as `node`'s children: its placed children, a transparent
  /// one's own in its place, overlays set aside for the root.
  fn children_of(
    &mut self,
    node: NodeId,
    part: &mut impl FnMut(NodeId) -> Part<C>,
  ) -> Vec<taffy::NodeId> {
    let placed: Vec<NodeId> = Scene::<P>::read(|scene| scene.placed_children(node).collect());
    let mut children = Vec::new();

    for child in placed {
      match part(child) {
        Part::Transparent => children.extend(self.children_of(child, part)),
        Part::Overlay(_) => self.overlays.push(child),
        Part::Absent => {}
        Part::Box(_) | Part::Leaf(..) => children.extend(self.add(child, part)),
      }
    }

    children
  }

  /// Records the rectangle of every node laid out at or beneath `top`, whose parent's
  /// origin is at `origin`.
  fn collect(
    &self,
    top: taffy::NodeId,
    origin: Point<f32>,
    frames: &mut FxHashMap<NodeId, Rect<f32>>,
  ) {
    let by_taffy: FxHashMap<taffy::NodeId, NodeId> = self
      .nodes
      .iter()
      .map(|(node, taffy)| (*taffy, *node))
      .collect();
    let mut stack = vec![(top, origin)];

    while let Some((current, parent)) = stack.pop() {
      let Ok(layout) = self.taffy.layout(current)
      else {
        continue;
      };
      let origin = Point::new(parent.x + layout.location.x, parent.y + layout.location.y);

      if let Some(node) = by_taffy.get(&current) {
        frames.insert(
          *node,
          Rect {
            origin,
            size: Size::new(layout.size.width, layout.size.height),
          },
        );
      }

      for child in self.taffy.children(current).unwrap_or_default() {
        stack.push((child, origin));
      }
    }
  }
}
