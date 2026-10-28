//! The engine: taffy's algorithms run over one container's members, a container beneath
//! them laid out by an engine of its own.

use skene_foundation::{NodeId, NodeMap};
use taffy::{
  BlockContext,
  Cache,
  CacheTree,
  ClearState,
  DetailedGridInfo,
  DetailedLayoutInfo,
  LayoutBlockContainer,
  LayoutContainingBlock,
  LayoutFlexboxContainer,
  LayoutGridContainer,
  LayoutPartialTree,
  RunMode,
  TraversePartialTree,
  TraverseTree,
  compute_block_layout,
  compute_cached_layout,
  compute_flexbox_layout,
  compute_grid_layout,
  compute_hidden_layout,
  compute_oof_layout,
  style::{Display, Position, Style},
  tree::{Layout, LayoutInput, LayoutOutput},
};

use super::Leaf;

/// One node of the layout: what it is laid out from, and what its last layout left.
pub(super) struct Slot<C> {
  /// Its taffy style, laid out relative where it leaves its position static.
  pub(super) style: Style,
  /// The context its measure reads, where it has one.
  pub(super) context: Option<C>,
  /// Its children, in order.
  pub(super) children: Vec<taffy::NodeId>,
  /// Its parent, where it has one.
  pub(super) parent: Option<taffy::NodeId>,
  /// Whether an engine of its own lays it out, as the scene created it.
  pub(super) decorated: bool,
  /// What its layout answered each input it was given since it was last marked.
  pub(super) cache: Cache,
  /// Its layout in its parent, unrounded.
  pub(super) unrounded: Layout,
  /// Its layout in its parent, rounded to whole units.
  pub(super) rounded: Layout,
  /// The out-of-flow boxes it contains, as its last layout placed them; none once it is
  /// laid out as a leaf or hidden.
  pub(super) hoisted: Vec<taffy::NodeId>,
  /// What its last layout as a grid recorded of its tracks and items.
  pub(super) detailed: DetailedLayoutInfo,
}

impl<C> Slot<C> {
  /// A node of `style` with no context, no parent and no children, laid out by an engine
  /// of its own where `decorated` says so.
  pub(super) const fn new(style: Style, decorated: bool) -> Self {
    Self {
      style,
      context: None,
      children: Vec::new(),
      parent: None,
      decorated,
      cache: Cache::new(),
      unrounded: Layout::new(),
      rounded: Layout::new(),
      hoisted: Vec::new(),
      detailed: DetailedLayoutInfo::None,
    }
  }

  /// Whether the box is taken out of its parent's flow: positioned absolutely, and
  /// displayed.
  #[inline(always)]
  fn out_of_flow(&self) -> bool {
    self.style.position.is_out_of_flow() && self.style.display != Display::None
  }
}

/// The id taffy knows `node` by.
#[inline(always)]
pub(super) fn key(node: NodeId) -> taffy::NodeId {
  taffy::NodeId::new((u64::from(node.generation()) << 32) | u64::from(node.index()))
}

/// The node taffy's `id` names.
#[expect(
  clippy::cast_possible_truncation,
  reason = "an id holds the node's index in its low half and its generation in its high half"
)]
#[inline(always)]
pub(super) fn node(id: taffy::NodeId) -> NodeId {
  let raw = u64::from(id);

  NodeId::new(raw as u32, (raw >> 32) as u32)
}

/// Every node of the layout, by the scene's node.
pub(super) struct Nodes<C> {
  /// Each node's slot, by the scene's node.
  map: NodeMap<Slot<C>>,
  /// How many sizings (`RunMode::ComputeSize`) the layout under way is inside.
  measuring: u32,
}

impl<C> Nodes<C> {
  /// No nodes.
  pub(super) const fn new() -> Self {
    Self {
      map: NodeMap::new(),
      measuring: 0,
    }
  }

  /// The nodes, by the scene's node.
  pub(super) const fn map(&self) -> &NodeMap<Slot<C>> {
    &self.map
  }

  /// The nodes, mutably, by the scene's node.
  pub(super) const fn map_mut(&mut self) -> &mut NodeMap<Slot<C>> {
    &mut self.map
  }

  /// The node taffy's `id` names, where it is in the layout.
  #[inline(always)]
  pub(super) fn get(&self, id: taffy::NodeId) -> Option<&Slot<C>> {
    self.map.get(node(id))
  }

  /// The node taffy's `id` names, mutably, where it is in the layout.
  #[inline(always)]
  pub(super) fn get_mut(&mut self, id: taffy::NodeId) -> Option<&mut Slot<C>> {
    self.map.get_mut(node(id))
  }

  /// The node taffy's `id` names.
  ///
  /// # Panics
  ///
  /// Panics where it is not in the layout.
  #[inline(always)]
  fn slot(&self, id: taffy::NodeId) -> &Slot<C> {
    self.get(id).expect("a node of the layout")
  }

