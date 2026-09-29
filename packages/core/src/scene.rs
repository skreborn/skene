//! The retained tree every view attaches to, owned by core and mirrored into a platform.

use std::{any::TypeId, fmt::Debug, ops::Deref, rc::Rc};

use rustc_hash::{FxHashMap, FxHashSet};
use skene_reactive::{Owner, Position};
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
  ClickHandler,
  Dimension,
  Flexibility,
  GridPlacement,
  Inset,
  ItemAlignment,
  KeyHandler,
  Line,
  PointerHandler,
  ScrollEvent,
  TextSelection,
};

/// A handler taking nothing: pointer enter and leave, focus and blur, dismiss.
pub type PlainHandler = dyn Fn();
/// A scroll handler: the container's new offsets.
pub type ScrollHandler = dyn Fn(&ScrollEvent);
/// A change handler: a control's new textual value.
pub type ChangeHandler = dyn Fn(&str);
/// A toggle handler: a control's new checked state.
pub type ToggleHandler = dyn Fn(bool);
/// A selection handler: a text entry's new selection.
pub type SelectionHandler = dyn Fn(TextSelection);

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

/// A handler with the owner it was registered under, which it runs as: what it
/// creates, spawns or looks up in context belongs to the view that registered it.
#[derive(Debug)]
pub struct Handler<H: ?Sized> {
  owner: Owner,
  run: Rc<H>,
}

impl<H: ?Sized> Handler<H> {
  /// `run`, registered under the current owner.
  fn new(run: Rc<H>) -> Self {
    Self {
      owner: Owner::current(),
      run,
    }
  }

  /// The owner the handler runs as.
  #[inline(always)]
  pub const fn owner(&self) -> Owner {
    self.owner
  }
}

impl<H: ?Sized> Clone for Handler<H> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      owner: self.owner,
      run: Rc::clone(&self.run),
    }
  }
}

impl<H: ?Sized> Deref for Handler<H> {
  type Target = H;

  #[inline(always)]
  fn deref(&self) -> &H {
    &self.run
  }
}

/// The handlers a node carries, by event.
///
/// Every handler on a node runs when its event reaches the node; bubbling is between
/// nodes. A control's behavior registers its click as *acting*, apart from the
/// observers, so a platform whose native element performed the action can dispatch the
/// observers alone.
#[derive(Default)]
#[must_use]
pub struct HandlerSet {
  acting_click: Vec<Handler<ClickHandler>>,
  click: Vec<Handler<ClickHandler>>,
  key_down: Vec<Handler<KeyHandler>>,
  key_up: Vec<Handler<KeyHandler>>,
  pointer_down: Vec<Handler<PointerHandler>>,
  pointer_up: Vec<Handler<PointerHandler>>,
  pointer_move: Vec<Handler<PointerHandler>>,
  pointer_enter: Vec<Handler<PlainHandler>>,
  pointer_leave: Vec<Handler<PlainHandler>>,
  focus: Vec<Handler<PlainHandler>>,
  blur: Vec<Handler<PlainHandler>>,
  scroll: Vec<Handler<ScrollHandler>>,
  change: Vec<Handler<ChangeHandler>>,
  selection_change: Vec<Handler<SelectionHandler>>,
  toggle: Vec<Handler<ToggleHandler>>,
  dismiss: Vec<Handler<PlainHandler>>,
  context_menu: Vec<Handler<PointerHandler>>,
}

impl std::fmt::Debug for HandlerSet {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("HandlerSet").finish_non_exhaustive()
  }
}

impl HandlerSet {
  /// The acting click handlers, in registration order: a control's own response to a
  /// click, run before the observers.
  #[inline(always)]
  #[must_use]
  pub fn acting_click(&self) -> &[Handler<ClickHandler>] {
    &self.acting_click
  }

