//! The retained tree every view attaches to, owned by core and mirrored into a driver.

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
  Driver,
  EventKind,
  NodeId,
  Part,
  TokenName,
  Value,
};
use crate::{
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
};

/// A handler taking nothing: pointer enter and leave, focus and blur, dismiss.
pub type PlainHandler = dyn Fn();
/// A scroll handler: the container's new offsets.
pub type ScrollHandler = dyn Fn(&ScrollEvent);
/// A change handler: a control's new textual value.
pub type ChangeHandler = dyn Fn(&str);
/// A toggle handler: a control's new checked state.
pub type ToggleHandler = dyn Fn(bool);

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
pub struct Annotation<D: Driver> {
  /// The child's flexibility in a flex container.
  pub flex: Option<Annotated<D, Flexibility<D::Scalar>>>,
  /// The child's own cross- or block-axis alignment.
  pub self_alignment: Option<Annotated<D, ItemAlignment>>,
  /// The child's own inline-axis alignment in a grid.
  pub self_horizontal_alignment: Option<Annotated<D, ItemAlignment>>,
  /// The edges the child is pinned to in a stack.
  pub pinned: Option<Annotated<D, Inset<Dimension<D::Scalar>>>>,
  /// The child's row placement in a grid.
  pub grid_row_placement: Option<Annotated<D, Line<GridPlacement>>>,
  /// The child's column placement in a grid.
  pub grid_column_placement: Option<Annotated<D, Line<GridPlacement>>>,
}

/// A property value as an annotation carries it: declared under the driver's
/// conditions.
pub type Annotated<D, T> = Value<T, Condition<<D as Driver>::Scalar>>;

impl<D: Driver> Annotation<D> {
  /// Writes every property set here into `style`, replacing what it held.
  pub fn apply_to(self, style: &mut DeclaredBoxStyle<D>) {
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

impl<D: Driver> Default for Annotation<D> {
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

impl<D: Driver> Clone for Annotation<D> {
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

impl<D: Driver> PartialEq for Annotation<D> {
  fn eq(&self, other: &Self) -> bool {
    self.flex == other.flex
      && self.self_alignment == other.self_alignment
      && self.self_horizontal_alignment == other.self_horizontal_alignment
      && self.pinned == other.pinned
      && self.grid_row_placement == other.grid_row_placement
      && self.grid_column_placement == other.grid_column_placement
  }
}

impl<D: Driver> Debug for Annotation<D> {
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

/// The handlers a node carries, by event.
///
/// Every handler on a node runs when its event reaches the node; bubbling is between
/// nodes. A control's behavior registers its click as *acting*, apart from the
/// observers, so a driver whose platform performed the action can dispatch the
/// observers alone.
#[derive(Default)]
#[must_use]
pub struct HandlerSet {
  acting_click: Vec<Rc<ClickHandler>>,
  click: Vec<Rc<ClickHandler>>,
  key_down: Vec<Rc<KeyHandler>>,
  pointer_down: Vec<Rc<PointerHandler>>,
  pointer_up: Vec<Rc<PointerHandler>>,
  pointer_move: Vec<Rc<PointerHandler>>,
  pointer_enter: Vec<Rc<PlainHandler>>,
  pointer_leave: Vec<Rc<PlainHandler>>,
  focus: Vec<Rc<PlainHandler>>,
  blur: Vec<Rc<PlainHandler>>,
  scroll: Vec<Rc<ScrollHandler>>,
  change: Vec<Rc<ChangeHandler>>,
  toggle: Vec<Rc<ToggleHandler>>,
  dismiss: Vec<Rc<PlainHandler>>,
}

impl HandlerSet {
  /// The acting click handlers, in registration order: a control's own response to a
  /// click, run before the observers.
  #[inline(always)]
  #[must_use]
  pub fn acting_click(&self) -> &[Rc<ClickHandler>] {
    &self.acting_click
  }

  /// The click handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn click(&self) -> &[Rc<ClickHandler>] {
    &self.click
  }

  /// The key-down handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn key_down(&self) -> &[Rc<KeyHandler>] {
    &self.key_down
  }

  /// The pointer-down handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_down(&self) -> &[Rc<PointerHandler>] {
    &self.pointer_down
  }

  /// The pointer-up handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_up(&self) -> &[Rc<PointerHandler>] {
    &self.pointer_up
  }

  /// The pointer-move handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_move(&self) -> &[Rc<PointerHandler>] {
    &self.pointer_move
  }

