//! The retained tree every view attaches to, owned by core and mirrored into a platform.

mod annotation;
mod controls;
mod drag;
mod focus_order;
mod interaction;
mod overlays;
mod placement;
mod structure;
mod tokens;

use std::{fmt::Debug, rc::Rc};

use rustc_hash::FxHashMap;
use skene_reactive::SnapshotPlace;

pub(in crate::spine) use self::interaction::Held;
pub use self::{
  annotation::{Annotated, Annotation},
  tokens::TokenScope,
};
use self::{
  controls::AccessReports,
  drag::DragRecord,
  overlays::OverlayRecord,
  placement::Structural,
  structure::SegmentRecord,
  tokens::{ResolvedTokens, TokenCache, TokenState},
};
use crate::spine::{
  ContainerName,
  ControlSink,
  ControlState,
  Definitions,
  EventKind,
  FocusOrder,
  NodeId,
  NodeSpec,
  Part,
  Realization,
  handler::HandlerSet,
  node_map::NodeSlots,
};

/// What a node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum NodeKind {
  /// A box: a layout node with decoration.
  Box,
  /// A control: a box carrying the state a box cannot, which the control-scoped
  /// conditions inside it resolve against.
  Control,
  /// A text run.
  Run,
  /// An anchor: a layout-invisible marker a reactive subtree mounts between.
  Anchor,
  /// A scope: a layout-invisible node token definitions hang off.
  Scope,
  /// An overlay: a box shown above the app while open.
  Overlay,
}

/// One node of the scene.
#[must_use]
#[expect(
  clippy::struct_excessive_bools,
  reason = "each flag is an independent fact about the node"
)]
pub struct NodeRecord<P: Realization> {
  parent: Option<NodeId>,
  enclosing: Option<NodeId>,
  first_child: Option<NodeId>,
  last_child: Option<NodeId>,
  previous_sibling: Option<NodeId>,
  next_sibling: Option<NodeId>,
  kind: NodeKind,
  realized: bool,
  annotation: Option<Box<Annotation<P>>>,
  definitions: Option<Box<Definitions<P::TokenValue>>>,
  container: Option<ContainerName>,
  handlers: Option<Box<HandlerSet>>,
  segment: Option<Box<SegmentRecord>>,
  cache: Option<Box<TokenCache>>,
  control: Option<ControlState>,
  entry: bool,
  acting: bool,
  hidden: bool,
  caption_beside: bool,
  part: Option<Part>,
  position: SnapshotPlace,
  access: Option<Box<AccessReports>>,
  focus_order: Option<FocusOrder>,
  overlay: Option<OverlayRecord>,
  sink: Option<Rc<dyn ControlSink>>,
}

impl<P: Realization> std::fmt::Debug for NodeRecord<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("NodeRecord").finish_non_exhaustive()
  }
}

impl<P: Realization> NodeRecord<P> {
  /// The node this one is attached under.
  #[inline(always)]
  #[must_use]
  pub const fn parent(&self) -> Option<NodeId> {
    self.parent
  }

  /// The node's place in the state snapshot, matched across runs as a signal's is: a
  /// platform carries what a node holds outside the reactive graph—focus, a scroll
  /// offset, a caret—by it.
  #[inline(always)]
  #[must_use]
  pub const fn position(&self) -> &SnapshotPlace {
    &self.position
  }

  /// What the node is.
  #[inline(always)]
  pub const fn kind(&self) -> NodeKind {
    self.kind
  }

  /// Whether the platform made a node for this one; see [`Realization::create`].
  #[inline(always)]
  #[must_use]
  pub const fn is_realized(&self) -> bool {
    self.realized
  }

  /// Whether the node is a box acting on a click of its own, an activatable row; see
  /// [`BoxFacts::acting`](crate::spine::BoxFacts::acting).
  #[inline(always)]
  #[must_use]
  pub const fn is_acting(&self) -> bool {
    self.acting
  }

  /// The per-child properties a wrapper asked for this box.
  #[inline(always)]
  #[must_use]
  pub fn annotation(&self) -> Option<&Annotation<P>> {
    self.annotation.as_deref()
  }

  /// The token definitions placed on this node.
  #[inline(always)]
  #[must_use]
  pub fn definitions(&self) -> Option<&Definitions<P::TokenValue>> {
    self.definitions.as_deref()
  }

  /// The name this node publishes for container queries.
  #[inline(always)]
  #[must_use]
  pub const fn container(&self) -> Option<ContainerName> {
    self.container
  }

  /// The handlers this node carries.
  #[inline(always)]
  #[must_use]
  pub(in crate::spine) fn handlers(&self) -> Option<&HandlerSet> {
    self.handlers.as_deref()
  }