  /// The click handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn click(&self) -> &[Handler<ClickHandler>] {
    &self.click
  }

  /// The key-down handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn key_down(&self) -> &[Handler<KeyHandler>] {
    &self.key_down
  }

  /// The key-up handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn key_up(&self) -> &[Handler<KeyHandler>] {
    &self.key_up
  }

  /// The pointer-down handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_down(&self) -> &[Handler<PointerHandler>] {
    &self.pointer_down
  }

  /// The pointer-up handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_up(&self) -> &[Handler<PointerHandler>] {
    &self.pointer_up
  }

  /// The pointer-move handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_move(&self) -> &[Handler<PointerHandler>] {
    &self.pointer_move
  }

  /// The pointer-enter handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_enter(&self) -> &[Handler<PlainHandler>] {
    &self.pointer_enter
  }

  /// The pointer-leave handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_leave(&self) -> &[Handler<PlainHandler>] {
    &self.pointer_leave
  }

  /// The focus handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn focus(&self) -> &[Handler<PlainHandler>] {
    &self.focus
  }

  /// The blur handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn blur(&self) -> &[Handler<PlainHandler>] {
    &self.blur
  }

  /// The scroll handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn scroll(&self) -> &[Handler<ScrollHandler>] {
    &self.scroll
  }

  /// The change handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn change(&self) -> &[Handler<ChangeHandler>] {
    &self.change
  }

  /// The selection handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn selection_change(&self) -> &[Handler<SelectionHandler>] {
    &self.selection_change
  }

  /// The toggle handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn toggle(&self) -> &[Handler<ToggleHandler>] {
    &self.toggle
  }

  /// The dismiss handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn dismiss(&self) -> &[Handler<PlainHandler>] {
    &self.dismiss
  }

  /// The context menu handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn context_menu(&self) -> &[Handler<PointerHandler>] {
    &self.context_menu
  }
}

/// The end of a segment and, while the segment is detached, the content waiting
/// between its anchors.
struct Segment {
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
  children: SmallVec<[NodeId; 4]>,
  kind: NodeKind,
  annotation: Option<Annotation<P>>,
  definitions: Option<Box<Definitions<P::TokenValue>>>,
  container: Option<ContainerName>,
  handlers: Option<Box<HandlerSet>>,
  sink: P::Sink,
  segment: Option<Box<Segment>>,
  cache: Option<Box<TokenCache<P::TokenValue>>>,
  control: Option<ControlState>,
  part: Option<Part>,
  position: Position,
  access: Option<Box<AccessReports>>,
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

  /// The nodes attached under this one, in order.
  #[inline(always)]
  pub fn children(&self) -> &[NodeId] {
    &self.children
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
  pub fn handlers(&self) -> Option<&HandlerSet> {
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

  /// The platform's handle for this node.
  #[inline(always)]
  #[must_use]
  pub const fn sink(&self) -> &P::Sink {
    &self.sink
  }

  /// The platform's handle for this node, mutably.
  #[inline(always)]
  #[must_use]
  pub const fn sink_mut(&mut self) -> &mut P::Sink {
    &mut self.sink
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
    parent: P::Sink,
    child: P::Sink,
    before: Option<P::Sink>,
  },
  Detach {
    parent: P::Sink,
    child: P::Sink,
  },
  Destroy(P::Sink),
  DefineTokens {
    scope: P::Sink,
    definitions: Definitions<P::TokenValue>,
  },
  DefineRootTokens(Definitions<P::TokenValue>),
  ControlState {
    sink: P::Sink,
    state: ControlState,
  },
  BindNode {
    sink: P::Sink,
    node: NodeId,
  },
  Listen {
    sink: P::Sink,
    kind: EventKind,
  },
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
      Self::Detach { parent, child } => P::detach(parent, child),
      Self::Destroy(sink) => P::destroy(sink),
      Self::DefineTokens { scope, definitions } => P::define_tokens(scope, &definitions),
      Self::DefineRootTokens(definitions) => P::define_root_tokens(&definitions),
      Self::ControlState { sink, state } => P::set_control_state(sink, &state),
      Self::BindNode { sink, node } => P::bind_node(sink, node),
      Self::Listen { sink, kind } => P::listen(sink, kind),
    }
  }
}