  /// The node taffy's `id` names, mutably.
  ///
  /// # Panics
  ///
  /// Panics where it is not in the layout.
  #[inline(always)]
  fn slot_mut(&mut self, id: taffy::NodeId) -> &mut Slot<C> {
    self.get_mut(id).expect("a node of the layout")
  }

  /// Marks `id` for its next layout to compute it again, with each ancestor up to the
  /// first one already marked.
  pub(super) fn mark_dirty(&mut self, id: taffy::NodeId) {
    let mut current = Some(id);

    while let Some(at) = current {
      let Some(slot) = self.get_mut(at)
      else {
        return;
      };

      if matches!(slot.cache.clear(), ClearState::AlreadyEmpty) {
        return;
      }

      current = slot.parent;
    }
  }

  /// Rounds `id`'s layout and every layout beneath it to whole units as taffy's
  /// `round_layout` does, from where its parent lies in the root at `x` and `y`: each
  /// edge rounded where it lies in the root, and an out-of-flow box through the hoisted
  /// list of the box containing it.
  pub(super) fn round(&mut self, id: taffy::NodeId, x: f32, y: f32) {
    let Some(slot) = self.get_mut(id)
    else {
      return;
    };
    let unrounded = slot.unrounded;
    let mut layout = unrounded;
    let left = x + unrounded.location.x;
    let top = y + unrounded.location.y;
    let right = left + unrounded.size.width;
    let bottom = top + unrounded.size.height;

    layout.location.x = round(left) - round(x);
    layout.location.y = round(top) - round(y);
    layout.size.width = round(right) - round(left);
    layout.size.height = round(bottom) - round(top);
    layout.scrollbar_size.width = round(unrounded.scrollbar_size.width);
    layout.scrollbar_size.height = round(unrounded.scrollbar_size.height);
    layout.border.left = round(left + unrounded.border.left) - round(left);
    layout.border.right = round(right) - round(right - unrounded.border.right);
    layout.border.top = round(top + unrounded.border.top) - round(top);
    layout.border.bottom = round(bottom) - round(bottom - unrounded.border.bottom);
    layout.padding.left = round(left + unrounded.padding.left) - round(left);
    layout.padding.right = round(right) - round(right - unrounded.padding.right);
    layout.padding.top = round(top + unrounded.padding.top) - round(top);
    layout.padding.bottom = round(bottom) - round(bottom - unrounded.padding.bottom);

    let overflow = unrounded.scrollable_overflow_rect;

    layout.scrollable_overflow_rect.left = round(left + overflow.left) - round(left);
    layout.scrollable_overflow_rect.right = round(left + overflow.right) - round(left);
    layout.scrollable_overflow_rect.top = round(top + overflow.top) - round(top);
    layout.scrollable_overflow_rect.bottom = round(top + overflow.bottom) - round(top);
    slot.rounded = layout;

    for index in 0..slot.children.len() {
      let child = self.slot(id).children[index];

      if !self.slot(child).out_of_flow() {
        self.round(child, left, top);
      }
    }

    for index in 0..self.slot(id).hoisted.len() {
      let child = self.slot(id).hoisted[index];

      self.round(child, left, top);
    }
  }

  /// Makes every ancestor of `id` forget its cached final layout, its measures kept.
  fn forget_final_layouts(&mut self, id: taffy::NodeId) {
    let mut ancestor = self.slot(id).parent;

    while let Some(at) = ancestor {
      let slot = self.slot_mut(at);

      slot.cache.forget_final_layout();
      ancestor = slot.parent;
    }
  }
}

/// `value` rounded to the nearest whole unit, a half up, as taffy rounds.
#[inline(always)]
fn round(value: f32) -> f32 {
  (value + 0.5).floor()
}

/// One container's engine: taffy's algorithms over its members, its root and the nodes
/// beneath it up to the next container, each container beneath them laid out by its own
/// engine from taffy's whole input; and each box positioned fixed it places that is no
/// container, with the nodes beneath it up to the next container.
pub(super) struct Engine<'t, C> {
  nodes: &'t mut Nodes<C>,
  root: taffy::NodeId,
}

impl<'t, C: Leaf> Engine<'t, C> {
  /// The engine of the container `root`, or of a node a layout starts from.
  pub(super) const fn new(nodes: &'t mut Nodes<C>, root: taffy::NodeId) -> Self {
    Self { nodes, root }
  }

