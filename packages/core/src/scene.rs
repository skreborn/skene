//! The retained tree every view attaches to, owned by core and mirrored into a platform.

use std::{any::TypeId, fmt::Debug, rc::Rc};

use rustc_hash::{FxHashMap, FxHashSet};
use skene_reactive::Position;
use smallvec::SmallVec;

use super::{
  Condition,
  ContainerName,
  ControlState,
  DeclaredBoxStyle,
  Definitions,
  EventKind,
  NodeId,
  Part,
  Platform,
  TokenName,
  Value,
};
use crate::{
  AccessProp,
  Dimension,
  DragData,
  DragSource,
  DropHandler,
  Flexibility,
  Focus,
  FocusOrder,
  GridPlacement,
  Handler,
  Inset,
  Interaction,
  ItemAlignment,
  Line,
  Registered,
  handler::HandlerSet,
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

/// The per-child properties a wrapper asked for the next box built under it.
///
/// Folded into the child's own declared style before the child is created, so each
/// takes conditions, tokens and derived values like any property.
#[must_use]
pub struct Annotation<P: Platform> {
  /// The child's flexibility in a flex container.
  pub flex: Option<Annotated<P, Flexibility<P::Scalar>>>,
  /// The child's own cross- or block-axis alignment.
  pub self_alignment: Option<Annotated<P, ItemAlignment>>,
  /// The child's own inline-axis alignment in a grid.
  pub self_horizontal_alignment: Option<Annotated<P, ItemAlignment>>,
  /// The edges the child is pinned to in a stack.
  pub pinned: Option<Annotated<P, Inset<Dimension<P::Scalar>>>>,
  /// The child's row placement in a grid.
  pub grid_row_placement: Option<Annotated<P, Line<GridPlacement>>>,
  /// The child's column placement in a grid.
  pub grid_column_placement: Option<Annotated<P, Line<GridPlacement>>>,
}

/// A property value as an annotation carries it: declared under the platform's
/// conditions.
pub type Annotated<P, T> = Value<T, Condition<<P as Platform>::Scalar>>;

impl<P: Platform> Annotation<P> {
  /// This annotation as a box style holding its properties and nothing else.
  pub fn into_style(self) -> DeclaredBoxStyle<P> {
    let mut style = DeclaredBoxStyle::<P>::default();

    self.apply_to(&mut style);

    style
  }

  /// Writes every property set here into `style`, replacing what it held.
  pub fn apply_to(self, style: &mut DeclaredBoxStyle<P>) {
    if let Some(flex) = self.flex {
      style.flex = Some(flex);
    }

    if let Some(alignment) = self.self_alignment {
      style.self_alignment = Some(alignment);
    }

    if let Some(alignment) = self.self_horizontal_alignment {
      style.self_horizontal_alignment = Some(alignment);
    }

    if let Some(pinned) = self.pinned {
      style.pinned = Some(pinned);
    }

    if let Some(placement) = self.grid_row_placement {
      style.grid_row_placement = Some(placement);
    }

    if let Some(placement) = self.grid_column_placement {
      style.grid_column_placement = Some(placement);
    }
  }
}

impl<P: Platform> Default for Annotation<P> {
  #[inline(always)]
  fn default() -> Self {
    Self {
      flex: None,
      self_alignment: None,
      self_horizontal_alignment: None,
      pinned: None,
      grid_row_placement: None,
      grid_column_placement: None,
    }
  }
}

impl<P: Platform> Clone for Annotation<P> {
  fn clone(&self) -> Self {
    Self {
      flex: self.flex.clone(),
      self_alignment: self.self_alignment.clone(),
      self_horizontal_alignment: self.self_horizontal_alignment.clone(),
      pinned: self.pinned.clone(),
      grid_row_placement: self.grid_row_placement.clone(),
      grid_column_placement: self.grid_column_placement.clone(),
    }
  }
}

impl<P: Platform> PartialEq for Annotation<P> {
  fn eq(&self, other: &Self) -> bool {
    self.flex == other.flex
      && self.self_alignment == other.self_alignment
      && self.self_horizontal_alignment == other.self_horizontal_alignment
      && self.pinned == other.pinned
      && self.grid_row_placement == other.grid_row_placement
      && self.grid_column_placement == other.grid_column_placement
  }
}

impl<P: Platform> Debug for Annotation<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Annotation")
      .field("flex", &self.flex)
      .field("self_alignment", &self.self_alignment)
      .field("self_horizontal_alignment", &self.self_horizontal_alignment)
      .field("pinned", &self.pinned)
      .field("grid_row_placement", &self.grid_row_placement)
      .field("grid_column_placement", &self.grid_column_placement)
      .finish()
  }
}

/// A drag in progress: the node it started from, what it carries, and the node taking
/// it.
struct DragRecord {
  source: NodeId,
  data: DragData,
  target: Option<NodeId>,
}

/// The end of a segment and, while the segment is detached, the content waiting
/// between its anchors.
struct SegmentRecord {
  end: NodeId,
  pending: Vec<NodeId>,
}

/// The token definitions a node looked up under one definitions generation, each with
/// the node defining it.
struct TokenCache<V> {
  generation: u64,
  values: FxHashMap<TokenName, Option<(Option<NodeId>, V)>>,
}

/// One node of the scene.
#[must_use]
pub struct NodeRecord<P: Platform> {
  parent: Option<NodeId>,
  enclosing: Option<NodeId>,
  first_child: Option<NodeId>,
  last_child: Option<NodeId>,
  previous_sibling: Option<NodeId>,
  next_sibling: Option<NodeId>,
  kind: NodeKind,
  annotation: Option<Annotation<P>>,
  definitions: Option<Box<Definitions<P::TokenValue>>>,
  container: Option<ContainerName>,
  handlers: Option<Box<HandlerSet>>,
  segment: Option<Box<SegmentRecord>>,
  cache: Option<Box<TokenCache<P::TokenValue>>>,
  control: Option<ControlState>,
  entry: bool,
  hidden: bool,
  part: Option<Part>,
  position: Position,
  access: Option<Box<AccessReports>>,
  focus_order: Option<FocusOrder>,
  overlay: Option<OverlayRecord>,
}

/// An overlay's standing: whether it blocks what lies beneath it while open, whether it
/// is open, the node it opened against, and the node focused when it opened.
#[derive(Debug, Clone, Copy)]
struct OverlayRecord {
  modal: bool,
  open: bool,
  anchor: Option<NodeId>,
  opener: Option<NodeId>,
}

impl<P: Platform> std::fmt::Debug for NodeRecord<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("NodeRecord").finish_non_exhaustive()
  }
}

/// What a node reports to assistive technology: the view's own report, and the report
/// of a view describing it.
#[derive(Default)]
struct AccessReports {
  own: Vec<AccessProp>,
  description: Vec<AccessProp>,
}

impl AccessReports {
  /// Both reports, the node's own first, with their relations folded.
  fn merged(&self) -> Vec<AccessProp> {
    AccessProp::fold(self.own.iter().chain(&self.description).cloned())
  }
}

impl<P: Platform> NodeRecord<P> {
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
  pub const fn position(&self) -> &Position {
    &self.position
  }

  /// What the node is.
  #[inline(always)]
  pub const fn kind(&self) -> NodeKind {
    self.kind
  }