/// The retained tree: a generational arena of [`NodeRecord`]s, every mutation mirrored
/// into the platform's sink.
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

  /// The platform's handle for `node`, cloned out, if the node is live.
  ///
  /// Copy it out and release the scene before calling the platform with it.
  #[inline(always)]
  #[must_use]
  pub fn sink(&self, node: NodeId) -> Option<P::Sink> {
    self.record(node).map(|record| record.sink)
  }

  /// Mints a detached node of `kind` owning `sink`; a control starts in the default
  /// [`ControlState`].
  ///
  /// # Panics
  ///
  /// Panics past 2^32 live nodes.
  pub fn create(&mut self, kind: NodeKind, sink: P::Sink) -> NodeId {
    let is_control = kind == NodeKind::Control;
    let position = Position::next();
    let record = NodeRecord {
      parent: None,
      children: SmallVec::new(),
      kind,
      annotation: None,
      definitions: None,
      container: None,
      handlers: None,
      sink,
      segment: None,
      cache: None,
      control: is_control.then_some(ControlState::DEFAULT),
      part: is_control.then_some(Part::Root),
      position,
      access: None,
    };

    let id = if let Some(index) = self.free.pop() {
      let entry = &mut self.entries[index as usize];

      entry.record = Some(record);

      NodeId::new(index, entry.generation)
    }
    else {
      let index = u32::try_from(self.entries.len()).expect("a scene holds at most 2^32 nodes");

      self.entries.push(Entry {
        generation: 0,
        record: Some(record),
      });

      NodeId::new(index, 0)
    };

    if let Some(record) = self.record(id) {
      let sink = record.sink;

      self.mirrors.push(Mirror::BindNode { sink, node: id });
    }

    id
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
  /// with whatever is mounted between them. The platform makes the anchors before the
  /// scene is borrowed.
  ///
  /// Returns `(start, end)`; the start stands for the segment everywhere.
  pub fn create_segment() -> (NodeId, NodeId) {
    let (start, end) = (P::create_anchor(), P::create_anchor());

    Self::with(|scene| {
      let start = scene.create(NodeKind::Anchor, start);
      let end = scene.create(NodeKind::Anchor, end);

      if let Some(record) = scene.record_mut(start) {
        record.segment = Some(Box::new(Segment {
          end,
          pending: Vec::new(),
        }));
      }

      (start, end)
    })
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

  /// The children of `node`, in order; empty for a leaf or a stale id.
  #[inline(always)]
  pub fn children(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    self
      .record(node)
      .map(NodeRecord::children)
      .unwrap_or_default()
      .iter()
      .copied()
  }

  /// Calls `f` with each child of `node`, in order.
  #[inline(always)]
  pub fn for_each_child(&self, node: NodeId, mut visit: impl FnMut(NodeId)) {
    for child in self.children(node) {
      visit(child);
    }
  }

  /// Places `child` under `parent`, immediately before `before` when given, else last,
  /// unlinking it from any previous parent first; nothing where it already stands there,
  /// or where either is gone.
  ///
  /// A segment start brings its content and its end anchor along.
  pub fn attach(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    if self.record(parent).is_none()
      || self.record(child).is_none()
      || self.stands_before(parent, child, before)
    {
      return;
    }

    self.unlink(child);
    self.link(parent, child, before);

    let segment = self
      .record_mut(child)
      .and_then(|record| record.segment.as_mut())
      .map(|segment| (segment.end, std::mem::take(&mut segment.pending)));

    if let Some((end, pending)) = segment {
      for node in pending {
        self.attach(parent, node, before);
      }

      self.link(parent, end, before);
    }

    self.definitions_generation += 1;
  }

  /// Removes `child` from under `parent`, leaving it detached but live.
  ///
  /// A segment start takes its content and its end anchor with it.
  pub fn detach(&mut self, parent: NodeId, child: NodeId) {
    if self.parent(child) != Some(parent) {
      return;
    }

    self.unlink(child);
    self.definitions_generation += 1;
  }

  /// Destroys `node` and its whole subtree, detaching it first if attached; every
  /// sink is handed to the platform.
  pub fn destroy(&mut self, node: NodeId) {
    self.unlink(node);

    let mut stack = vec![node];

    while let Some(node) = stack.pop() {
      let Some(record) = self.remove(node)
      else {
        continue;
      };

      stack.extend(record.children);

      if let Some(segment) = record.segment {
        stack.extend(segment.pending);
        stack.push(segment.end);
      }

      if let Some(handlers) = record.handlers {
        self.graveyard.push(*handlers);
      }

      self.mirrors.push(Mirror::Destroy(record.sink));
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
      self.attach(parent, node, before.or(Some(end)));

      return;
    }

    self.unlink(node);

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

  /// Takes `node` out of the segment `start` begins, leaving it detached but live.
  pub fn remove_from_segment(&mut self, start: NodeId, node: NodeId) {
    let Some((parent, _)) = self.segment_place(start)
    else {
      return;
    };

    if let Some(parent) = parent {
      self.detach(parent, node);

      return;
    }

    if let Some(segment) = self
      .record_mut(start)
      .and_then(|record| record.segment.as_mut())
    {
      segment.pending.retain(|pending| *pending != node);
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
      self.remove_from_segment(start, node);
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

    let scope = record.sink;

    record.definitions = Some(Box::new(definitions.clone()));
    self
      .mirrors
      .push(Mirror::DefineTokens { scope, definitions });
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

    let sink = record.sink;

    self.mirrors.push(Mirror::ControlState { sink, state });
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

  /// The part of its enclosing control `node` is recorded as.
  #[inline(always)]
  #[must_use]
  pub fn part(&self, node: NodeId) -> Option<Part> {
    self.record(node).and_then(NodeRecord::part)
  }

  /// Adds a click handler to `node`.
  pub fn handle_click(&mut self, node: NodeId, handler: Rc<ClickHandler>) {
    self.register(node, EventKind::Click, |set| {
      set.click.push(Handler::new(handler));
    });
  }

  /// Adds an acting click handler to `node`: a control's own response, run before the
  /// node's other click handlers and skipped by an observed dispatch.
  pub fn handle_click_acting(&mut self, node: NodeId, handler: Rc<ClickHandler>) {
    self.register(node, EventKind::Click, |set| {
      set.acting_click.push(Handler::new(handler));
    });
  }

  /// Adds a key-down handler to `node`.
  pub fn handle_key_down(&mut self, node: NodeId, handler: Rc<KeyHandler>) {
    self.register(node, EventKind::KeyDown, |set| {
      set.key_down.push(Handler::new(handler));
    });
  }

  /// Adds a key-up handler to `node`.
  pub fn handle_key_up(&mut self, node: NodeId, handler: Rc<KeyHandler>) {
    self.register(node, EventKind::KeyUp, |set| {
      set.key_up.push(Handler::new(handler));
    });
  }

  /// Adds a pointer-down handler to `node`.
  pub fn handle_pointer_down(&mut self, node: NodeId, handler: Rc<PointerHandler>) {
    self.register(node, EventKind::PointerDown, |set| {
      set.pointer_down.push(Handler::new(handler));
    });
  }

  /// Adds a pointer-up handler to `node`.
  pub fn handle_pointer_up(&mut self, node: NodeId, handler: Rc<PointerHandler>) {
    self.register(node, EventKind::PointerUp, |set| {
      set.pointer_up.push(Handler::new(handler));
    });
  }

  /// Adds a pointer-move handler to `node`.
  pub fn handle_pointer_move(&mut self, node: NodeId, handler: Rc<PointerHandler>) {
    self.register(node, EventKind::PointerMove, |set| {
      set.pointer_move.push(Handler::new(handler));
    });
  }

  /// Adds a pointer-enter handler to `node`.
  pub fn handle_pointer_enter(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::PointerEnter, |set| {
      set.pointer_enter.push(Handler::new(handler));
    });
  }

  /// Adds a pointer-leave handler to `node`.
  pub fn handle_pointer_leave(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::PointerLeave, |set| {
      set.pointer_leave.push(Handler::new(handler));
    });
  }

  /// Adds a focus handler to `node`.
  pub fn handle_focus(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::Focus, |set| {
      set.focus.push(Handler::new(handler));
    });
  }

  /// Adds a blur handler to `node`.
  pub fn handle_blur(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::Blur, |set| {
      set.blur.push(Handler::new(handler));
    });
  }

  /// Adds a scroll handler to `node`.
  pub fn handle_scroll(&mut self, node: NodeId, handler: Rc<ScrollHandler>) {
    self.register(node, EventKind::Scroll, |set| {
      set.scroll.push(Handler::new(handler));
    });
  }

  /// Adds a change handler to `node`.
  pub fn handle_change(&mut self, node: NodeId, handler: Rc<ChangeHandler>) {
    self.register(node, EventKind::Change, |set| {
      set.change.push(Handler::new(handler));
    });
  }

  /// Adds a selection handler to `node`.
  pub fn handle_selection_change(&mut self, node: NodeId, handler: Rc<SelectionHandler>) {
    self.register(node, EventKind::SelectionChange, |set| {
      set.selection_change.push(Handler::new(handler));
    });
  }

  /// Adds a toggle handler to `node`.
  pub fn handle_toggle(&mut self, node: NodeId, handler: Rc<ToggleHandler>) {
    self.register(node, EventKind::Toggle, |set| {
      set.toggle.push(Handler::new(handler));
    });
  }

  /// Adds a dismiss handler to `node`.
  pub fn handle_dismiss(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::Dismiss, |set| {
      set.dismiss.push(Handler::new(handler));
    });
  }

  /// Adds a context menu handler to `node`.
  pub fn handle_context_menu(&mut self, node: NodeId, handler: Rc<PointerHandler>) {
    self.register(node, EventKind::ContextMenu, |set| {
      set.context_menu.push(Handler::new(handler));
    });
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
  pub fn handlers(&self, node: NodeId) -> Option<&HandlerSet> {
    self.record(node).and_then(NodeRecord::handlers)
  }

  fn register(&mut self, node: NodeId, kind: EventKind, add: impl FnOnce(&mut HandlerSet)) {
    let Some(record) = self.record_mut(node)
    else {
      return;
    };

    add(record.handlers.get_or_insert_default());

    let sink = record.sink;

    self.mirrors.push(Mirror::Listen { sink, kind });
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
    let children = self
      .record(parent)
      .map(NodeRecord::children)
      .unwrap_or_default();

    children
      .iter()
      .position(|node| *node == last)
      .is_some_and(|at| children.get(at + 1).copied() == before)
  }

  /// Inserts `child` into `parent`'s children before `before` (else last) and mirrors
  /// it; `child` must be detached.
  fn link(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    if self.record(child).is_none() {
      return;
    }

    let Some(parent_record) = self.record_mut(parent)
    else {
      return;
    };
    let index = before
      .and_then(|before| position_from_ends(&parent_record.children, before))
      .unwrap_or(parent_record.children.len());

    parent_record.children.insert(index, child);

    let Some(child_record) = self.record_mut(child)
    else {
      return;
    };

    child_record.parent = Some(parent);

    let (Some(parent), Some(child)) = (self.sink(parent), self.sink(child))
    else {
      return;
    };
    let before = before.and_then(|before| self.sink(before));

    self.mirrors.push(Mirror::Attach {
      parent,
      child,
      before,
    });
  }

  /// Removes `child` from under its parent, if any, and mirrors it; a segment start
  /// pulls its content and end anchor back into the pending list.
  fn unlink(&mut self, child: NodeId) {
    let Some(record) = self.record(child)
    else {
      return;
    };
    let Some(parent) = record.parent
    else {
      return;
    };

    if record.is_segment_start() {
      self.unlink_segment(parent, child);

      return;
    }

    self.unlink_one(parent, child);
  }

  /// Removes one node from under `parent` and mirrors it.
  fn unlink_one(&mut self, parent: NodeId, child: NodeId) {
    if let Some(parent_record) = self.record_mut(parent)
      && let Some(index) = position_from_ends(&parent_record.children, child)
    {
      let _unlinked = parent_record.children.remove(index);
    }

    if let Some(child_record) = self.record_mut(child) {
      child_record.parent = None;
    }

    if let (Some(parent), Some(child)) = (self.sink(parent), self.sink(child)) {
      self.mirrors.push(Mirror::Detach { parent, child });
    }
  }

  /// Removes a segment start, its content and its end anchor from under `parent`, and
  /// keeps the content pending; a nested segment is unlinked as one, keeping its own
  /// content pending in turn.
  fn unlink_segment(&mut self, parent: NodeId, start: NodeId) {
    let content = self.segment_nodes(start);
    let end = self.segment_end(start);

    self.unlink_one(parent, start);

    for node in &content {
      self.unlink(*node);
    }

    if let Some(end) = end {
      self.unlink_one(parent, end);
    }

    if let Some(segment) = self
      .record_mut(start)
      .and_then(|record| record.segment.as_mut())
    {
      segment.pending = content;
    }
  }
}