  /// Lays out `id` from `inputs`, inside the block context `block` where block layout
  /// passes one: through the engine of its own where it is a container beneath the root,
  /// answering that engine's whole output.
  fn lay_out(
    &mut self,
    id: taffy::NodeId,
    inputs: LayoutInput,
    block: Option<&mut BlockContext<'_>>,
  ) -> LayoutOutput {
    if id != self.root && self.nodes.slot(id).decorated {
      let output = Engine::new(&mut *self.nodes, id).run(id, inputs, block);

      debug_assert!(
        output
          .oof_candidates
          .iter()
          .all(|candidate| candidate.position == Position::Fixed),
        "a container contains the boxes positioned absolutely inside it"
      );

      return output;
    }

    self.run(id, inputs, block)
  }

  /// Lays out `id`, a member of this engine, as taffy's own tree dispatches it: hidden
  /// where an ancestor is not displayed or it is not, by its display where it has
  /// children, measured by its context where it has none, and then the boxes it contains
  /// placed where it is laid out in full.
  ///
  /// A box laid out hidden keeps its emptied cache counted as holding entries, so a write
  /// beneath it marks the hidden ancestor; a box laid out in full while an ancestor is
  /// being sized makes every ancestor forget its final layout; and a box whose own height
  /// resolves against its parent's is served a cached size for that height alone.
  fn run(
    &mut self,
    id: taffy::NodeId,
    inputs: LayoutInput,
    block: Option<&mut BlockContext<'_>>,
  ) -> LayoutOutput {
    debug_assert!(
      self.reaches(id),
      "a node is laid out by its container's engine, or within a box positioned fixed by \
       the engine placing that box"
    );

    if inputs.run_mode == RunMode::PerformHiddenLayout {
      let slot = self.nodes.slot_mut(id);

      slot.rounded = Layout::with_order(0);
      slot.hoisted.clear();

      let output = compute_hidden_layout(self, id);

      self.nodes.slot_mut(id).cache.hold_hidden();

      return output;
    }

    let measuring = u32::from(inputs.run_mode == RunMode::ComputeSize);

    self.nodes.measuring += measuring;

    let output = compute_cached_layout(self, id, inputs, |engine, id, inputs| {
      let slot = engine.nodes.slot(id);
      let display = slot.style.display;
      let parent = !slot.children.is_empty();
      let mut output = match (display, parent) {
        (Display::None, _) => {
          engine.nodes.slot_mut(id).hoisted.clear();

          compute_hidden_layout(engine, id)
        }
        (Display::Block, true) => compute_block_layout(engine, id, inputs, block),
        (Display::FlowRoot, true) => compute_block_layout(engine, id, inputs, None),
        (Display::Flex, true) => compute_flexbox_layout(engine, id, inputs),
        (Display::Grid, true) => compute_grid_layout(engine, id, inputs),
        (_, false) => {
          let known = inputs.known_dimensions;
          let size = slot.context.as_ref().map_or(
            skene_foundation::Size::new(
              known.width.unwrap_or_default(),
              known.height.unwrap_or_default(),
            ),
            |context| context.measure(known, inputs.available_space),
          );

          engine.nodes.slot_mut(id).hoisted.clear();

          LayoutOutput::from_outer_size(taffy::geometry::Size {
            width: size.width,
            height: size.height,
          })
        }
      };

      if inputs.run_mode == RunMode::PerformLayout {
        compute_oof_layout(engine, id, &mut output);

        if engine.nodes.measuring > 0 {
          engine.nodes.forget_final_layouts(id);
        }
      }

      output
    });

    self.nodes.measuring -= measuring;

    output
  }

  /// Whether this engine lays `id` out: `id` is its root or a member beneath it, every
  /// node between them laid out by it, or `id` is a box positioned fixed or lies beneath
  /// one with no container between them, which the engine placing that box lays out.
  fn reaches(&self, id: taffy::NodeId) -> bool {
    let mut at = id;

    loop {
      if at == self.root {
        return true;
      }

      let slot = self.nodes.slot(at);

      if at != id && slot.decorated {
        return false;
      }

      if slot.style.position == Position::Fixed {
        return true;
      }

      let Some(parent) = slot.parent
      else {
        return false;
      };

      at = parent;
    }
  }
}

impl<C> TraversePartialTree for Engine<'_, C> {
  type ChildIter<'a>
    = std::iter::Copied<std::slice::Iter<'a, taffy::NodeId>>
  where
    Self: 'a;

  #[inline(always)]
  fn child_ids(&self, parent_node_id: taffy::NodeId) -> Self::ChildIter<'_> {
    self.nodes.slot(parent_node_id).children.iter().copied()
  }

  #[inline(always)]
  fn child_count(&self, parent_node_id: taffy::NodeId) -> usize {
    self.nodes.slot(parent_node_id).children.len()
  }

  #[inline(always)]
  fn get_child_id(&self, parent_node_id: taffy::NodeId, child_index: usize) -> taffy::NodeId {
    self.nodes.slot(parent_node_id).children[child_index]
  }
}