  /// The per-child properties a wrapper asked for this box.
  #[inline(always)]
  #[must_use]
  pub const fn annotation(&self) -> Option<&Annotation<P>> {
    self.annotation.as_ref()
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
  pub(crate) fn handlers(&self) -> Option<&HandlerSet> {
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

struct Entry<P: Platform> {
  generation: u32,
  record: Option<NodeRecord<P>>,
}

/// A mutation the scene owes its platform, mirrored once the scene is released.
enum Mirror<P: Platform> {
  Attach {
    parent: NodeId,
    child: NodeId,
    before: Option<NodeId>,
  },
  Relocate {
    parent: NodeId,
    child: NodeId,
    before: Option<NodeId>,
  },
  Hide {
    parent: NodeId,
    child: NodeId,
  },
  Remove {
    parent: NodeId,
    child: NodeId,
  },
  Destroy(NodeId),
  DefineTokens {
    scope: NodeId,
    definitions: Definitions<P::TokenValue>,
  },
  DefineRootTokens(Definitions<P::TokenValue>),
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

impl<P: Platform> Mirror<P> {
  /// Hands the mutation to the platform.
  fn run(self) {
    match self {
      Self::Attach {
        parent,
        child,
        before,
      } => P::attach(parent, child, before),
      Self::Relocate {
        parent,
        child,
        before,
      } => P::relocate(parent, child, before),
      Self::Hide { parent, child } => P::hide(parent, child),
      Self::Remove { parent, child } => P::remove(parent, child),
      Self::Destroy(node) => P::destroy(node),
      Self::DefineTokens { scope, definitions } => P::define_tokens(scope, &definitions),
      Self::DefineRootTokens(definitions) => P::define_root_tokens(&definitions),
      Self::ControlState { node, state } => P::set_control_state(node, &state),
      Self::Listen { node, kind } => P::listen(node, kind),
      Self::FocusOrder { node, order } => P::update_focus_order(node, order),
      Self::OverlayOpen { node, open, anchor } => P::set_overlay_open(node, open, anchor),
      Self::Focus(node) => P::focus(node),
    }
  }
}

/// How a node comes to stand under a parent: from nowhere, or from under another.
#[derive(Clone, Copy)]
enum Arrival {
  Attach,
  Relocate,
}

/// Why a node leaves its parent: to return while it stays live, or for good.
#[derive(Clone, Copy)]
enum Departure {
  Hide,
  Remove,
}

/// The retained tree: a generational arena of [`NodeRecord`]s, every mutation mirrored
/// into the platform's records, which the platform keys by the same [`NodeId`]s.
///
/// Reached through [`with`](Self::with). No borrow of the scene is held while a user
/// callback or a platform's mutation runs: dispatch and every handler invocation snapshot
/// what they need and release the scene first, and the mutations the scene mirrors into
/// the platform ([`attach`](Self::attach), [`detach`](Self::detach),
/// [`destroy`](Self::destroy), [`define_tokens`](Self::define_tokens),
/// [`define_root_tokens`](Self::define_root_tokens),
/// [`set_control_state`](Self::set_control_state)) reach it in order once the borrow
/// that recorded them is released.
#[must_use]
pub struct Scene<P: Platform> {
  entries: Vec<Entry<P>>,
  free: Vec<u32>,
  pending_annotation: Option<Annotation<P>>,
  root_definitions: Definitions<P::TokenValue>,
  root_sets: FxHashSet<TypeId>,
  definitions_generation: u64,
  graveyard: Vec<HandlerSet>,
  mirrors: Vec<Mirror<P>>,
  open_overlays: Vec<NodeId>,
  hovered: Option<NodeId>,
  pressed: Option<NodeId>,
  focused: Option<NodeId>,
  kept_focus: FxHashMap<NodeId, NodeId>,
  focus_visible: bool,
  drag: Option<DragRecord>,
  #[cfg(debug_assertions)]
  token_kinds: FxHashMap<TokenName, std::mem::Discriminant<P::TokenValue>>,
}

impl<P: Platform> std::fmt::Debug for Scene<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Scene").finish_non_exhaustive()
  }
}

impl<P: Platform> Default for Scene<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Scene<P> {
  /// An empty scene.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      entries: Vec::new(),
      free: Vec::new(),
      pending_annotation: None,
      root_definitions: Definitions::from(Vec::new()),
      root_sets: FxHashSet::default(),
      definitions_generation: 0,
      graveyard: Vec::new(),
      mirrors: Vec::new(),
      open_overlays: Vec::new(),
      hovered: None,
      pressed: None,
      focused: None,
      kept_focus: FxHashMap::default(),
      focus_visible: true,
      drag: None,
      #[cfg(debug_assertions)]
      token_kinds: FxHashMap::default(),
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
    self
      .entries
      .get(node.index() as usize)
      .filter(|entry| entry.generation == node.generation())
      .and_then(|entry| entry.record.as_ref())
  }

  /// The record of `node`, mutably, if it is live.
  #[inline(always)]
  #[must_use]
  pub fn record_mut(&mut self, node: NodeId) -> Option<&mut NodeRecord<P>> {
    self
      .entries
      .get_mut(node.index() as usize)
      .filter(|entry| entry.generation == node.generation())
      .and_then(|entry| entry.record.as_mut())
  }

  /// Whether `node` is live.
  #[inline(always)]
  #[must_use]
  pub fn contains(&self, node: NodeId) -> bool {
    self.record(node).is_some()
  }

  /// Mints a detached node of `kind`; a control starts in the default
  /// [`ControlState`].
  ///
  /// The platform realizes the node, keyed by the id this returns, once the scene is
  /// released and before the node takes part in any other operation.
  ///
  /// # Panics
  ///
  /// Panics past 2^32 live nodes.
  pub fn create(&mut self, kind: NodeKind) -> NodeId {
    let is_control = kind == NodeKind::Control;
    let position = Position::next();
    let record = NodeRecord {
      parent: None,
      enclosing: None,
      first_child: None,
      last_child: None,
      previous_sibling: None,
      next_sibling: None,
      kind,
      annotation: None,
      definitions: None,
      container: None,
      handlers: None,
      segment: None,
      cache: None,
      control: is_control.then_some(ControlState::DEFAULT),
      entry: false,
      hidden: false,
      part: is_control.then_some(Part::Root),
      position,
      access: None,
      focus_order: None,
      overlay: None,
    };

    if let Some(index) = self.free.pop() {
      let entry = &mut self.entries[index as usize];

      entry.record = Some(record);

      return NodeId::new(index, entry.generation);
    }

    let index = u32::try_from(self.entries.len()).expect("a scene holds at most 2^32 nodes");

    self.entries.push(Entry {
      generation: 0,
      record: Some(record),
    });

    NodeId::new(index, 0)
  }

  /// The live node created at `position`, if any.
  #[must_use]
  pub fn node_at(&self, position: &Position) -> Option<NodeId> {
    self.entries.iter().enumerate().find_map(|(index, entry)| {
      let record = entry.record.as_ref()?;

      (record.position == *position)
        .then(|| {
          u32::try_from(index)
            .ok()
            .map(|index| NodeId::new(index, entry.generation))
        })
        .flatten()
    })
  }

  /// Mints a detached segment: a start and an end anchor, attached and detached as one
  /// with whatever is mounted between them. The platform realizes the anchors once the
  /// scene is released.
  ///
  /// Returns `(start, end)`; the start stands for the segment everywhere.
  pub fn create_segment() -> (NodeId, NodeId) {
    let (start, end) = Self::with(|scene| {
      let start = scene.create(NodeKind::Anchor);
      let end = scene.create(NodeKind::Anchor);

      if let Some(record) = scene.record_mut(start) {
        record.segment = Some(Box::new(SegmentRecord {
          end,
          pending: Vec::new(),
        }));
      }

      (start, end)
    });

    P::create_anchor(start);
    P::create_anchor(end);

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
    self
      .entries
      .iter()
      .enumerate()
      .filter_map(|(index, entry)| {
        let record = entry.record.as_ref()?;

        if record.parent.is_some() || record.enclosing.is_some() || record.hidden {
          return None;
        }

        Some(NodeId::new(u32::try_from(index).ok()?, entry.generation))
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
  fn children_reversed(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
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
    let kept = to.map(|to| self.ancestry(to)).unwrap_or_default();

    from
      .map(|from| self.ancestry(from))
      .unwrap_or_default()
      .into_iter()
      .filter(|node| !kept.contains(node))
      .collect()
  }

  /// The deepest node under the pointer.
  #[inline(always)]
  #[must_use]
  pub const fn hovered(&self) -> Option<NodeId> {
    self.hovered
  }

  /// Moves the pointer over `next`, the deepest node under it, returning the nodes it
  /// left, innermost first, and those it entered, outermost first.
  pub fn move_pointer(
    &mut self,
    next: Option<NodeId>,
  ) -> (SmallVec<[NodeId; 16]>, SmallVec<[NodeId; 16]>) {
    let previous = std::mem::replace(&mut self.hovered, next);
    let left = self.crossed(previous, next);
    let mut entered = self.crossed(next, previous);

    entered.reverse();

    (left, entered)
  }

  /// Whether `node` is under the pointer or holds the node that is, as `:hover` holds.
  #[must_use]
  pub fn is_hovered(&self, node: NodeId) -> bool {
    self
      .hovered
      .is_some_and(|hovered| self.ancestry(hovered).contains(&node))
  }

  /// The deepest node a primary press landed on, until its release.
  #[inline(always)]
  #[must_use]
  pub const fn pressed(&self) -> Option<NodeId> {
    self.pressed
  }

  /// Holds a primary press on `node`, or ends the press for `None`.
  #[inline(always)]
  pub const fn set_pressed(&mut self, node: Option<NodeId>) {
    self.pressed = node;
  }

  /// Whether a primary press is held on `node` or a node beneath it, as `:active` holds.
  #[must_use]
  pub fn is_pressed(&self, node: NodeId) -> bool {
    self
      .pressed
      .is_some_and(|pressed| self.ancestry(pressed).contains(&node))
  }

  /// The node holding focus.
  #[inline(always)]
  #[must_use]
  pub const fn focused(&self) -> Option<NodeId> {
    self.focused
  }

  /// Moves focus to `next`, returning the nodes whose subtree it left, innermost first,
  /// and those whose subtree it reached, outermost first.
  pub fn move_focus(
    &mut self,
    next: Option<NodeId>,
  ) -> (SmallVec<[NodeId; 16]>, SmallVec<[NodeId; 16]>) {
    let previous = std::mem::replace(&mut self.focused, next);
    let left = self.crossed(previous, next);
    let mut reached = self.crossed(next, previous);

    reached.reverse();

    (left, reached)
  }

  /// Whether focus shows while held: set by a key, cleared by a pointer press.
  #[inline(always)]
  #[must_use]
  pub const fn focus_visible(&self) -> bool {
    self.focus_visible
  }

  /// Records whether focus shows while held.
  #[inline(always)]
  pub const fn set_focus_visible(&mut self, visible: bool) {
    self.focus_visible = visible;
  }

  /// Records that the control `node` is a text entry, whose focus shows however it was
  /// reached, as `:focus-visible` does on an `<input>`.
  pub fn note_entry(&mut self, node: NodeId) {
    if let Some(record) = self.record_mut(node) {
      record.entry = true;
    }
  }

  /// Whether `node` holds focus that shows: reached by a key and not since a pointer
  /// press, or held by a text entry.
  #[must_use]
  pub fn focus_shows(&self, node: NodeId) -> bool {
    self.focused == Some(node)
      && (self.focus_visible || self.record(node).is_some_and(|record| record.entry))
  }

  /// The interaction `node`'s conditions resolve against: its own hover, the focus and
  /// press of the control enclosing it or of itself, and that control's state.
  pub fn interaction(&self, node: NodeId) -> Interaction {
    let control = self.enclosing_control(node);
    let holder = control.unwrap_or(node);

    Interaction {
      hovered: self.is_hovered(node),
      focus: Focus::new(self.focused == Some(holder), self.focus_shows(holder)),
      pressed: self.is_pressed(holder),
      dragged: self.dragged() == Some(node),
      targeted: self.drag_target() == Some(node),
      control: control.and_then(|control| self.control_state(control)),
    }
  }

  /// Calls `f` with each child of `node`, in order.
  #[inline(always)]
  pub fn for_each_child(&self, node: NodeId, mut visit: impl FnMut(NodeId)) {
    for child in self.children(node) {
      visit(child);
    }
  }

  /// Places `child` under `parent`, immediately before `before` when given, else last,
  /// moving it from any previous parent; nothing where it already stands there, or
  /// where either is gone.
  ///
  /// A segment start brings its content and its end anchor along; `child` is mounted in
  /// no segment after. A node detached earlier takes back the focus it held when it
  /// left, where focus has gone nowhere since.
  pub fn attach(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
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
  /// platform releases the record of every node destroyed.
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

      if record.overlay.is_some_and(|overlay| overlay.open) {
        self.open_overlays.retain(|open| *open != node);
      }

      for held in [&mut self.hovered, &mut self.pressed, &mut self.focused] {
        if *held == Some(node) {
          *held = None;
        }
      }

      self.kept_focus.remove(&node);

      if self.drag.as_ref().is_some_and(|drag| drag.source == node) {
        self.drag = None;
      }

      if let Some(drag) = &mut self.drag
        && drag.target == Some(node)
      {
        drag.target = None;
      }

      self.mirrors.push(Mirror::Destroy(node));
    }

    self.definitions_generation += 1;
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
  /// detached but live.
  pub fn remove_from_segment(&mut self, start: NodeId, node: NodeId) {
    let Some((parent, _)) = self.segment_place(start)
    else {
      return;
    };

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

  /// Builds the node `build` returns under `annotation`, where there is one: the first
  /// box, control, run or low-level node `build` creates takes it, and no node built
  /// after `build` returns does.
  pub fn annotate(
    annotation: impl Into<Option<Annotation<P>>>,
    build: impl FnOnce() -> NodeId,
  ) -> NodeId {
    let annotation = annotation.into();

    Self::with(|scene| scene.pending_annotation = annotation);

    let node = build();

    Self::with(|scene| scene.pending_annotation = None);

    node
  }

  /// Stores the annotation the next box, control, run or low-level node created takes;
  /// [`annotate`](Self::annotate) scopes it to one build.
  #[inline(always)]
  pub fn set_pending_annotation(&mut self, annotation: Annotation<P>) {
    self.pending_annotation = Some(annotation);
  }

  /// Takes the pending annotation, for the node about to be created: a box folds it
  /// into its style and records it, any other node records it.
  #[inline(always)]
  pub const fn take_pending_annotation(&mut self) -> Option<Annotation<P>> {
    self.pending_annotation.take()
  }

  /// Records the annotation a box was built with.
  #[inline(always)]
  pub fn set_annotation(&mut self, node: NodeId, annotation: Annotation<P>) {
    if let Some(record) = self.record_mut(node) {
      record.annotation = Some(annotation);
    }
  }

  /// Panics, in a debug build, when a token of `definitions` was defined before as
  /// another kind: one name is one kind wherever it is defined.
  #[cfg(debug_assertions)]
  fn check_kinds(&mut self, definitions: &Definitions<P::TokenValue>) {
    for (name, value) in definitions.iter() {
      let kind = std::mem::discriminant(value);
      let first = *self.token_kinds.entry(name).or_insert(kind);

      assert!(first == kind, "the token `{name}` is defined as two kinds");
    }
  }

  /// Defines tokens on `node`, replacing what it defined before.
  ///
  /// # Panics
  ///
  /// Panics, in a debug build, when a token was defined before as another kind.
  pub fn define_tokens(&mut self, node: NodeId, definitions: Definitions<P::TokenValue>) {
    #[cfg(debug_assertions)]
    self.check_kinds(&definitions);

    let Some(record) = self.record_mut(node)
    else {
      return;
    };

    record.definitions = Some(Box::new(definitions.clone()));
    self.mirrors.push(Mirror::DefineTokens {
      scope: node,
      definitions,
    });
    self.definitions_generation += 1;
  }

  /// Defines the tokens every node falls back to, replacing the previous set.
  ///
  /// # Panics
  ///
  /// Panics, in a debug build, when a token was defined before as another kind.
  pub fn define_root_tokens(&mut self, definitions: Definitions<P::TokenValue>) {
    #[cfg(debug_assertions)]
    self.check_kinds(&definitions);

    self
      .mirrors
      .push(Mirror::DefineRootTokens(definitions.clone()));
    self.root_definitions = definitions;
    self.definitions_generation += 1;
  }

  /// Whether the definitions of `set` were laid over the root.
  #[inline(always)]
  #[must_use]
  pub fn defines_root_set(&self, set: TypeId) -> bool {
    self.root_sets.contains(&set)
  }

  /// Lays `set`'s `definitions` over the tokens every node falls back to, once per set: a
  /// later call for the same set does nothing and returns `false`.
  ///
  /// [`defines_root_set`](Self::defines_root_set) tells a caller whether the set's
  /// definitions are wanted before it builds them, outside the scene's borrow.
  pub fn define_root_tokens_once(
    &mut self,
    set: TypeId,
    definitions: Definitions<P::TokenValue>,
  ) -> bool {
    if !self.root_sets.insert(set) {
      return false;
    }

    let mut root = std::mem::replace(&mut self.root_definitions, Definitions::from(Vec::new()));

    root.overlay(definitions);
    self.define_root_tokens(root);

    true
  }

  /// The tokens every node falls back to.
  #[inline(always)]
  pub const fn root_definitions(&self) -> &Definitions<P::TokenValue> {
    &self.root_definitions
  }

  /// The definition of `name` in force at `node`: the nearest defining ancestor's, else
  /// the root's.
  ///
  /// Cached per node until a definition or the tree changes.
  pub fn resolve_token(&mut self, node: NodeId, name: TokenName) -> Option<P::TokenValue> {
    self
      .resolve_scoped_token(node, name)
      .map(|(_, value)| value)
  }

  /// The definition of `name` in force at `node`, with the node defining it: the
  /// nearest defining ancestor, else `None` for the root.
  ///
  /// Cached per node until a definition or the tree changes.
  pub fn resolve_scoped_token(
    &mut self,
    node: NodeId,
    name: TokenName,
  ) -> Option<(Option<NodeId>, P::TokenValue)> {
    let generation = self.definitions_generation;
    let cached = self
      .record(node)
      .and_then(|record| record.cache.as_ref())
      .filter(|cache| cache.generation == generation)
      .and_then(|cache| cache.values.get(&name));

    if let Some(value) = cached {
      return value.clone();
    }

    let mut current = Some(node);
    let mut found = None;

    while let Some(visited) = current {
      let record = self.record(visited)?;

      if let Some(value) = record
        .definitions
        .as_ref()
        .and_then(|definitions| definitions.get(name))
      {
        found = Some((Some(visited), value.clone()));

        break;
      }

      current = record.parent;
    }

    let found = found.or_else(|| {
      self
        .root_definitions
        .get(name)
        .map(|value| (None, value.clone()))
    });
    let record = self.record_mut(node)?;
    let cache = record.cache.get_or_insert_with(|| {
      Box::new(TokenCache {
        generation,
        values: FxHashMap::default(),
      })
    });

    if cache.generation != generation {
      cache.generation = generation;
      cache.values.clear();
    }

    cache.values.insert(name, found.clone());

    found
  }

  /// Names `node` for container queries against its size.
  #[inline(always)]
  pub fn set_container(&mut self, node: NodeId, name: ContainerName) {
    if let Some(record) = self.record_mut(node) {
      record.container = Some(name);
    }
  }

  /// Records `node`'s place in focus traversal as the platform was given it at
  /// creation; `None` keeps it out of focus.
  pub fn note_focus_order(&mut self, node: NodeId, order: Option<FocusOrder>) {
    if let Some(record) = self.record_mut(node) {
      record.focus_order = order;
    }
  }

  /// Replaces `node`'s place in focus traversal and mirrors it; `None` takes it out of
  /// focus.
  pub fn set_focus_order(&mut self, node: NodeId, order: Option<FocusOrder>) {
    let Some(record) = self.record_mut(node)
    else {
      return;
    };

    record.focus_order = order;
    self.mirrors.push(Mirror::FocusOrder { node, order });
  }

  /// `node`'s place in focus traversal; `None` for a node focus never reaches.
  #[inline(always)]
  #[must_use]
  pub fn focus_order(&self, node: NodeId) -> Option<FocusOrder> {
    self.record(node).and_then(|record| record.focus_order)
  }

  /// The node that takes focus for the subtree `node` roots: `node` itself where it
  /// has a place in focus traversal, else the first node beneath it with one, in
  /// document order, a segment's content counting as beneath its start; `node` where
  /// none has.
  pub fn focus_holder_within(&self, node: NodeId) -> NodeId {
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
      let Some(record) = self.record(current)
      else {
        continue;
      };

      if record.focus_order.is_some() {
        return current;
      }

      if record.is_segment_start() {
        stack.extend(self.segment_nodes(current).into_iter().rev());
      }
      else {
        stack.extend(self.children_reversed(current));
      }
    }

    node
  }

  /// Records that `node` is an overlay, closed, `modal` when it blocks what lies beneath
  /// it while open.
  pub fn note_overlay(&mut self, node: NodeId, modal: bool) {
    if let Some(record) = self.record_mut(node) {
      record.overlay = Some(OverlayRecord {
        modal,
        open: false,
        anchor: None,
        opener: None,
      });
    }
  }

  /// Opens or closes the overlay `node`, placed against `anchor` while open, and mirrors
  /// it; an opening puts it on top of every open overlay, as a browser's top layer
  /// stacks, and a node of any other kind is left alone.
  ///
  /// A close of a modal, or of an overlay holding focus, returns focus as a browser's
  /// top layer does: to the node focused when it opened, while that node takes focus,
  /// else to the nearest node taking focus at or above the anchor it opened against.
  pub fn set_overlay_open(&mut self, node: NodeId, open: bool, anchor: Option<NodeId>) {
    let placed = anchor.and_then(|anchor| self.placement(anchor));
    let focused = self.focused;
    let Some(record) = self.record_mut(node)
    else {
      return;
    };
    let Some(overlay) = record.overlay.as_mut()
    else {
      return;
    };
    let was_open = std::mem::replace(&mut overlay.open, open);
    let opened_against = std::mem::replace(&mut overlay.anchor, anchor);
    let modal = overlay.modal;
    let opener = match (was_open, open) {
      (false, true) => {
        overlay.opener = focused;

        None
      }
      (true, false) => overlay.opener.take(),
      _ => None,
    };

    match (was_open, open) {
      (false, true) => self.open_overlays.push(node),
      (true, false) => self.open_overlays.retain(|open| *open != node),
      _ => {}
    }

    self.mirrors.push(Mirror::OverlayOpen {
      node,
      open,
      anchor: placed,
    });

    if was_open && !open {
      self.return_focus(node, modal, opener, opened_against);
    }
  }

  /// Returns focus from the overlay `node` as it closes, where it is `modal` or holds
  /// focus: to `opener` while it takes focus, else to the nearest node taking focus at
  /// or above `anchor`.
  fn return_focus(
    &mut self,
    node: NodeId,
    modal: bool,
    opener: Option<NodeId>,
    anchor: Option<NodeId>,
  ) {
    let inside = self
      .focused
      .is_some_and(|focused| self.ancestry(focused).contains(&node));

    if !modal && !inside {
      return;
    }

    let overlays = self.open_overlays.clone();
    let takes_focus = |scene: &Self, candidate: NodeId| {
      scene.focus_order(candidate).is_some() && scene.reachable_under_modal(&overlays, candidate)
    };
    let target = opener
      .filter(|opener| takes_focus(self, *opener))
      .or_else(|| {
        anchor.and_then(|anchor| {
          self
            .ancestry(anchor)
            .into_iter()
            .find(|candidate| takes_focus(self, *candidate))
        })
      });

    if let Some(target) = target {
      self.mirrors.push(Mirror::Focus(target));
    }
  }

  /// The node focused when the overlay `node` opened, while it is open.
  #[inline(always)]
  #[must_use]
  pub fn overlay_opener(&self, node: NodeId) -> Option<NodeId> {
    self
      .record(node)
      .and_then(|record| record.overlay)
      .filter(|overlay| overlay.open)
      .and_then(|overlay| overlay.opener)
  }

  /// The node an overlay anchored on `node` is placed against: `node` itself, or for a
  /// segment or a scope, the first node inside it with a box of its own, in document
  /// order; `None` where there is none.
  #[must_use]
  pub fn placement(&self, node: NodeId) -> Option<NodeId> {
    match self.record(node)?.kind {
      NodeKind::Anchor => self
        .segment_nodes(node)
        .into_iter()
        .find_map(|inner| self.placement(inner)),
      NodeKind::Scope => self.children(node).find_map(|child| self.placement(child)),
      NodeKind::Box | NodeKind::Control | NodeKind::Run | NodeKind::Overlay => Some(node),
    }
  }

  /// The node the overlay `node` opened against, while one is.
  #[inline(always)]
  #[must_use]
  pub fn overlay_anchor(&self, node: NodeId) -> Option<NodeId> {
    self
      .record(node)
      .and_then(|record| record.overlay)
      .and_then(|overlay| overlay.anchor)
  }

  /// The overlays among `overlays`, open overlays in the order they opened, that a
  /// primary press at `at` dismisses, topmost first; `at` is `None` for a press on no
  /// node.
  ///
  /// From the top down, each overlay the press lands outside of—on neither the overlay
  /// nor, for a popup, the anchor it opened from—is dismissed, until the first it lands
  /// inside of or on the anchor of; a modal stops the descent, dismissed itself by a
  /// press outside it.
  #[must_use]
  pub fn dismissed_by_press(&self, overlays: &[NodeId], at: Option<NodeId>) -> Vec<NodeId> {
    let ancestry = at.map(|at| self.ancestry(at)).unwrap_or_default();
    let mut dismissed = Vec::new();

    for overlay in overlays.iter().rev().copied() {
      let modal = self.is_modal(overlay);
      let on_anchor = !modal
        && self
          .overlay_anchor(overlay)
          .is_some_and(|anchor| ancestry.contains(&anchor));

      if ancestry.contains(&overlay) || on_anchor {
        break;
      }

      dismissed.push(overlay);

      if modal {
        break;
      }
    }

    dismissed
  }

  /// The open overlays in the order they opened: the last stands on top.
  #[inline(always)]
  pub fn open_overlays(&self) -> &[NodeId] {
    &self.open_overlays
  }

  /// Whether `node` is a modal overlay.
  #[inline(always)]
  #[must_use]
  pub fn is_modal(&self, node: NodeId) -> bool {
    self
      .record(node)
      .and_then(|record| record.overlay)
      .is_some_and(|overlay| overlay.modal)
  }

  /// The topmost open modal overlay among `overlays`, open overlays in the order they
  /// opened.
  #[must_use]
  pub fn topmost_modal(&self, overlays: &[NodeId]) -> Option<NodeId> {
    overlays
      .iter()
      .rev()
      .copied()
      .find(|overlay| self.is_modal(*overlay))
  }

  /// Whether `node` stays reachable under the topmost modal among `overlays`, open
  /// overlays in the order they opened: inside that modal, or inside an overlay opened
  /// after it; every node is while none is open.
  #[must_use]
  pub fn reachable_under_modal(&self, overlays: &[NodeId], node: NodeId) -> bool {
    let Some(modal) = overlays.iter().rposition(|overlay| self.is_modal(*overlay))
    else {
      return true;
    };
    let ancestry = self.ancestry(node);

    overlays[modal..]
      .iter()
      .any(|overlay| ancestry.contains(overlay))
  }

  /// The tab stops under `root` in traversal order, with `overlays` the open overlays
  /// in the order they opened: positioned ones first by position, then the rest in
  /// document order; closed overlays and the subtrees `hidden` answers for skipped, and
  /// the topmost modal containing the order with the overlays opened after it.
  #[must_use]
  pub fn tab_order(
    &self,
    root: NodeId,
    overlays: &[NodeId],
    hidden: impl Fn(NodeId) -> bool,
  ) -> Vec<NodeId> {
    let mut positioned = Vec::new();
    let mut ordered = Vec::new();
    let mut stack = vec![root];

    while let Some(current) = stack.pop() {
      let Some(record) = self.record(current)
      else {
        continue;
      };

      if record.overlay.is_some_and(|overlay| !overlay.open) || hidden(current) {
        continue;
      }

      match record.focus_order {
        Some(FocusOrder::Position(position)) => positioned.push((position, current)),
        Some(FocusOrder::Auto) => ordered.push(current),
        Some(FocusOrder::Skip) | None => {}
      }

      stack.extend(self.children_reversed(current));
    }

    positioned.sort_by_key(|(position, _)| *position);

    let mut order: Vec<NodeId> = positioned.into_iter().map(|(_, node)| node).collect();

    order.extend(ordered);
    order.retain(|node| self.reachable_under_modal(overlays, *node));

    order
  }

  /// The stop `step` places from `from` along `order`, the tab stops under `root` in
  /// traversal order, wrapping at both ends.
  ///
  /// From a node that is no stop, such as one disabled while it held focus, the nearest
  /// stop after it in document order is taken, or before it for a negative `step`; from
  /// no node, the first stop, or the last for a negative `step`.
  #[must_use]
  pub fn tab_from(
    &self,
    root: NodeId,
    order: &[NodeId],
    from: Option<NodeId>,
    step: isize,
  ) -> Option<NodeId> {
    let (first, last) = (*order.first()?, *order.last()?);

    if let Some(index) = from.and_then(|from| order.iter().position(|stop| *stop == from)) {
      let count = isize::try_from(order.len()).ok()?;
      let index = isize::try_from(index).ok()?;

      return usize::try_from((index + step).rem_euclid(count))
        .ok()
        .and_then(|index| order.get(index).copied());
    }

    let positions = self.document_positions(root);
    let position = |node: NodeId| positions.get(&node).copied();
    let at = from.and_then(position);
    let beside = at.and_then(|at| {
      if step >= 0 {
        order
          .iter()
          .copied()
          .find(|stop| position(*stop).is_some_and(|stop| stop > at))
      }
      else {
        order
          .iter()
          .rev()
          .copied()
          .find(|stop| position(*stop).is_some_and(|stop| stop < at))
      }
    });

    Some(beside.unwrap_or(if step >= 0 { first } else { last }))
  }

  /// Every node under `root` with its position in document order.
  fn document_positions(&self, root: NodeId) -> FxHashMap<NodeId, usize> {
    let mut positions = FxHashMap::default();
    let mut stack = vec![root];

    while let Some(current) = stack.pop() {
      if !self.contains(current) {
        continue;
      }

      positions.insert(current, positions.len());
      stack.extend(self.children_reversed(current));
    }

    positions
  }

  /// Writes the state of the control `node` and mirrors it; a node of any other kind
  /// is left alone.
  pub fn set_control_state(&mut self, node: NodeId, state: ControlState) {
    let Some(record) = self.record_mut(node)
    else {
      return;
    };

    if record.kind != NodeKind::Control {
      return;
    }

    record.control = Some(state);
    self.mirrors.push(Mirror::ControlState { node, state });
  }

  /// The state of the control `node`; `None` for a node of any other kind.
  #[inline(always)]
  #[must_use]
  pub fn control_state(&self, node: NodeId) -> Option<ControlState> {
    self.record(node).and_then(NodeRecord::control_state)
  }

  /// The nearest control at or above `node`.
  #[must_use]
  pub fn enclosing_control(&self, node: NodeId) -> Option<NodeId> {
    let mut current = Some(node);

    while let Some(visited) = current {
      let record = self.record(visited)?;

      if record.kind == NodeKind::Control {
        return Some(visited);
      }

      current = record.parent;
    }

    None
  }

  /// Records `node` as `part` of the control enclosing it.
  #[inline(always)]
  pub fn set_part(&mut self, node: NodeId, part: Part) {
    if let Some(record) = self.record_mut(node) {
      record.part = Some(part);
    }
  }

  /// The first node under the control `control`, in tree order, recorded as `part`;
  /// the controls inside it are passed over with their parts.
  #[must_use]
  pub fn find_part(&self, control: NodeId, part: Part) -> Option<NodeId> {
    let mut stack: Vec<NodeId> = self.children_reversed(control).collect();

    while let Some(node) = stack.pop() {
      let Some(record) = self.record(node)
      else {
        continue;
      };

      if record.part == Some(part) {
        return Some(node);
      }

      if record.kind == NodeKind::Control {
        continue;
      }

      stack.extend(self.children_reversed(node));
    }

    None
  }

  /// The part of its enclosing control `node` is recorded as.
  #[inline(always)]
  #[must_use]
  pub fn part(&self, node: NodeId) -> Option<Part> {
    self.record(node).and_then(NodeRecord::part)
  }

  /// Adds `handler` to `node`: it runs as the current owner when an event of its kind
  /// reaches the node, after the node's earlier handlers of that kind.
  pub fn handle(&mut self, node: NodeId, handler: Handler) {
    let Some(record) = self.record_mut(node)
    else {
      return;
    };
    let kind = handler.kind();

    record.handlers.get_or_insert_default().push(handler);
    self.mirrors.push(Mirror::Listen { node, kind });
  }

  /// The nearest node at or above `node` offering drag data, with what builds it.
  #[must_use]
  pub fn drag_source(&self, node: NodeId) -> Option<(NodeId, Registered<DragSource>)> {
    self.ancestry(node).into_iter().find_map(|at| {
      let source = self.handlers(at)?.select(pick_drag_source).last()?;

      Some((at, source))
    })
  }

  /// The nodes at or above `node` taking drops, innermost first, each with its drop
  /// handlers.
  #[must_use]
  pub fn drop_candidates(&self, node: NodeId) -> Vec<(NodeId, Vec<Registered<DropHandler>>)> {
    self
      .ancestry(node)
      .into_iter()
      .filter_map(|at| {
        let handlers = self.drop_handlers(at);

        (!handlers.is_empty()).then_some((at, handlers))
      })
      .collect()
  }

  /// `node`'s drop handlers.
  #[must_use]
  pub fn drop_handlers(&self, node: NodeId) -> Vec<Registered<DropHandler>> {
    self
      .handlers(node)
      .map_or_else(Vec::new, |handlers| handlers.select(pick_drop).collect())
  }

  /// The node the drag in progress started from.
  #[inline(always)]
  #[must_use]
  pub fn dragged(&self) -> Option<NodeId> {
    self.drag.as_ref().map(|drag| drag.source)
  }

  /// What the drag in progress carries.
  #[inline(always)]
  #[must_use]
  pub fn drag_data(&self) -> Option<&DragData> {
    self.drag.as_ref().map(|drag| &drag.data)
  }

  /// The node taking the drag in progress.
  #[inline(always)]
  #[must_use]
  pub fn drag_target(&self) -> Option<NodeId> {
    self.drag.as_ref().and_then(|drag| drag.target)
  }

  /// Starts a drag from `source` carrying `data`.
  pub(crate) fn begin_drag(&mut self, source: NodeId, data: DragData) {
    self.drag = Some(DragRecord {
      source,
      data,
      target: None,
    });
  }

  /// Makes `target` the node taking the drag in progress.
  pub(crate) const fn set_drag_target(&mut self, target: Option<NodeId>) {
    if let Some(drag) = &mut self.drag {
      drag.target = target;
    }
  }

  /// Ends the drag in progress.
  pub(crate) fn finish_drag(&mut self) {
    self.drag = None;
  }

  /// Notes `access` as what the view that made `node` reports, without a write: for
  /// a report the platform received as the node was created.
  pub fn note_access(&mut self, node: NodeId, access: Vec<AccessProp>) {
    if let Some(record) = self.record_mut(node) {
      record.access.get_or_insert_default().own = access;
    }
  }

  /// Replaces what the view that made `node` reports with `access`, returning the
  /// whole report the platform is to receive: the node's own, then its description,
  /// with their relations folded ([`AccessProp::fold`]).
  #[must_use]
  pub fn report_access(&mut self, node: NodeId, access: Vec<AccessProp>) -> Vec<AccessProp> {
    let Some(record) = self.record_mut(node)
    else {
      return access;
    };
    let reports = record.access.get_or_insert_default();

    reports.own = access;

    reports.merged()
  }

  /// Replaces what a view describing `node` reports with `description`, returning the
  /// whole report the platform is to receive: the node's own, then its description,
  /// with their relations folded ([`AccessProp::fold`]).
  #[must_use]
  pub fn describe(&mut self, node: NodeId, description: Vec<AccessProp>) -> Vec<AccessProp> {
    let Some(record) = self.record_mut(node)
    else {
      return description;
    };
    let reports = record.access.get_or_insert_default();

    reports.description = description;

    reports.merged()
  }

  /// The handlers `node` carries.
  #[inline(always)]
  #[must_use]
  pub(crate) fn handlers(&self, node: NodeId) -> Option<&HandlerSet> {
    self.record(node).and_then(NodeRecord::handlers)
  }

  /// The parent the segment's content sits under (`None` while detached) and its end.
  fn segment_place(&self, start: NodeId) -> Option<(Option<NodeId>, NodeId)> {
    let record = self.record(start)?;
    let segment = record.segment.as_ref()?;

    Some((record.parent, segment.end))
  }

  /// Takes the record of `node` out of the arena, retiring the slot's generation; a
  /// slot whose generations are exhausted is retired whole, so a stale id never names a
  /// live node.
  fn remove(&mut self, node: NodeId) -> Option<NodeRecord<P>> {
    let entry = self
      .entries
      .get_mut(node.index() as usize)
      .filter(|entry| entry.generation == node.generation())?;
    let record = entry.record.take()?;

    if let Some(generation) = entry.generation.checked_add(1) {
      entry.generation = generation;
      self.free.push(node.index());
    }

    Some(record)
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
    self.link(parent, child, before, Arrival::Attach);

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

      self.link(parent, end, before, Arrival::Attach);
    }

    self.return_kept_focus(child);
    self.definitions_generation += 1;
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
      if let Some(from) = self.parent(node) {
        self.sever(from, node);
      }

      self.link(parent, node, before, Arrival::Relocate);
    }

    self.definitions_generation += 1;
  }

  /// Takes `child`, standing under a parent, out of the tree live, keeping the focus
  /// held inside it for its return.
  fn hide(&mut self, child: NodeId) {
    if let Some(focused) = self.focused
      && self.ancestry(focused).contains(&child)
    {
      self.focused = None;
      self.kept_focus.insert(child, focused);
    }

    self.unlink(child, Departure::Hide);

    if let Some(record) = self.record_mut(child) {
      record.hidden = true;
    }

    self.definitions_generation += 1;
  }

  /// Gives the focus `child` kept when it left back to its node, where that node is live
  /// and focus has gone nowhere since.
  fn return_kept_focus(&mut self, child: NodeId) {
    if let Some(focused) = self.kept_focus.remove(&child)
      && self.focused.is_none()
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
  /// its arrival; `child` must be detached.
  fn link(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>, arrival: Arrival) {
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

    self.mirrors.push(match arrival {
      Arrival::Attach => Mirror::Attach {
        parent,
        child,
        before,
      },
      Arrival::Relocate => Mirror::Relocate {
        parent,
        child,
        before,
      },
    });
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
      self.mirrors.push(match departure {
        Departure::Hide => Mirror::Hide { parent, child },
        Departure::Remove => Mirror::Remove { parent, child },
      });
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

/// A drag source, out of a handler.
fn pick_drag_source(handler: &Handler) -> Option<&Rc<DragSource>> {
  match handler {
    Handler::DragStart(run) => Some(run),
    _ => None,
  }
}

/// A drop handler, out of a handler.
fn pick_drop(handler: &Handler) -> Option<&Rc<DropHandler>> {
  match handler {
    Handler::Drop(run) => Some(run),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::{NodeKind, Scene};
  use crate::{
    Color,
    Definition,
    Definitions,
    FocusOrder,
    NodeId,
    TokenName,
    testing::{Call, Erased, TestBox, TestPlatform, children},
  };

  fn with<R>(operation: impl FnOnce(&mut Scene<TestPlatform>) -> R) -> R {
    Scene::<TestPlatform>::with(operation)
  }

  fn color(value: f32) -> Erased {
    Erased::Color(Definition::from(Color::srgb(value, value, value)))
  }

  fn definitions(name: &'static str, value: Erased) -> Definitions<Erased> {
    Definitions::from(vec![(TokenName::new(name), value)])
  }

  /// Attaching to a destroyed parent, or attaching a destroyed child, changes nothing:
  /// the child stays where it stood.
  #[test]
  fn attach_needs_both_nodes_live() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let gone = TestBox::node();
    let child = TestBox::node();

    with(|scene| {
      scene.attach(parent, child, None);
      scene.destroy(gone);
    });
    TestPlatform::take_log();
    with(|scene| {
      scene.attach(gone, child, None);
      scene.attach(parent, gone, None);
    });

    assert_eq!(with(|scene| scene.parent(child)), Some(parent));
    assert_eq!(children(parent), [child]);
    assert_eq!(TestPlatform::take_log(), []);
  }

  /// A slot whose generations are exhausted is never reused, so its last id stays
  /// stale.
  #[test]
  fn an_exhausted_slot_is_retired() {
    TestPlatform::reset();

    let node = TestBox::node();

    with(|scene| {
      scene.entries[node.index() as usize].generation = u32::MAX;
    });

    let last = super::NodeId::new(node.index(), u32::MAX);

    with(|scene| scene.destroy(last));

    let next = TestBox::node();

    assert_ne!(next.index(), node.index());
    assert!(with(|scene| scene.record(last).is_none()));
  }

  /// One name is one kind: redefining a token as another kind panics in a debug build,
  /// where redefining it as the same kind does not.
  #[test]
  #[cfg(debug_assertions)]
  #[should_panic(expected = "the token `ink` is defined as two kinds")]
  fn a_token_is_one_kind() {
    TestPlatform::reset();

    with(|scene| {
      scene.define_root_tokens(definitions("ink", color(0.1)));
      scene.define_root_tokens(definitions("ink", color(0.9)));
      scene.define_root_tokens(definitions("ink", Erased::Number(Definition::from(1.))));
    });
  }

  /// Attaching an attached node moves it from its previous parent, in the scene and in
  /// the platform.
  #[test]
  fn attach_unlinks_the_previous_parent() {
    TestPlatform::reset();

    let first = TestBox::node();
    let second = TestBox::node();
    let child = TestBox::node();

    with(|scene| scene.attach(first, child, None));
    TestPlatform::take_log();
    with(|scene| scene.attach(second, child, None));

    assert_eq!(with(|scene| scene.parent(child)), Some(second));
    assert_eq!(children(first), []);
    assert_eq!(children(second), [child]);
    assert_eq!(
      TestPlatform::take_log(),
      [Call::Relocate(second, child, None)]
    );
  }

  /// `before` places a child ahead of a sibling, in the scene and in the platform.
  #[test]
  fn attach_before_orders_the_children() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let first = TestBox::node();
    let second = TestBox::node();

    with(|scene| {
      scene.attach(parent, second, None);
      scene.attach(parent, first, Some(second));
    });

    assert_eq!(children(parent), [first, second]);
    assert_eq!(
      TestPlatform::take_log(),
      [
        Call::Attach(parent, second, None),
        Call::Attach(parent, first, Some(second)),
      ]
    );
  }

  /// Attaching a node where it already stands tells the platform nothing, and moving it
  /// does.
  #[test]
  fn attach_in_place_is_a_no_op() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let first = TestBox::node();
    let second = TestBox::node();

    with(|scene| {
      scene.attach(parent, first, None);
      scene.attach(parent, second, None);
    });
    TestPlatform::take_log();
    with(|scene| {
      scene.attach(parent, first, Some(second));
      scene.attach(parent, second, None);
    });

    assert_eq!(TestPlatform::take_log(), []);

    with(|scene| scene.attach(parent, first, None));

    assert_eq!(children(parent), [second, first]);
    assert_eq!(
      TestPlatform::take_log(),
      [Call::Relocate(parent, first, None)]
    );
  }

  /// Detaching a node hides it, no root while hidden, and attaching it again gives it
  /// back from nowhere.
  #[test]
  fn detach_hides_and_attach_gives_back() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let child = TestBox::node();

    with(|scene| scene.attach(parent, child, None));
    TestPlatform::take_log();
    with(|scene| scene.detach(parent, child));

    assert_eq!(TestPlatform::take_log(), [Call::Hide(parent, child)]);
    assert!(
      !with(|scene| scene.roots().any(|root| root == child)),
      "a hidden node is no root"
    );

    with(|scene| scene.attach(parent, child, None));

    assert_eq!(
      TestPlatform::take_log(),
      [Call::Attach(parent, child, None)]
    );
  }

  /// Moving a segment relocates its start, its content and its end as one run, in
  /// order.
  #[test]
  fn moving_a_segment_relocates_its_run_in_order() {
    TestPlatform::reset();

    let first = TestBox::node();
    let second = TestBox::node();
    let (start, end) = Scene::<TestPlatform>::create_segment();
    let content = [TestBox::node(), TestBox::node()];

    with(|scene| {
      scene.attach(first, start, None);

      for node in content {
        scene.insert_in_segment(start, node, None);
      }
    });
    TestPlatform::take_log();
    with(|scene| scene.attach(second, start, None));

    assert_eq!(children(first), []);
    assert_eq!(children(second), [start, content[0], content[1], end]);
    assert_eq!(
      TestPlatform::take_log(),
      [
        Call::Relocate(second, start, None),
        Call::Relocate(second, content[0], None),
        Call::Relocate(second, content[1], None),
        Call::Relocate(second, end, None),
      ]
    );
  }

  /// Hiding a subtree holding focus clears it, and attaching the subtree again gives it
  /// back where focus has gone nowhere since.
  #[test]
  fn a_hidden_subtree_keeps_its_focus_for_its_return() {
    TestPlatform::reset();

    let root = TestBox::node();
    let panel = TestBox::node();
    let field = TestBox::node();
    let other = TestBox::node();

    with(|scene| {
      scene.attach(root, panel, None);
      scene.attach(panel, field, None);
      scene.attach(root, other, None);
      let _ = scene.move_focus(Some(field));
      scene.detach(root, panel);
    });

    assert_eq!(with(|scene| scene.focused()), None);

    TestPlatform::take_log();
    with(|scene| scene.attach(root, panel, None));

    assert_eq!(
      TestPlatform::take_log(),
      [Call::Attach(root, panel, None), Call::Focus(field)]
    );

    with(|scene| {
      let _ = scene.move_focus(Some(field));
      scene.detach(root, panel);
      let _ = scene.move_focus(Some(other));
    });
    TestPlatform::take_log();
    with(|scene| scene.attach(root, panel, None));

    assert_eq!(TestPlatform::take_log(), [Call::Attach(root, panel, None)]);
  }

  /// Destroying a node takes its whole subtree, releases every record on the platform,
  /// and retires every id.
  #[test]
  fn destroy_takes_the_subtree_and_releases_every_record() {
    TestPlatform::reset();

    let root = TestBox::node();
    let parent = TestBox::node();
    let child = TestBox::node();
    let grandchild = TestBox::node();
    let nodes = [parent, child, grandchild];

    with(|scene| {
      scene.attach(root, parent, None);
      scene.attach(parent, child, None);
      scene.attach(child, grandchild, None);
    });
    TestPlatform::take_log();
    with(|scene| scene.destroy(parent));

    let log = TestPlatform::take_log();
    let mut destroyed: Vec<NodeId> = log
      .iter()
      .filter_map(|call| match call {
        Call::Destroy(node) => Some(*node),
        _ => None,
      })
      .collect();

    destroyed.sort_unstable();

    assert_eq!(log[0], Call::Remove(root, nodes[0]));
    assert_eq!(destroyed, nodes);
    assert_eq!(children(root), []);
    assert!(with(|scene| scene.record(child).is_none()));

    let reused = TestBox::node();

    assert_eq!(reused.index(), grandchild.index());
    assert_ne!(reused, grandchild);
    assert!(with(|scene| scene.record(grandchild).is_none()));
  }

  /// A token resolves to the nearest defining ancestor, then to the root; the answer
  /// is cached and a redefinition invalidates it.
  #[test]
  fn tokens_resolve_to_the_nearest_definer_and_cache() {
    TestPlatform::reset();

    let card = TokenName::new("card");
    let outer = TestBox::node();
    let inner = TestBox::node();
    let leaf = TestBox::node();
    let stranger = TestBox::node();

    with(|scene| {
      scene.attach(outer, inner, None);
      scene.attach(inner, leaf, None);
    });
    TestPlatform::take_log();
    with(|scene| {
      scene.define_root_tokens(definitions("card", color(0.)));
      scene.define_tokens(outer, definitions("card", color(0.5)));
    });

    assert_eq!(
      TestPlatform::take_log(),
      [Call::DefineRootTokens(1), Call::DefineTokens(outer, 1)]
    );
    assert_eq!(
      with(|scene| scene.resolve_token(leaf, card)),
      Some(color(0.5))
    );
    assert_eq!(
      with(|scene| scene.resolve_token(stranger, card)),
      Some(color(0.))
    );
    assert_eq!(
      with(|scene| scene.resolve_token(leaf, TokenName::new("missing"))),
      None
    );

    let generation = with(|scene| scene.definitions_generation);

    assert_eq!(
      with(|scene| {
        let cache = scene.record(leaf).unwrap().cache.as_ref().unwrap();

        (cache.generation, cache.values.get(&card).cloned())
      }),
      (generation, Some(Some((Some(outer), color(0.5)))))
    );
    assert_eq!(
      with(|scene| scene.resolve_scoped_token(stranger, card)),
      Some((None, color(0.)))
    );

    with(|scene| scene.define_tokens(inner, definitions("card", color(1.))));

    assert_eq!(
      with(|scene| scene.resolve_token(leaf, card)),
      Some(color(1.))
    );
    assert_eq!(
      with(|scene| scene.resolve_token(inner, card)),
      Some(color(1.))
    );
    assert_eq!(
      with(|scene| scene.resolve_token(outer, card)),
      Some(color(0.5))
    );

    with(|scene| scene.detach(inner, leaf));

    assert_eq!(
      with(|scene| scene.resolve_token(leaf, card)),
      Some(color(0.))
    );
  }

  /// A node destroyed while it waits in a detached segment leaves the segment.
  #[test]
  fn a_destroyed_node_leaves_the_detached_segment_it_waits_in() {
    TestPlatform::reset();

    let (start, _) = Scene::<TestPlatform>::create_segment();
    let first = TestBox::node();
    let second = TestBox::node();

    with(|scene| {
      scene.insert_in_segment(start, first, None);
      scene.insert_in_segment(start, second, None);
      scene.destroy(first);
    });

    assert_eq!(with(|scene| scene.segment_nodes(start)), [second]);
  }

  /// An overlay anchored on a segment or a scope is placed against the first node
  /// inside it with a box of its own, an empty segment standing for none.
  #[test]
  fn a_segment_is_placed_against_its_first_box() {
    TestPlatform::reset();

    let (outer, _) = Scene::<TestPlatform>::create_segment();
    let (empty, _) = Scene::<TestPlatform>::create_segment();
    let scope = with(|scene| scene.create(NodeKind::Scope));
    let inside = TestBox::node();
    let after = TestBox::node();

    with(|scene| {
      scene.attach(scope, inside, None);
      scene.insert_in_segment(outer, empty, None);
      scene.insert_in_segment(outer, scope, None);
      scene.insert_in_segment(outer, after, None);
    });

    assert_eq!(with(|scene| scene.placement(outer)), Some(inside));
    assert_eq!(with(|scene| scene.placement(scope)), Some(inside));
    assert_eq!(with(|scene| scene.placement(empty)), None);
    assert_eq!(with(|scene| scene.placement(after)), Some(after));
  }

  /// The node taking focus for a segment is in the segment's content.
  #[test]
  fn the_focus_holder_of_a_segment_is_in_its_content() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let (start, _) = Scene::<TestPlatform>::create_segment();
    let plain = TestBox::node();
    let focusable = TestBox::node();

    with(|scene| {
      scene.attach(parent, start, None);
      scene.insert_in_segment(start, plain, None);
      scene.insert_in_segment(start, focusable, None);
      scene.note_focus_order(focusable, Some(FocusOrder::Auto));
    });

    assert_eq!(with(|scene| scene.focus_holder_within(start)), focusable);
    assert_eq!(
      with(|scene| scene.enclosing_segment(focusable)),
      Some(start)
    );
  }

  /// A segment attaches and detaches with its content and its end anchor.
  #[test]
  fn segments_travel_with_their_content() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let other = TestBox::node();
    let content = TestBox::node();
    let (start, end) = Scene::<TestPlatform>::create_segment();

    with(|scene| scene.insert_in_segment(start, content, None));

    assert_eq!(with(|scene| scene.segment_nodes(start)), [content]);
    assert_eq!(with(|scene| scene.parent(content)), None);

    with(|scene| scene.attach(parent, start, None));

    assert_eq!(children(parent), [start, content, end]);
    assert_eq!(
      with(|scene| scene.record(start).unwrap().kind()),
      NodeKind::Anchor
    );

    with(|scene| scene.attach(other, start, None));

    assert_eq!(children(parent), []);
    assert_eq!(children(other), [start, content, end]);

    with(|scene| scene.detach(other, start));

    assert_eq!(children(other), []);
    assert_eq!(with(|scene| scene.segment_nodes(start)), [content]);

    TestPlatform::take_log();
    with(|scene| scene.destroy(start));

    let mut destroyed: Vec<NodeId> = TestPlatform::take_log()
      .iter()
      .filter_map(|call| match call {
        Call::Destroy(node) => Some(*node),
        _ => None,
      })
      .collect();

    destroyed.sort_unstable();

    assert_eq!(destroyed.len(), 3);
    assert!(with(|scene| scene.record(content).is_none()));
    assert!(with(|scene| scene.record(end).is_none()));
  }