/// The index of `node` among `children`, searched from both ends at once, so a node
/// near either end is found at once.
fn position_from_ends(children: &[NodeId], node: NodeId) -> Option<usize> {
  let (mut front, mut back) = (0, children.len());

  while front < back {
    if children[front] == node {
      return Some(front);
    }

    back -= 1;

    if children[back] == node {
      return Some(back);
    }

    front += 1;
  }

  None
}

#[cfg(test)]
mod tests {
  use super::{NodeKind, Scene};
  use crate::{
    Color,
    Definition,
    Definitions,
    TokenName,
    testing::{Call, Erased, TestBox, TestPlatform, children, sink},
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

  /// Attaching an attached node unlinks it from its previous parent first, in the
  /// scene and in the platform.
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
      [
        Call::Detach(sink(first), sink(child)),
        Call::Attach(sink(second), sink(child), None),
      ]
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
        Call::Attach(sink(parent), sink(second), None),
        Call::Attach(sink(parent), sink(first), Some(sink(second))),
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
      [
        Call::Detach(sink(parent), sink(first)),
        Call::Attach(sink(parent), sink(first), None),
      ]
    );
  }

  /// Destroying a node takes its whole subtree, hands every sink back, and retires
  /// every id.
  #[test]
  fn destroy_takes_the_subtree_and_hands_every_sink_back() {
    TestPlatform::reset();

    let root = TestBox::node();
    let parent = TestBox::node();
    let child = TestBox::node();
    let grandchild = TestBox::node();
    let sinks = [sink(parent), sink(child), sink(grandchild)];

    with(|scene| {
      scene.attach(root, parent, None);
      scene.attach(parent, child, None);
      scene.attach(child, grandchild, None);
    });
    TestPlatform::take_log();
    with(|scene| scene.destroy(parent));

    let log = TestPlatform::take_log();
    let mut destroyed: Vec<u32> = log
      .iter()
      .filter_map(|call| match call {
        Call::Destroy(sink) => Some(*sink),
        _ => None,
      })
      .collect();

    destroyed.sort_unstable();

    assert_eq!(log[0], Call::Detach(sink(root), sinks[0]));
    assert_eq!(destroyed, sinks);
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
      [
        Call::DefineRootTokens(1),
        Call::DefineTokens(sink(outer), 1)
      ]
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

    let mut destroyed: Vec<u32> = TestPlatform::take_log()
      .iter()
      .filter_map(|call| match call {
        Call::Destroy(sink) => Some(*sink),
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
}