  /// The state of this node as a control; `None` for a node of any other kind.
  #[inline(always)]
  #[must_use]
  pub const fn control_state(&self) -> Option<ControlState> {
    self.control
  }

  /// The part of the enclosing control this node is recorded as; a control node is its
  /// own [`Part::Root`].
  #[inline(always)]
  #[must_use]
  pub const fn part(&self) -> Option<Part> {
    self.part
  }

  /// Whether this node is the start anchor of a segment.
  #[inline(always)]
  const fn is_segment_start(&self) -> bool {
    self.segment.is_some()
  }
}

/// A mutation the scene owes its platform, mirrored once the scene is released.
enum Mirror<P: Realization> {
  Structure(Structural),
  Destroy(NodeId),
  DefineTokens {
    scope: TokenScope,
    definitions: Definitions<P::TokenValue>,
  },
  ControlState {
    node: NodeId,
    state: ControlState,
  },
  Listen {
    node: NodeId,
    kind: EventKind,
  },
  FocusOrder {
    node: NodeId,
    order: Option<FocusOrder>,
  },
  OverlayOpen {
    node: NodeId,
    open: bool,
    anchor: Option<NodeId>,
  },
  Focus(NodeId),
}

impl<P: Realization> Mirror<P> {
  /// Hands the mutation to the platform.
  fn run(self) {
    match self {
      Self::Structure(change) => change.run::<P>(),
      Self::Destroy(node) => P::destroy(node),
      Self::DefineTokens { scope, definitions } => P::define_tokens(scope, &definitions),
      Self::ControlState { node, state } => P::set_control_state(node, &state),
      Self::Listen { node, kind } => P::listen(node, kind),
      Self::FocusOrder { node, order } => P::update_focus_order(node, order),
      Self::OverlayOpen { node, open, anchor } => P::set_overlay_open(node, open, anchor),
      Self::Focus(node) => P::focus(node),
    }
  }
}

/// The retained tree: a generational arena of [`NodeRecord`]s, every mutation mirrored
/// into the platform's records, which the platform keys by the same [`NodeId`]s.
///
/// Reached through [`with`](Self::with). No borrow of the scene is held while a user
/// callback or a platform's mutation runs: the mutations the scene mirrors into the
/// platform ([`attach`](Self::attach), [`detach`](Self::detach),
/// [`destroy`](Self::destroy), [`define_tokens`](Self::define_tokens),
/// [`set_control_state`](Self::set_control_state)) reach it in order once the borrow
/// that recorded them is released.
#[must_use]
pub struct Scene<P: Realization> {
  nodes: NodeSlots<NodeRecord<P>>,
  pending_annotation: Option<Annotation<P>>,
  tokens: TokenState<P::TokenValue>,
  graveyard: Vec<HandlerSet>,
  mirrors: Vec<Mirror<P>>,
  open_overlays: Vec<NodeId>,
  held: Held,
  kept_focus: FxHashMap<NodeId, NodeId>,
  drag: Option<DragRecord>,
  resolved_tokens: ResolvedTokens<P::Scalar>,
}

impl<P: Realization> std::fmt::Debug for Scene<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Scene").finish_non_exhaustive()
  }
}