  /// A segment nested in a detached segment keeps its own content pending, so a
  /// removal from it while detached holds when the outer segment is reattached.
  #[test]
  fn nested_segments_keep_their_own_content_while_detached() {
    TestPlatform::reset();

    let parent = TestBox::node();
    let before = TestBox::node();
    let inner_content = TestBox::node();
    let (outer, outer_end) = Scene::<TestPlatform>::create_segment();
    let (inner, inner_end) = Scene::<TestPlatform>::create_segment();

    with(|scene| {
      scene.attach(parent, outer, None);
      scene.insert_in_segment(outer, before, None);
      scene.insert_in_segment(outer, inner, None);
      scene.insert_in_segment(inner, inner_content, None);
    });

    assert_eq!(
      children(parent),
      [outer, before, inner, inner_content, inner_end, outer_end]
    );
    assert_eq!(with(|scene| scene.segment_nodes(outer)), [before, inner]);

    with(|scene| scene.detach(parent, outer));

    assert_eq!(with(|scene| scene.segment_nodes(outer)), [before, inner]);
    assert_eq!(with(|scene| scene.segment_nodes(inner)), [inner_content]);

    with(|scene| scene.remove_from_segment(inner, inner_content));
    with(|scene| scene.attach(parent, outer, None));

    assert_eq!(
      children(parent),
      [outer, before, inner, inner_end, outer_end]
    );
    assert!(with(|scene| scene.record(inner_content).is_some()));

    with(|scene| scene.insert_in_segment(inner, inner_content, None));

    assert_eq!(
      children(parent),
      [outer, before, inner, inner_content, inner_end, outer_end]
    );
  }

