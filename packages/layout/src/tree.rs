//! A layout tree keyed by the scene's nodes, retained across layouts: each node's style,
//! context and children written where they changed.

use skene_foundation::{NodeId, NodeMap, Rect, Size};
use taffy::{
  TaffyTree,
  TraversePartialTree,
  prelude::AvailableSpace,
  style::{Position, Style},
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

  /// The nodes flowing inside the leaf as boxes, which its host lays out at their
  /// content's size and gives it through [`size_box`](Self::size_box) before it measures;
  /// none by default.
  fn boxes(&self) -> Vec<NodeId> {
    Vec::new()
  }

  /// Gives the box `node` flowing inside the leaf the size it laid out at.
  fn size_box(&mut self, node: NodeId, size: Size<f32>) {
    let _ = (node, size);
  }

  /// Takes `next`'s paint in place, where `next` differs from this leaf in paint alone:
  /// `true` where it did, every measurement and layout kept, `false` where `next` measures
  /// or places differently or the leaf keeps no paint of its own to change; none by
  /// default.
  fn repaint_as(&mut self, next: &Self) -> bool {
    let _ = next;

    false
  }
}

/// A layout tree of the scene's nodes, its leaves measured from their contexts `C`.
///
/// A node takes part once inserted, as a leaf of the default style with no context
/// and no children, until it is removed. Writing a node's style, context or children
/// marks it for the next layout only where the value differs from the one it holds.
///
/// Every node contains the boxes positioned absolutely inside it: a style that leaves
/// its position static is laid out relative, so a pinned box or an overlay is placed
/// against its parent.
#[must_use]
pub struct Tree<C> {
  taffy: TaffyTree<C>,
  nodes: NodeMap<taffy::NodeId>,
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
      .new_leaf(Self::containing(Style::default()))
      .expect("allocate a layout node");

    self.nodes.insert(node, taffy);
  }

  /// Takes `node` out of the tree, and out of its parent's children.
  pub fn remove(&mut self, node: NodeId) {
    if let Some(taffy) = self.nodes.remove(node) {
      let _ = self.taffy.remove(taffy);
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
    let Some(&taffy) = self.nodes.get(node)
    else {
      return false;
    };
    let style = Self::containing(style);

    if self.taffy.style(taffy).is_ok_and(|held| *held == style) {
      return false;
    }

    self.taffy.set_style(taffy, style).is_ok()
  }

  /// Gives `node` the context its measure reads, or none; `true` where it differs from
  /// the one it held, `false` where it is the same, where it differs in paint alone, which
  /// the held one takes in place ([`Leaf::repaint_as`]) without the node measured again,
  /// or where `node` is outside the tree.
  pub fn set_context(&mut self, node: NodeId, context: Option<C>) -> bool
  where
    C: PartialEq,
  {
    let Some(&taffy) = self.nodes.get(node)
    else {
      return false;
    };

    if self.taffy.get_node_context(taffy) == context.as_ref() {
      return false;
    }

    if let (Some(held), Some(next)) = (self.taffy.get_node_context_mut(taffy), &context)
      && held.repaint_as(next)
    {
      return false;
    }

    self.taffy.set_node_context(taffy, context).is_ok()
  }

  /// Whether `node` is marked for the next [`compute`](Self::compute) to measure and lay
  /// out again.
  #[cfg(test)]
  pub(crate) fn is_dirty(&self, node: NodeId) -> bool {
    self
      .nodes
      .get(node)
      .is_some_and(|taffy| self.taffy.dirty(*taffy).unwrap_or(true))
  }

  /// The context `node`'s measure reads; `None` for a node outside the tree or without
  /// one.
  #[must_use]
  pub fn context(&self, node: NodeId) -> Option<&C> {
    self.taffy.get_node_context(*self.nodes.get(node)?)
  }

  /// The context `node`'s measure reads, to change in place; a change to what it
  /// measures to takes [`mark_dirty`](Self::mark_dirty) as well.
  pub fn context_mut(&mut self, node: NodeId) -> Option<&mut C> {
    self.taffy.get_node_context_mut(*self.nodes.get(node)?)
  }

  /// Marks `node` for the next [`compute`](Self::compute) to measure and lay out
  /// again, with every ancestor.
  pub fn mark_dirty(&mut self, node: NodeId) {
    if let Some(&taffy) = self.nodes.get(node) {
      let _ = self.taffy.mark_dirty(taffy);
    }
  }

  /// Gives `node` the children `children`, in order, leaving out each outside the tree,
  /// each taken from the parent it had; `true` where they differ from the ones it holds,
  /// which a child set elsewhere or removed since has left.
  pub fn set_children(&mut self, node: NodeId, children: impl IntoIterator<Item = NodeId>) -> bool {
    let children: Vec<taffy::NodeId> = children
      .into_iter()
      .filter_map(|child| self.nodes.get(child).copied())
      .collect();
    let Some(&taffy) = self.nodes.get(node)
    else {
      return false;
    };

    if self.taffy.child_ids(taffy).eq(children.iter().copied()) {
      return false;
    }

    self.taffy.set_children(taffy, &children).is_ok()
  }

  /// Lays out the tree under `root` in the space `available`, each leaf with a context
  /// measured by it and each without one sized to its known dimensions or nothing.
  pub fn compute(&mut self, root: NodeId, available: taffy::geometry::Size<AvailableSpace>) {
    if let Some(&taffy) = self.nodes.get(root) {
      let _ = self.compute_at(taffy, available);
    }
  }

  /// `node`'s layout in its parent, as the last [`compute`](Self::compute) left it,
  /// rounded where the tree rounds; `None` for a node outside the tree.
  #[must_use]
  pub fn layout(&self, node: NodeId) -> Option<&Layout> {
    self.taffy.layout(*self.nodes.get(node)?).ok()
  }

  /// `node`'s layout in its parent, as the last [`compute`](Self::compute) left it
  /// before any rounding; `None` for a node outside the tree.
  #[must_use]
  pub fn unrounded_layout(&self, node: NodeId) -> Option<&Layout> {
    Some(self.taffy.unrounded_layout(*self.nodes.get(node)?))
  }

  /// `style`, laid out relative where it leaves its position static, so its node
  /// contains the boxes positioned absolutely inside it.
  const fn containing(mut style: Style) -> Style {
    if matches!(style.position, Position::Static) {
      style.position = Position::Relative;
    }

    style
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
}