  /// The pointer-enter handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_enter(&self) -> &[Rc<PlainHandler>] {
    &self.pointer_enter
  }

  /// The pointer-leave handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn pointer_leave(&self) -> &[Rc<PlainHandler>] {
    &self.pointer_leave
  }

  /// The focus handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn focus(&self) -> &[Rc<PlainHandler>] {
    &self.focus
  }

  /// The blur handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn blur(&self) -> &[Rc<PlainHandler>] {
    &self.blur
  }

  /// The scroll handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn scroll(&self) -> &[Rc<ScrollHandler>] {
    &self.scroll
  }

  /// The change handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn change(&self) -> &[Rc<ChangeHandler>] {
    &self.change
  }

  /// The toggle handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn toggle(&self) -> &[Rc<ToggleHandler>] {
    &self.toggle
  }

  /// The dismiss handlers, in registration order.
  #[inline(always)]
  #[must_use]
  pub fn dismiss(&self) -> &[Rc<PlainHandler>] {
    &self.dismiss
  }
}

/// The end of a segment and, while the segment is detached, the content waiting
/// between its anchors.
struct Segment {
  end: NodeId,
  pending: Vec<NodeId>,
}

/// The token values a node resolved under one definitions generation.
struct TokenCache<V> {
  generation: u64,
  values: FxHashMap<TokenName, Option<V>>,
}

/// One node of the scene.
#[must_use]
pub struct NodeRecord<D: Driver> {
  parent: Option<NodeId>,
  children: SmallVec<[NodeId; 4]>,
  kind: NodeKind,
  annotation: Option<Annotation<D>>,
  definitions: Option<Box<Definitions<D::TokenValue>>>,
  container: Option<ContainerName>,
  handlers: Option<Box<HandlerSet>>,
  sink: D::Sink,
  segment: Option<Box<Segment>>,
  cache: Option<Box<TokenCache<D::TokenValue>>>,
  control: Option<ControlState>,
  part: Option<Part>,
  position: Position,
}

impl<D: Driver> NodeRecord<D> {
  /// The node this one is attached under.
  #[inline(always)]
  #[must_use]
  pub const fn parent(&self) -> Option<NodeId> {
    self.parent
  }

  /// The node's place in the state snapshot, matched across runs as a signal's is: a
  /// driver carries what a node holds outside the reactive graph—focus, a scroll
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
  pub const fn annotation(&self) -> Option<&Annotation<D>> {
    self.annotation.as_ref()
  }