impl<P: Realization> Default for Scene<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Scene<P> {
  /// An empty scene.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      nodes: NodeSlots::new(),
      pending_annotation: None,
      tokens: TokenState::new(),
      graveyard: Vec::new(),
      mirrors: Vec::new(),
      open_overlays: Vec::new(),
      held: Held::new(),
      kept_focus: FxHashMap::default(),
      drag: None,
      resolved_tokens: ResolvedTokens::default(),
    }
  }

  /// Runs `f` on the platform's scene.
  ///
  /// The mutations `f` recorded are mirrored into the platform after the scene is
  /// released, in order, and handlers released inside `f` are dropped after that, so
  /// a platform's mutation or a captured value's drop may reach the scene again.
  ///
  /// # Panics
  ///
  /// Panics if the scene is already borrowed: `f` must not re-enter it.
  pub fn with<R>(operation: impl FnOnce(&mut Self) -> R) -> R {
    P::scene().with(|cell| {
      let mut scene = cell.borrow_mut();
      let result = operation(&mut scene);
      let mirrors = std::mem::take(&mut scene.mirrors);
      let graveyard = std::mem::take(&mut scene.graveyard);

      drop(scene);

      for mirror in mirrors {
        mirror.run();
      }

      drop(graveyard);

      result
    })
  }

  /// Runs `f` on the platform's scene for reading; reads may nest, and a read records
  /// nothing to mirror.
  ///
  /// # Panics
  ///
  /// Panics if the scene is borrowed by [`with`](Self::with).
  pub fn read<R>(operation: impl FnOnce(&Self) -> R) -> R {
    P::scene().with(|cell| operation(&cell.borrow()))
  }

  /// Whether the scene is borrowed right now, so a [`with`](Self::with) would panic.
  #[inline(always)]
  #[must_use]
  pub fn is_borrowed() -> bool {
    P::scene().with(|cell| cell.try_borrow_mut().is_err())
  }

  /// The record of `node`, if it is live.
  #[inline(always)]
  #[must_use]
  pub fn record(&self, node: NodeId) -> Option<&NodeRecord<P>> {
    self.nodes.get(node)
  }

  /// The record of `node`, mutably, if it is live.
  #[inline(always)]
  #[must_use]
  pub fn record_mut(&mut self, node: NodeId) -> Option<&mut NodeRecord<P>> {
    self.nodes.get_mut(node)
  }

  /// Whether `node` is live.
  #[inline(always)]
  #[must_use]
  pub fn contains(&self, node: NodeId) -> bool {
    self.record(node).is_some()
  }

  /// Mints a detached node of `kind`, recording the `annotation` a wrapper gave it; a
  /// control starts in the default [`ControlState`].
  ///
  /// The platform realizes the node, keyed by the id this returns, once the scene is
  /// released and before the node takes part in any other operation, and the node
  /// counts as one the platform made; [`realize`](Self::realize) records the
  /// platform's answer instead.
  ///
  /// # Panics
  ///
  /// Panics past 2^32 live nodes.
  pub fn create(&mut self, kind: NodeKind, annotation: Option<Annotation<P>>) -> NodeId {
    let is_control = kind == NodeKind::Control;
    let position = SnapshotPlace::next();
    let record = NodeRecord {
      parent: None,
      enclosing: None,
      first_child: None,
      last_child: None,
      previous_sibling: None,
      next_sibling: None,
      kind,
      realized: true,
      annotation: annotation.map(Box::new),
      definitions: None,
      container: None,
      handlers: None,
      segment: None,
      cache: None,
      control: is_control.then_some(ControlState::DEFAULT),
      entry: false,
      acting: false,
      hidden: false,
      caption_beside: false,
      part: is_control.then_some(Part::Root),
      position,
      access: None,
      focus_order: None,
      overlay: None,
      sink: None,
    };

    self.nodes.insert(record)
  }

  /// Mints a detached node as `spec` describes it, recording the `annotation` a wrapper
  /// gave it, and has the platform create it, recording whether it made a node; see
  /// [`Realization::create`].
  ///
  /// # Panics
  ///
  /// Panics past 2^32 live nodes, or if the scene is borrowed.
  pub fn realize(spec: &NodeSpec, annotation: Option<Annotation<P>>) -> NodeId {
    let node = Self::with(|scene| scene.mint(spec, annotation));

    if !P::create(node, spec) {
      Self::with(|scene| {
        if let Some(record) = scene.record_mut(node) {
          record.realized = false;
        }
      });
    }

    node
  }

  /// Mints a detached node as `spec` describes it, recording the `annotation` a wrapper
  /// gave it, and has the platform adopt `native`, its own view, as the node, recording
  /// whether it made a node; see [`Realization::adopt`].
  ///
  /// # Panics
  ///
  /// Panics past 2^32 live nodes, or if the scene is borrowed.
  pub fn adopt(spec: &NodeSpec, native: P::Native, annotation: Option<Annotation<P>>) -> NodeId {
    let node = Self::with(|scene| scene.mint(spec, annotation));

    if !P::adopt(node, native, spec) {
      Self::with(|scene| {
        if let Some(record) = scene.record_mut(node) {
          record.realized = false;
        }
      });
    }

    node
  }

  /// Mints a detached node as `spec` describes it, recording the `annotation` a wrapper
  /// gave it and whether it acts on a click of its own.
  fn mint(&mut self, spec: &NodeSpec, annotation: Option<Annotation<P>>) -> NodeId {
    let node = self.create(spec.kind(), annotation);

    if let Some(record) = self.record_mut(node) {
      record.acting = spec.acting();
    }

    node
  }

  /// The live node created at `position`, if any.
  #[must_use]
  pub fn node_at(&self, position: &SnapshotPlace) -> Option<NodeId> {
    self
      .nodes
      .iter()
      .find_map(|(node, record)| (record.position == *position).then_some(node))
  }

  /// Takes the record of `node` out of the arena, retiring the slot's generation; a
  /// slot whose generations are exhausted is retired whole, so a stale id never names a
  /// live node.
  fn remove(&mut self, node: NodeId) -> Option<NodeRecord<P>> {
    self.nodes.remove(node)
  }
}