  /// The tab order puts positioned stops first by position, then the rest in document
  /// order, skipping closed overlays and hidden subtrees; the modal opened last contains
  /// it with the overlays opened after it, whatever order they were built in.
  #[test]
  fn the_tab_order_follows_positions_documents_and_the_last_modal() {
    TestPlatform::reset();

    let root = TestBox::node();
    let [first, second, positioned, hidden, closed, earlier, later] =
      std::array::from_fn(|_| TestBox::node());
    let (inside_earlier, inside_later, inside_closed, inside_hidden) = (
      TestBox::node(),
      TestBox::node(),
      TestBox::node(),
      TestBox::node(),
    );

    with(|scene| {
      for node in [first, second, positioned, hidden, closed, earlier, later] {
        scene.attach(root, node, None);
      }

      scene.attach(earlier, inside_earlier, None);
      scene.attach(later, inside_later, None);
      scene.attach(closed, inside_closed, None);
      scene.attach(hidden, inside_hidden, None);

      for node in [
        first,
        second,
        inside_earlier,
        inside_later,
        inside_closed,
        inside_hidden,
      ] {
        scene.note_focus_order(node, Some(FocusOrder::Auto));
      }

      scene.note_focus_order(positioned, Some(FocusOrder::Position(1)));

      for (overlay, modal) in [(closed, false), (earlier, true), (later, true)] {
        scene.note_overlay(overlay, modal);
      }
    });

    let order = || {
      with(|scene| {
        let overlays = scene.open_overlays().to_vec();

        scene.tab_order(root, &overlays, |node| node == hidden)
      })
    };

    assert_eq!(order(), [positioned, first, second]);

    with(|scene| {
      scene.set_overlay_open(later, true, None);
      scene.set_overlay_open(earlier, true, None);
    });

    assert_eq!(order(), [inside_earlier], "the modal opened last wins");

    with(|scene| scene.set_overlay_open(earlier, false, None));

    assert_eq!(order(), [inside_later]);
  }