impl<C> TraverseTree for Engine<'_, C> {}

impl<C: Leaf> LayoutPartialTree for Engine<'_, C> {
  type CoreContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type CustomIdent = String;

  #[inline(always)]
  fn get_core_container_style(&self, node_id: taffy::NodeId) -> Self::CoreContainerStyle<'_> {
    &self.nodes.slot(node_id).style
  }

  #[inline(always)]
  fn set_unrounded_layout(&mut self, node_id: taffy::NodeId, layout: &Layout) {
    self.nodes.slot_mut(node_id).unrounded = *layout;
  }

  #[inline(always)]
  fn compute_child_layout(&mut self, node_id: taffy::NodeId, inputs: LayoutInput) -> LayoutOutput {
    self.lay_out(node_id, inputs, None)
  }
}

impl<C> CacheTree for Engine<'_, C> {
  #[inline(always)]
  fn cache_get(&mut self, node_id: taffy::NodeId, input: &LayoutInput) -> Option<LayoutOutput> {
    let slot = self.nodes.slot_mut(node_id);
    let resolves_parent_height = slot.style.resolves_parent_height();

    slot.cache.get_resolving(input, resolves_parent_height)
  }

  #[inline(always)]
  fn cache_store(
    &mut self,
    node_id: taffy::NodeId,
    input: &LayoutInput,
    layout_output: LayoutOutput,
  ) {
    self
      .nodes
      .slot_mut(node_id)
      .cache
      .store(input, layout_output);
  }

  #[inline(always)]
  fn cache_clear(&mut self, node_id: taffy::NodeId) {
    let _ = self.nodes.slot_mut(node_id).cache.clear();
  }
}

impl<C: Leaf> LayoutContainingBlock for Engine<'_, C> {
  type OofItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  #[inline(always)]
  fn get_oof_item_style(&self, node_id: taffy::NodeId) -> Self::OofItemStyle<'_> {
    &self.nodes.slot(node_id).style
  }

  #[inline(always)]
  fn clear_hoisted_children(&mut self, node_id: taffy::NodeId) {
    self.nodes.slot_mut(node_id).hoisted.clear();
  }

  #[inline(always)]
  fn add_hoisted_children(&mut self, node_id: taffy::NodeId, hoisted: &[taffy::NodeId]) {
    self
      .nodes
      .slot_mut(node_id)
      .hoisted
      .extend_from_slice(hoisted);
  }

  #[inline(always)]
  fn get_detailed_layout_info(&self, node_id: taffy::NodeId) -> &DetailedLayoutInfo {
    &self.nodes.slot(node_id).detailed
  }
}

impl<C: Leaf> LayoutBlockContainer for Engine<'_, C> {
  type BlockContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type BlockItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  #[inline(always)]
  fn get_block_container_style(&self, node_id: taffy::NodeId) -> Self::BlockContainerStyle<'_> {
    &self.nodes.slot(node_id).style
  }

  #[inline(always)]
  fn get_block_child_style(&self, child_node_id: taffy::NodeId) -> Self::BlockItemStyle<'_> {
    &self.nodes.slot(child_node_id).style
  }

  #[inline(always)]
  fn compute_block_child_layout(
    &mut self,
    node_id: taffy::NodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
  ) -> LayoutOutput {
    self.lay_out(node_id, inputs, block_ctx)
  }
}

impl<C: Leaf> LayoutFlexboxContainer for Engine<'_, C> {
  type FlexboxContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type FlexboxItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  #[inline(always)]
  fn get_flexbox_container_style(&self, node_id: taffy::NodeId) -> Self::FlexboxContainerStyle<'_> {
    &self.nodes.slot(node_id).style
  }

  #[inline(always)]
  fn get_flexbox_child_style(&self, child_node_id: taffy::NodeId) -> Self::FlexboxItemStyle<'_> {
    &self.nodes.slot(child_node_id).style
  }
}

impl<C: Leaf> LayoutGridContainer for Engine<'_, C> {
  type GridContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type GridItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  #[inline(always)]
  fn get_grid_container_style(&self, node_id: taffy::NodeId) -> Self::GridContainerStyle<'_> {
    &self.nodes.slot(node_id).style
  }

  #[inline(always)]
  fn get_grid_child_style(&self, child_node_id: taffy::NodeId) -> Self::GridItemStyle<'_> {
    &self.nodes.slot(child_node_id).style
  }

  #[inline(always)]
  fn set_detailed_grid_info(
    &mut self,
    node_id: taffy::NodeId,
    detailed_grid_info: DetailedGridInfo<String>,
  ) {
    self.nodes.slot_mut(node_id).detailed = DetailedLayoutInfo::Grid(Box::new(detailed_grid_info));
  }
}
