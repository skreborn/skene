//! A layout tree keyed by the scene's nodes, retained across layouts: each node's style,
//! context and children written where they changed, laid out by one taffy engine per
//! container.

mod engine;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod oracle;
#[cfg(test)]
mod tests;

use skene_foundation::{NodeId, Rect, Size};
use taffy::{
  compute_root_layout,
  prelude::AvailableSpace,
  style::{Position, Style},
  tree::Layout,
};

use self::engine::{Engine, Nodes, Slot, key};

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
/// Each container, a node inserted as one, is laid out by a taffy engine of its own over
/// its members: itself, and the nodes beneath it up to the next containers, which their
/// own engines lay out from the whole of taffy's input. A layout is that of one taffy tree
/// of every node, whichever nodes are containers, rounded once over the whole hierarchy.
///
/// Every node contains the boxes positioned absolutely inside it: a style that leaves
/// its position static is laid out relative, so a pinned box or an overlay is placed
/// against its parent.
#[must_use]
pub struct Tree<C> {
  nodes: Nodes<C>,
  rounding: bool,
}

impl<C> std::fmt::Debug for Tree<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Tree")
      .field("nodes", &self.nodes.map().values().count())
      .field("rounding", &self.rounding)
      .finish_non_exhaustive()
  }
}

impl<C: Leaf> Tree<C> {
  /// An empty tree, each layout rounded to whole units where `rounding` says so.
  pub const fn new(rounding: bool) -> Self {
    Self {
      nodes: Nodes::new(),
      rounding,
    }
  }

  /// Adds `node` as a leaf of the default style with no context and no children: a
  /// container laid out by an engine of its own where `decorated` says so, and a member
  /// its nearest container's engine lays out otherwise; a node already in the tree stays
  /// as it is.
  pub fn insert(&mut self, node: NodeId, decorated: bool) {
    if self.nodes.map().contains(node) {
      return;
    }

    let _ = self.nodes.map_mut().insert(
      node,
      Slot::new(Self::containing(Style::default()), decorated),
    );
  }

  /// Takes `node` out of the tree, and out of its parent's children, its children left
  /// without a parent.
  pub fn remove(&mut self, node: NodeId) {
    let Some(slot) = self.nodes.map_mut().remove(node)
    else {
      return;
    };
    let id = key(node);

    if let Some(parent) = slot.parent {
      if let Some(held) = self.nodes.get_mut(parent) {
        held.children.retain(|child| *child != id);
      }

      self.nodes.mark_dirty(parent);
    }

    for child in slot.children {
      if let Some(held) = self.nodes.get_mut(child) {
        held.parent = None;
      }
    }
  }

  /// Whether `node` takes part in the layout.
  #[must_use]
  pub fn contains(&self, node: NodeId) -> bool {
    self.nodes.map().contains(node)
  }

  /// Whether `node` is a container, laid out by an engine of its own: inserted as
  /// decorated, and still in the tree.
  #[must_use]
  pub fn is_container(&self, node: NodeId) -> bool {
    self
      .nodes
      .map()
      .get(node)
      .is_some_and(|slot| slot.decorated)
  }

  /// Gives `node` the taffy style `style`; `true` where it differs from the one it
  /// held, `false` where it is the same or `node` is outside the tree.
  pub fn set_style(&mut self, node: NodeId, style: Style) -> bool {
    let style = Self::containing(style);
    let Some(slot) = self.nodes.map_mut().get_mut(node)
    else {
      return false;
    };

    if slot.style == style {
      return false;
    }

    slot.style = style;
    self.nodes.mark_dirty(key(node));

    true
  }

  /// Gives `node` the context its measure reads, or none; `true` where it differs from
  /// the one it held, `false` where it is the same, where it differs in paint alone, which
  /// the held one takes in place ([`Leaf::repaint_as`]) without the node measured again,
  /// or where `node` is outside the tree.
  pub fn set_context(&mut self, node: NodeId, context: Option<C>) -> bool
  where
    C: PartialEq,
  {
    let Some(slot) = self.nodes.map_mut().get_mut(node)
    else {
      return false;
    };

    if slot.context == context {
      return false;
    }

    if let (Some(held), Some(next)) = (&mut slot.context, &context)
      && held.repaint_as(next)
    {
      return false;
    }

    slot.context = context;
    self.nodes.mark_dirty(key(node));

    true
  }