  /// The token definitions placed on this node.
  #[inline(always)]
  #[must_use]
  pub fn definitions(&self) -> Option<&Definitions<D::TokenValue>> {
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

  /// The driver's handle for this node.
  #[inline(always)]
  #[must_use]
  pub const fn sink(&self) -> &D::Sink {
    &self.sink
  }

  /// The driver's handle for this node, mutably.
  #[inline(always)]
  #[must_use]
  pub const fn sink_mut(&mut self) -> &mut D::Sink {
    &mut self.sink
  }

  /// Whether this node is the start anchor of a segment.
  #[inline(always)]
  const fn is_segment_start(&self) -> bool {
    self.segment.is_some()
  }
}

struct Entry<D: Driver> {
  generation: u32,
  record: Option<NodeRecord<D>>,
}

/// A mutation the scene owes its driver, mirrored once the scene is released.
enum Mirror<D: Driver> {
  Attach {
    parent: D::Sink,
    child: D::Sink,
    before: Option<D::Sink>,
  },
  Detach {
    parent: D::Sink,
    child: D::Sink,
  },
  Destroy(D::Sink),
  DefineTokens {
    scope: D::Sink,
    definitions: Definitions<D::TokenValue>,
  },
  DefineRootTokens(Definitions<D::TokenValue>),
  ControlState {
    sink: D::Sink,
    state: ControlState,
  },
}

impl<D: Driver> Mirror<D> {
  /// Hands the mutation to the driver.
  fn run(self) {
    let driver = D::default();

    match self {
      Self::Attach {
        parent,
        child,
        before,
      } => driver.attach(&parent, &child, before.as_ref()),
      Self::Detach { parent, child } => driver.detach(&parent, &child),
      Self::Destroy(sink) => driver.destroy(sink),
      Self::DefineTokens { scope, definitions } => driver.define_tokens(&scope, &definitions),
      Self::DefineRootTokens(definitions) => driver.define_root_tokens(&definitions),
      Self::ControlState { sink, state } => driver.set_control_state(&sink, &state),
    }
  }
}

/// The retained tree: a generational arena of [`NodeRecord`]s, every mutation mirrored
/// into the driver's sink.
///
/// Reached through [`with`](Self::with). No borrow of the scene is held while a user
/// callback or a driver's mutation runs: dispatch and every handler invocation snapshot
/// what they need and release the scene first, and the mutations the scene mirrors into
/// the driver ([`attach`](Self::attach), [`detach`](Self::detach),
/// [`destroy`](Self::destroy), [`define_tokens`](Self::define_tokens),
/// [`define_root_tokens`](Self::define_root_tokens),
/// [`set_control_state`](Self::set_control_state)) reach it in order once the borrow
/// that recorded them is released.
#[must_use]
pub struct Scene<D: Driver> {
  entries: Vec<Entry<D>>,
  free: Vec<u32>,
  pending_annotation: Option<Annotation<D>>,
  root_definitions: Definitions<D::TokenValue>,
  root_sets: FxHashSet<TypeId>,
  definitions_generation: u64,
  graveyard: Vec<HandlerSet>,
  mirrors: Vec<Mirror<D>>,
}

impl<D: Driver> Default for Scene<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> Scene<D> {
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
    }
  }

  /// Runs `f` on the driver's scene.
  ///
  /// The mutations `f` recorded are mirrored into the driver after the scene is
  /// released, in order, and handlers released inside `f` are dropped after that, so
  /// a driver's mutation or a captured value's drop may reach the scene again.
  ///
  /// # Panics
  ///
  /// Panics if the scene is already borrowed: `f` must not re-enter it.
  pub fn with<R>(operation: impl FnOnce(&mut Self) -> R) -> R {
    D::scene().with(|cell| {
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
    D::scene().with(|cell| cell.try_borrow_mut().is_err())
  }

  /// The record of `node`, if it is live.
  #[inline(always)]
  #[must_use]
  pub fn record(&self, node: NodeId) -> Option<&NodeRecord<D>> {
    self
      .entries
      .get(node.index() as usize)
      .filter(|entry| entry.generation == node.generation())
      .and_then(|entry| entry.record.as_ref())
  }

  /// The record of `node`, mutably, if it is live.
  #[inline(always)]
  #[must_use]
  pub fn record_mut(&mut self, node: NodeId) -> Option<&mut NodeRecord<D>> {
    self
      .entries
      .get_mut(node.index() as usize)
      .filter(|entry| entry.generation == node.generation())
      .and_then(|entry| entry.record.as_mut())
  }

  /// The driver's handle for `node`, cloned out, if the node is live.
  ///
  /// Copy it out and release the scene before calling the driver with it.
  #[inline(always)]
  #[must_use]
  pub fn sink(&self, node: NodeId) -> Option<D::Sink> {
    self.record(node).map(|record| record.sink.clone())
  }

  /// Mints a detached node of `kind` owning `sink`; a box, a control or a run consumes
  /// the pending annotation, and a control starts in the default [`ControlState`].
  ///
  /// A box folds the annotation into its style before creation; a run keeps it on its
  /// record, where the driver reads it when it lays the run out.
  ///
  /// # Panics
  ///
  /// Panics past 2^32 live nodes.
  pub fn create(&mut self, kind: NodeKind, sink: D::Sink) -> NodeId {
    let is_control = kind == NodeKind::Control;
    let annotation = if matches!(kind, NodeKind::Box | NodeKind::Control | NodeKind::Run) {
      self.pending_annotation.take()
    }
    else {
      None
    };
    let position = Position::next();
    let record = NodeRecord {
      parent: None,
      children: SmallVec::new(),
      kind,
      annotation,
      definitions: None,
      container: None,
      handlers: None,
      sink,
      segment: None,
      cache: None,
      control: is_control.then_some(ControlState::DEFAULT),
      part: is_control.then_some(Part::Root),
      position,
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
      D::default().bind_node(&record.sink, id);
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
  /// with whatever is mounted between them.
  ///
  /// Returns `(start, end)`; the start stands for the segment everywhere.
  pub fn create_segment(&mut self) -> (NodeId, NodeId) {
    let driver = D::default();
    let start = self.create(NodeKind::Anchor, driver.create_anchor());
    let end = self.create(NodeKind::Anchor, driver.create_anchor());

    if let Some(record) = self.record_mut(start) {
      record.segment = Some(Box::new(Segment {
        end,
        pending: Vec::new(),
      }));
    }

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
  /// unlinking it from any previous parent first.
  ///
  /// A segment start brings its content and its end anchor along.
  pub fn attach(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
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
  /// sink is handed to the driver.
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

  /// Stores the annotation the next box created consumes.
  #[inline(always)]
  pub fn set_pending_annotation(&mut self, annotation: Annotation<D>) {
    self.pending_annotation = Some(annotation);
  }

  /// Takes the annotation waiting for the next box, so a builder can fold it into the
  /// box's style before creation.
  #[inline(always)]
  pub fn take_pending_annotation(&mut self) -> Option<Annotation<D>> {
    self.pending_annotation.take()
  }

  /// Records the annotation a box was built with.
  #[inline(always)]
  pub fn set_annotation(&mut self, node: NodeId, annotation: Annotation<D>) {
    if let Some(record) = self.record_mut(node) {
      record.annotation = Some(annotation);
    }
  }

  /// Defines tokens on `node`, replacing what it defined before.
  pub fn define_tokens(&mut self, node: NodeId, definitions: Definitions<D::TokenValue>) {
    let Some(record) = self.record_mut(node)
    else {
      return;
    };

    let scope = record.sink.clone();

    record.definitions = Some(Box::new(definitions.clone()));
    self
      .mirrors
      .push(Mirror::DefineTokens { scope, definitions });
    self.definitions_generation += 1;
  }

  /// Defines the tokens every node falls back to, replacing the previous set.
  pub fn define_root_tokens(&mut self, definitions: Definitions<D::TokenValue>) {
    self
      .mirrors
      .push(Mirror::DefineRootTokens(definitions.clone()));
    self.root_definitions = definitions;
    self.definitions_generation += 1;
  }

  /// Lays the definitions `define` produces over the tokens every node falls back to,
  /// once per `set`: a later call for the same set does nothing and returns `false`.
  pub fn define_root_tokens_once(
    &mut self,
    set: TypeId,
    define: impl FnOnce() -> Definitions<D::TokenValue>,
  ) -> bool {
    if !self.root_sets.insert(set) {
      return false;
    }

    let mut definitions =
      std::mem::replace(&mut self.root_definitions, Definitions::from(Vec::new()));

    definitions.overlay(define());
    self.define_root_tokens(definitions);

    true
  }

  /// The tokens every node falls back to.
  #[inline(always)]
  pub const fn root_definitions(&self) -> &Definitions<D::TokenValue> {
    &self.root_definitions
  }

  /// The definition of `name` in force at `node`: the nearest defining ancestor's, else
  /// the root's.
  ///
  /// Cached per node until a definition or the tree changes.
  pub fn resolve_token(&mut self, node: NodeId, name: TokenName) -> Option<D::TokenValue> {
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
        found = Some(value.clone());

        break;
      }

      current = record.parent;
    }

    let found = found.or_else(|| self.root_definitions.get(name).cloned());
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

    let sink = record.sink.clone();

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
    self.register(node, EventKind::Click, |set| set.click.push(handler));
  }

  /// Adds an acting click handler to `node`: a control's own response, run before the
  /// node's other click handlers and skipped by an observed dispatch.
  pub fn handle_click_acting(&mut self, node: NodeId, handler: Rc<ClickHandler>) {
    self.register(node, EventKind::Click, |set| set.acting_click.push(handler));
  }

  /// Adds a key-down handler to `node`.
  pub fn handle_key_down(&mut self, node: NodeId, handler: Rc<KeyHandler>) {
    self.register(node, EventKind::KeyDown, |set| set.key_down.push(handler));
  }

  /// Adds a pointer-down handler to `node`.
  pub fn handle_pointer_down(&mut self, node: NodeId, handler: Rc<PointerHandler>) {
    self.register(node, EventKind::PointerDown, |set| {
      set.pointer_down.push(handler);
    });
  }

  /// Adds a pointer-up handler to `node`.
  pub fn handle_pointer_up(&mut self, node: NodeId, handler: Rc<PointerHandler>) {
    self.register(node, EventKind::PointerUp, |set| {
      set.pointer_up.push(handler);
    });
  }

  /// Adds a pointer-move handler to `node`.
  pub fn handle_pointer_move(&mut self, node: NodeId, handler: Rc<PointerHandler>) {
    self.register(node, EventKind::PointerMove, |set| {
      set.pointer_move.push(handler);
    });
  }

  /// Adds a pointer-enter handler to `node`.
  pub fn handle_pointer_enter(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::PointerEnter, |set| {
      set.pointer_enter.push(handler);
    });
  }

  /// Adds a pointer-leave handler to `node`.
  pub fn handle_pointer_leave(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::PointerLeave, |set| {
      set.pointer_leave.push(handler);
    });
  }

  /// Adds a focus handler to `node`.
  pub fn handle_focus(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::Focus, |set| set.focus.push(handler));
  }

  /// Adds a blur handler to `node`.
  pub fn handle_blur(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::Blur, |set| set.blur.push(handler));
  }

  /// Adds a scroll handler to `node`.
  pub fn handle_scroll(&mut self, node: NodeId, handler: Rc<ScrollHandler>) {
    self.register(node, EventKind::Scroll, |set| set.scroll.push(handler));
  }

  /// Adds a change handler to `node`.
  pub fn handle_change(&mut self, node: NodeId, handler: Rc<ChangeHandler>) {
    self.register(node, EventKind::Change, |set| set.change.push(handler));
  }

  /// Adds a toggle handler to `node`.
  pub fn handle_toggle(&mut self, node: NodeId, handler: Rc<ToggleHandler>) {
    self.register(node, EventKind::Toggle, |set| set.toggle.push(handler));
  }

  /// Adds a dismiss handler to `node`.
  pub fn handle_dismiss(&mut self, node: NodeId, handler: Rc<PlainHandler>) {
    self.register(node, EventKind::Dismiss, |set| set.dismiss.push(handler));
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
    D::default().listen(&record.sink, kind);
  }

  /// The parent the segment's content sits under (`None` while detached) and its end.
  fn segment_place(&self, start: NodeId) -> Option<(Option<NodeId>, NodeId)> {
    let record = self.record(start)?;
    let segment = record.segment.as_ref()?;

    Some((record.parent, segment.end))
  }

  /// Takes the record of `node` out of the arena, retiring the slot's generation.
  fn remove(&mut self, node: NodeId) -> Option<NodeRecord<D>> {
    let entry = self
      .entries
      .get_mut(node.index() as usize)
      .filter(|entry| entry.generation == node.generation())?;
    let record = entry.record.take()?;

    entry.generation = entry.generation.wrapping_add(1);
    self.free.push(node.index());

    Some(record)
  }

  /// Inserts `child` into `parent`'s children before `before` (else last) and mirrors
  /// it; `child` must be detached.
  fn link(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    let Some(parent_record) = self.record_mut(parent)
    else {
      return;
    };
    let index = before
      .and_then(|before| {
        parent_record
          .children
          .iter()
          .position(|node| *node == before)
      })
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
    if let Some(parent_record) = self.record_mut(parent) {
      parent_record.children.retain(|node| *node != child);
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

#[cfg(test)]
mod tests {
  use super::{NodeKind, Scene};
  use crate::{
    Color,
    Definition,
    Definitions,
    TokenName,
    testing::{Call, Erased, TestBox, TestDriver, children, sink},
  };

  fn with<R>(operation: impl FnOnce(&mut Scene<TestDriver>) -> R) -> R {
    Scene::<TestDriver>::with(operation)
  }

  fn color(value: f32) -> Erased {
    Erased::Color(Definition::from(Color::srgb(value, value, value)))
  }

  fn definitions(name: &'static str, value: Erased) -> Definitions<Erased> {
    Definitions::from(vec![(TokenName::new(name), value)])
  }

  /// Attaching an attached node unlinks it from its previous parent first, in the
  /// scene and in the driver.
  #[test]
  fn attach_unlinks_the_previous_parent() {
    TestDriver::reset();

    let first = TestBox::node();
    let second = TestBox::node();
    let child = TestBox::node();

    with(|scene| scene.attach(first, child, None));
    TestDriver::take_log();
    with(|scene| scene.attach(second, child, None));

    assert_eq!(with(|scene| scene.parent(child)), Some(second));
    assert_eq!(children(first), []);
    assert_eq!(children(second), [child]);
    assert_eq!(
      TestDriver::take_log(),
      [
        Call::Detach(sink(first), sink(child)),
        Call::Attach(sink(second), sink(child), None),
      ]
    );
  }

  /// `before` places a child ahead of a sibling, in the scene and in the driver.
  #[test]
  fn attach_before_orders_the_children() {
    TestDriver::reset();

    let parent = TestBox::node();
    let first = TestBox::node();
    let second = TestBox::node();

    with(|scene| {
      scene.attach(parent, second, None);
      scene.attach(parent, first, Some(second));
    });

    assert_eq!(children(parent), [first, second]);
    assert_eq!(
      TestDriver::take_log(),
      [
        Call::Attach(sink(parent), sink(second), None),
        Call::Attach(sink(parent), sink(first), Some(sink(second))),
      ]
    );
  }

  /// Destroying a node takes its whole subtree, hands every sink back, and retires
  /// every id.
  #[test]
  fn destroy_takes_the_subtree_and_hands_every_sink_back() {
    TestDriver::reset();

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
    TestDriver::take_log();
    with(|scene| scene.destroy(parent));

    let log = TestDriver::take_log();
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
    TestDriver::reset();

    let card = TokenName::new("card");
    let outer = TestBox::node();
    let inner = TestBox::node();
    let leaf = TestBox::node();
    let stranger = TestBox::node();

    with(|scene| {
      scene.attach(outer, inner, None);
      scene.attach(inner, leaf, None);
    });
    TestDriver::take_log();
    with(|scene| {
      scene.define_root_tokens(definitions("card", color(0.)));
      scene.define_tokens(outer, definitions("card", color(0.5)));
    });

    assert_eq!(
      TestDriver::take_log(),
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
      (generation, Some(Some(color(0.5))))
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
    TestDriver::reset();

    let parent = TestBox::node();
    let other = TestBox::node();
    let content = TestBox::node();
    let (start, end) = with(Scene::create_segment);

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

    TestDriver::take_log();
    with(|scene| scene.destroy(start));

    let mut destroyed: Vec<u32> = TestDriver::take_log()
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
    TestDriver::reset();

    let parent = TestBox::node();
    let before = TestBox::node();
    let inner_content = TestBox::node();
    let (outer, outer_end) = with(Scene::create_segment);
    let (inner, inner_end) = with(Scene::create_segment);

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