  /// A press dismisses, from the top down, each overlay it lands outside of, until one
  /// it lands inside of or on the anchor of; a modal ends the descent, dismissed itself.
  #[test]
  fn a_press_dismisses_the_overlays_it_lands_outside_of() {
    TestPlatform::reset();

    let root = TestBox::node();
    let [page, modal, menu, item, submenu] = std::array::from_fn(|_| TestBox::node());

    with(|scene| {
      scene.attach(root, page, None);
      scene.attach(root, modal, None);
      scene.attach(modal, menu, None);
      scene.attach(menu, item, None);
      scene.attach(menu, submenu, None);
      scene.note_overlay(modal, true);
      scene.note_overlay(menu, false);
      scene.note_overlay(submenu, false);
      scene.set_overlay_open(modal, true, None);
      scene.set_overlay_open(menu, true, None);
      scene.set_overlay_open(submenu, true, Some(item));
    });

    let dismissed = |at| {
      with(|scene| {
        let overlays = scene.open_overlays().to_vec();

        scene.dismissed_by_press(&overlays, at)
      })
    };

    assert_eq!(dismissed(Some(item)), [], "on the submenu's anchor");
    assert_eq!(dismissed(Some(menu)), [submenu]);
    assert_eq!(dismissed(Some(page)), [submenu, menu, modal]);
    assert_eq!(dismissed(None), [submenu, menu, modal]);
  }

  /// Focus shows after a key and always on a text entry, a press holds along its
  /// ancestors, and a destroyed node's focus is gone, the scene resolving each node's
  /// interaction from them.
  #[test]
  fn the_scene_holds_focus_press_and_their_rules() {
    TestPlatform::reset();

    let root = TestBox::node();
    let [button, entry] = std::array::from_fn(|_| TestBox::node());

    with(|scene| {
      scene.attach(root, button, None);
      scene.attach(root, entry, None);
      scene.note_entry(entry);
      scene.set_focus_visible(false);

      let _ = scene.move_focus(Some(button));
    });

    assert!(
      with(|scene| !scene.focus_shows(button)),
      "focus a press gave"
    );

    with(|scene| {
      let _ = scene.move_focus(Some(entry));

      scene.set_pressed(Some(entry));
    });

    assert!(
      with(|scene| scene.focus_shows(entry)),
      "an entry's focus shows"
    );
    assert!(with(|scene| scene.interaction(root).pressed));
    assert!(with(|scene| !scene.interaction(button).pressed));

    with(|scene| scene.destroy(entry));

    assert_eq!(with(|scene| scene.focused()), None);
    assert_eq!(with(|scene| scene.pressed()), None);
  }
}