  /// Whether `node` is marked for the next [`compute`](Self::compute) to measure and lay
  /// out again.
  #[cfg(test)]
  pub(crate) fn is_dirty(&self, node: NodeId) -> bool {
    self
      .nodes
      .map()
      .get(node)
      .is_some_and(|slot| slot.cache.is_empty())
  }

  /// The context `node`'s measure reads; `None` for a node outside the tree or without
  /// one.
  #[must_use]
  pub fn context(&self, node: NodeId) -> Option<&C> {
    self.nodes.map().get(node)?.context.as_ref()
  }

  /// The context `node`'s measure reads, to change in place; a change to what it
  /// measures to takes [`mark_dirty`](Self::mark_dirty) as well.
  pub fn context_mut(&mut self, node: NodeId) -> Option<&mut C> {
    self.nodes.map_mut().get_mut(node)?.context.as_mut()
  }

  /// Marks `node` for the next [`compute`](Self::compute) to measure and lay out
  /// again, with every ancestor.
  pub fn mark_dirty(&mut self, node: NodeId) {
    self.nodes.mark_dirty(key(node));
  }

  /// Gives `node` the children `children`, in order, leaving out each outside the tree,
  /// each taken from the parent it had; `true` where they differ from the ones it holds,
  /// which a child set elsewhere or removed since has left.
  pub fn set_children(&mut self, node: NodeId, children: impl IntoIterator<Item = NodeId>) -> bool {
    let children: Vec<taffy::NodeId> = children
      .into_iter()
      .filter(|child| self.nodes.map().contains(*child))
      .map(key)
      .collect();
    let id = key(node);
    let Some(slot) = self.nodes.get_mut(id)
    else {
      return false;
    };

    if slot.children == children {
      return false;
    }

    for child in std::mem::take(&mut slot.children) {
      if let Some(held) = self.nodes.get_mut(child) {
        held.parent = None;
      }
    }

    for &child in &children {
      if let Some(previous) = self.nodes.get(child).and_then(|held| held.parent) {
        if let Some(parent) = self.nodes.get_mut(previous)
          && let Some(at) = parent.children.iter().position(|held| *held == child)
        {
          parent.children.remove(at);
        }

        self.nodes.mark_dirty(previous);
      }

      if let Some(held) = self.nodes.get_mut(child) {
        held.parent = Some(id);
      }
    }

    if let Some(slot) = self.nodes.get_mut(id) {
      slot.children = children;
    }

    self.nodes.mark_dirty(id);

    true
  }

  /// Lays out the tree under `root` in the space `available`, as its root whether or
  /// not it has a parent, each leaf with a context measured by it and each without one
  /// sized to its known dimensions or nothing, and rounds it from `root` where the tree
  /// rounds.
  pub fn compute(&mut self, root: NodeId, available: taffy::geometry::Size<AvailableSpace>) {
    if !self.nodes.map().contains(root) {
      return;
    }

    let id = key(root);

    compute_root_layout(&mut Engine::new(&mut self.nodes, id), id, available);

    if self.rounding {
      self.nodes.round(id, 0., 0.);
    }
  }

  /// `node`'s layout in its parent, as the last [`compute`](Self::compute) left it,
  /// rounded where the tree rounds; `None` for a node outside the tree.
  #[must_use]
  pub fn layout(&self, node: NodeId) -> Option<&Layout> {
    let slot = self.nodes.map().get(node)?;

    Some(if self.rounding {
      &slot.rounded
    }
    else {
      &slot.unrounded
    })
  }

  /// `node`'s layout in its parent, as the last [`compute`](Self::compute) left it
  /// before any rounding; `None` for a node outside the tree.
  #[must_use]
  pub fn unrounded_layout(&self, node: NodeId) -> Option<&Layout> {
    Some(&self.nodes.map().get(node)?.unrounded)
  }

  /// `style`, laid out relative where it leaves its position static, so its node
  /// contains the boxes positioned absolutely inside it.
  const fn containing(mut style: Style) -> Style {
    if matches!(style.position, Position::Static) {
      style.position = Position::Relative;
    }

    style
  }
}
