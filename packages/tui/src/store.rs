//! The record store beside the scene: what the driver keeps per node, the taffy tree,
//! the interaction state, and the environment it reports.

use std::{rc::Rc, time::Instant};

use rustc_hash::{FxHashMap, FxHashSet};
use skene_core::{
  BoxDynamics,
  BoxProps,
  BoxStyle,
  Color,
  ColorScheme,
  ContainerName,
  ContainerSizes,
  ControlKind,
  ControlState,
  FocusOrder,
  Layout,
  MediaSignals,
  NodeId,
  Placement,
  Plain,
  RunProps,
  RunStyle,
  Size,
  Slot,
};
use skene_reactive::{Owner, Signal, WriteSignal};

use crate::{
  Tui,
  TuiExt,
  motion::{BoxMotion, RunMotion},
  paint::Buffer,
  selection::Selection,
};

/// The size the driver reports until a frame is rendered.
pub(crate) const DEFAULT_SIZE: (u16, u16) = (80, 24);

/// A box's style resolved against live state.
pub(crate) type PlainBox = BoxStyle<u32, Plain, TuiExt>;

/// A run's style resolved against live state.
pub(crate) type PlainRun = RunStyle<Plain, TuiExt>;

/// A rectangle of cells on the screen: its origin, then its size.
pub(crate) type CellRect = (i64, i64, i64, i64);

/// The driver's sink: the index of a node's record and the generation it was minted
/// in, so a handle whose record was destroyed reaches nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct Handle {
  index: u32,
  generation: u32,
}

impl Handle {
  #[inline(always)]
  const fn new(index: u32, generation: u32) -> Self {
    Self { index, generation }
  }

  /// The record's index: its place in creation order.
  #[inline(always)]
  #[must_use]
  pub const fn index(self) -> u32 {
    self.index
  }
}

/// The value a derived leaf last wrote into a node's slot.
pub(crate) type SlotValue = skene_core::SlotValue<u32, TuiExt>;

pub(crate) use skene_core::Interaction;

/// A box's style resolved under one generation and interaction: the style presented,
/// with its motion laid over, and the target it resolved to.
pub(crate) struct BoxCache {
  pub(crate) generation: u64,
  pub(crate) interaction: Interaction,
  /// The style laid out and painted: the target with every running tween sampled.
  pub(crate) style: PlainBox,
  /// The style the resolution produced, which a change is judged against.
  pub(crate) target: PlainBox,
  /// The border's color leaf presented; `None` for no border or a transparent one.
  pub(crate) border_ink: Option<Color>,
  /// The border's color leaf resolved.
  pub(crate) target_ink: Option<Color>,
  /// The keyframes resolved at the node, reused across the frames of a running motion.
  pub(crate) keyframes: Vec<skene_core::Animation<PlainBox>>,
}

/// A run's style resolved under one generation and interaction, with the per-child
/// layout properties a wrapper annotated the run with.
pub(crate) struct RunCache {
  pub(crate) generation: u64,
  pub(crate) interaction: Interaction,
  /// The style painted: the target with every running tween sampled.
  pub(crate) style: PlainRun,
  /// The style the resolution produced, which a change is judged against.
  pub(crate) target: PlainRun,
  pub(crate) layout: PlainBox,
  /// The keyframes resolved at the node, reused across the frames of a running motion.
  pub(crate) keyframes: Vec<skene_core::Animation<PlainRun>>,
}

/// A box-like node's props, its resolution and its motion.
pub(crate) struct BoxRecord {
  pub(crate) props: BoxProps<Tui>,
  pub(crate) cache: Option<BoxCache>,
  pub(crate) motion: BoxMotion,
}

impl BoxRecord {
  pub(crate) fn new(props: BoxProps<Tui>) -> Self {
    Self {
      props,
      cache: None,
      motion: BoxMotion::default(),
    }
  }

  /// Marks the resolution stale while keeping what the node presents, so the next
  /// frame resolves again and a moved value starts from where the eye left off.
  #[inline(always)]
  pub(crate) fn mark_stale(&mut self) {
    if let Some(cache) = &mut self.cache {
      cache.generation = 0;
    }
  }

  /// The resolved style, once a frame resolved it.
  #[inline(always)]
  pub(crate) fn style(&self) -> Option<&PlainBox> {
    self.cache.as_ref().map(|cache| &cache.style)
  }
}

/// A text entry's editing state; the caret is a character position in the value,
/// `0..=len`.
pub(crate) struct Field {
  pub(crate) value: String,
  pub(crate) caret: usize,
  pub(crate) placeholder: Option<String>,
  pub(crate) lines: u16,
}

/// What a record is, with everything its creation call carried.
pub(crate) enum Kind {
  Box(BoxRecord),
  Control {
    kind: ControlKind,
    inner: BoxRecord,
    state: ControlState,
    field: Option<Field>,
    checked: bool,
  },
  Overlay {
    inner: BoxRecord,
    modal: bool,
    placement: Placement,
    backdrop: Option<Color>,
    open: bool,
    anchor: Option<Handle>,
  },
  Run {
    props: RunProps<Tui>,
    cache: Option<RunCache>,
    motion: RunMotion,
  },
  Anchor,
  Scope,
}

impl Kind {
  /// The box record of a box, a control or an overlay.
  #[inline(always)]
  pub(crate) const fn box_record(&self) -> Option<&BoxRecord> {
    match self {
      Self::Box(inner) | Self::Control { inner, .. } | Self::Overlay { inner, .. } => Some(inner),
      Self::Run { .. } | Self::Anchor | Self::Scope => None,
    }
  }

  /// The box record of a box, a control or an overlay, mutably.
  #[inline(always)]
  pub(crate) const fn box_record_mut(&mut self) -> Option<&mut BoxRecord> {
    match self {
      Self::Box(inner) | Self::Control { inner, .. } | Self::Overlay { inner, .. } => Some(inner),
      Self::Run { .. } | Self::Anchor | Self::Scope => None,
    }
  }

  /// The resolved style of a box-like record, once a frame resolved it.
  #[inline(always)]
  pub(crate) fn box_style(&self) -> Option<&PlainBox> {
    self.box_record().and_then(BoxRecord::style)
  }

  /// Whether the record takes part in layout: everything but an anchor or a scope.
  #[inline(always)]
  pub(crate) const fn is_laid_out(&self) -> bool {
    !matches!(self, Self::Anchor | Self::Scope)
  }

  /// The editing state of a text entry.
  #[inline(always)]
  pub(crate) const fn field(&self) -> Option<&Field> {
    match self {
      Self::Control { field, .. } => field.as_ref(),
      _ => None,
    }
  }

  /// The editing state of a text entry, mutably.
  #[inline(always)]
  pub(crate) const fn field_mut(&mut self) -> Option<&mut Field> {
    match self {
      Self::Control { field, .. } => field.as_mut(),
      _ => None,
    }
  }
}

/// One realized node.
pub(crate) struct Record {
  pub(crate) kind: Kind,
  pub(crate) taffy: Option<taffy::NodeId>,
  pub(crate) node: Option<NodeId>,
  pub(crate) parent: Option<Handle>,
  pub(crate) children: Vec<Handle>,
  /// The taffy children last handed to the layout tree.
  pub(crate) taffy_children: Vec<taffy::NodeId>,
  /// The text the layout tree carries for a run or a paragraph.
  pub(crate) measured_text: Option<String>,
  pub(crate) applied: Option<taffy::Style>,
  pub(crate) text: String,
  pub(crate) slots: FxHashMap<Slot, SlotValue>,
  pub(crate) listened: u16,
  pub(crate) observers: Vec<Rc<dyn Fn(Size<u32>)>>,
  pub(crate) reported: Option<Size<u32>>,
  /// The scroll offsets, in cells.
  pub(crate) scroll: (u32, u32),
  /// The screen rectangle the node covered at the last paint.
  pub(crate) painted: Option<CellRect>,
  /// Whether a tween or an animation of the node was still running at the last
  /// restyle, so the next frame resolves it again.
  pub(crate) in_motion: bool,
  /// The painter a low-level view runs inside the box on every repaint.
  pub(crate) painter: Option<Box<crate::cells::Painter>>,
}

impl Record {
  fn new(kind: Kind, taffy: Option<taffy::NodeId>) -> Self {
    Self {
      kind,
      taffy,
      node: None,
      parent: None,
      children: Vec::new(),
      taffy_children: Vec::new(),
      measured_text: None,
      applied: None,
      text: String::new(),
      slots: FxHashMap::default(),
      listened: 0,
      observers: Vec::new(),
      reported: None,
      scroll: (0, 0),
      painted: None,
      in_motion: false,
      painter: None,
    }
  }

  /// The box props of a box, a control or an overlay.
  #[inline(always)]
  pub(crate) fn box_props(&self) -> Option<&BoxProps<Tui>> {
    self.kind.box_record().map(|inner| &inner.props)
  }

  /// The record's place in focus traversal.
  #[inline(always)]
  pub(crate) fn focus_order(&self) -> Option<FocusOrder> {
    self.box_props().and_then(|props| props.focus_order)
  }

  /// Whether the record takes focus at all: by traversal, by pointer, or
  /// programmatically.
  #[inline(always)]
  pub(crate) fn is_focusable(&self) -> bool {
    self.focus_order().is_some()
  }

  /// The layout mode of a box-like record.
  #[inline(always)]
  pub(crate) fn layout(&self) -> Option<Layout> {
    self.box_props().map(|props| props.layout)
  }

  /// The name the record publishes for container queries.
  #[inline(always)]
  pub(crate) fn container(&self) -> Option<ContainerName> {
    self.box_props().and_then(|props| props.container)
  }

  /// Whether the record is an overlay.
  #[inline(always)]
  pub(crate) const fn is_overlay(&self) -> bool {
    matches!(self.kind, Kind::Overlay { .. })
  }

  /// Whether the record is an open overlay.
  #[inline(always)]
  pub(crate) const fn is_open_overlay(&self) -> bool {
    matches!(self.kind, Kind::Overlay { open: true, .. })
  }

  /// Whether the record is a control.
  #[inline(always)]
  pub(crate) const fn is_control(&self) -> bool {
    matches!(self.kind, Kind::Control { .. })
  }
}

struct Entry {
  generation: u32,
  record: Option<Record>,
}

/// A press-initiated drag in progress, followed by pointer motion until release.
#[derive(Clone, Copy)]
pub(crate) enum Drag {
  /// A scrollbar's thumb: motion scrolls so the grabbed spot follows the pointer.
  Scrollbar {
    node: Handle,
    axis: skene_core::Axis,
    /// The grabbed position within the thumb, in track cells.
    grab: i64,
  },
  /// A text selection: motion extends it to the position under the pointer.
  Selection,
}

/// The pause that ends a wheel gesture: the next wheel event re-targets, as on the
/// web.
pub(crate) const SCROLL_GESTURE: std::time::Duration = std::time::Duration::from_millis(500);

/// One wheel gesture's scroll target: the cell it hit, the box it scrolls, and the
/// last wheel event's time.
pub(crate) struct ScrollLatch {
  pub(crate) cell: (u16, u16),
  pub(crate) target: Handle,
  pub(crate) last: Instant,
}

/// The delivered left press awaiting its release: the hit path, the node the press
/// captured, and the focusable it landed on.
pub(crate) struct Press {
  pub(crate) path: Vec<Handle>,
  pub(crate) captured: Handle,
  pub(crate) target: Option<Handle>,
}

/// The environment as signals, with the write halves the driver drives.
pub(crate) struct MediaState {
  pub(crate) owner: Owner,
  pub(crate) set_viewport: WriteSignal<Size<u32>>,
  pub(crate) set_scheme: WriteSignal<ColorScheme>,
  pub(crate) set_reduced_motion: WriteSignal<bool>,
  pub(crate) signals: MediaSignals<u32>,
}

/// Every record the driver holds, the layout tree, the interaction state and the
/// environment.
// Each flag is an independent fact about the frame or the pointer.
#[expect(clippy::struct_excessive_bools)]
pub(crate) struct Store {
  entries: Vec<Entry>,
  free: Vec<u32>,
  /// The layout tree; a text run's leaf carries its text as context, wrapped by the
  /// layout's measure function.
  pub(crate) taffy: taffy::TaffyTree<String>,
  pub(crate) root: Handle,
  /// Every overlay, in build order; later entries stack on top.
  pub(crate) overlays: Vec<Handle>,
  pub(crate) focused: Option<Handle>,
  /// The path from the root to the deepest node under the mouse, when known.
  hovered: Vec<Handle>,
  /// The nodes on the hover path, for membership.
  hovered_set: FxHashSet<Handle>,
  /// The nodes whose paint changed since the last frame; each repaints the rows it
  /// covered and covers.
  damaged: Vec<Handle>,
  /// Screen rectangles to repaint that no live node covers any more.
  damaged_rects: Vec<CellRect>,
  /// The last painted frame, which the next paints over where nothing changed.
  pub(crate) last_frame: Option<Buffer>,
  /// The rows the last paint rewrote.
  pub(crate) repainted: Vec<usize>,
  pub(crate) drag: Option<Drag>,
  pub(crate) press: Option<Press>,
  /// Whether a left or right press has been delivered and not yet released; a release
  /// arriving without one stands in for a click whose press the terminal swallowed.
  pub(crate) left_held: bool,
  pub(crate) right_held: bool,
  /// Whether the press landed on the copy menu's `Copy` item, awaiting its release.
  pub(crate) pressed_copy: bool,
  pub(crate) selection: Option<Selection>,
  /// The copy menu's pressed cell, while the menu is open.
  pub(crate) menu: Option<(u16, u16)>,
  /// The last press, for counting multi-click sequences: its time, cell, and count.
  pub(crate) last_press: Option<(Instant, u16, u16, u8)>,
  /// The pointer's last known cell.
  pub(crate) last_pointer: (u16, u16),
  pub(crate) scroll_latch: Option<ScrollLatch>,
  /// Text staged for the terminal loop's clipboard write.
  pub(crate) pending_copy: Option<String>,
  /// The link pressed at its cell, opened when the release lands on the same cell.
  pub(crate) pressed_link: Option<(u16, u16, String)>,
  /// A link destination staged for the terminal loop to open.
  pub(crate) pending_open: Option<String>,
  /// The size of the last computed layout, which hit-testing resolves against.
  pub(crate) last_size: (u16, u16),
  /// Whether the terminal background is dark, as detected.
  pub(crate) detected_dark: bool,
  /// The color scheme the app has chosen, overriding the detected background.
  pub(crate) choice: Option<ColorScheme>,
  /// Whether OSC 52 clipboard writes reach the system clipboard.
  pub(crate) clipboard: bool,
  /// The node to scroll into view once the next layout is computed.
  pub(crate) reveal: Option<Handle>,
  /// The measured sizes of the named query containers, from the last layout.
  pub(crate) container_sizes: FxHashMap<ContainerName, Size<u32>>,
  pub(crate) title: Option<String>,
  pub(crate) title_dirty: bool,
  /// The resolution generation: moves when definitions, the environment or a control's
  /// state change, invalidating every resolved style.
  pub(crate) generation: u64,
  /// How many layouts have been computed.
  pub(crate) layout_generation: u64,
  /// How many taffy styles have been written.
  pub(crate) set_style_calls: u64,
  /// How many node styles have been resolved from their declarations.
  pub(crate) style_resolutions: u64,
  pub(crate) structure_dirty: bool,
  pub(crate) layout_dirty: bool,
  /// Whether the whole frame repaints; a change confined to a node damages it instead.
  pub(crate) paint_dirty: bool,
  pub(crate) quit: bool,
  /// The clock motion samples against.
  pub(crate) clock: Clock,
  /// Whether the reader prefers reduced motion.
  pub(crate) reduced_motion: bool,
  /// Whether a tween or an animation was still running at the last restyle, so the
  /// next frame is due at the frame rate.
  pub(crate) motion_active: bool,
  media: Option<MediaState>,
}

/// The clock motion samples against, in seconds: the wall clock in a launched app, a
/// hand-stepped one under a headless test.
pub(crate) enum Clock {
  /// The wall clock since `Instant`.
  Real(Instant),
  /// A reading stepped by hand.
  Manual(f64),
}

impl Clock {
  /// The current reading, in seconds.
  #[inline(always)]
  pub(crate) fn now(&self) -> f64 {
    match self {
      Self::Real(epoch) => epoch.elapsed().as_secs_f64(),
      Self::Manual(seconds) => *seconds,
    }
  }

  /// Steps the reading forward by `seconds`; a wall clock becomes a hand-stepped one
  /// at its current reading.
  #[inline(always)]
  pub(crate) fn advance(&mut self, seconds: f64) {
    *self = Self::Manual(self.now() + seconds);
  }
}

impl Store {
  pub(crate) fn new() -> Self {
    let mut taffy = taffy::TaffyTree::new();
    let root_taffy = taffy
      .new_leaf(taffy::style::Style::default())
      .expect("create the root taffy node");
    let root_props = BoxProps {
      layout: Layout::Flex {
        direction: skene_core::FlexDirection::Column,
      },
      role: None,
      style: BoxStyle::default(),
      dynamics: BoxDynamics::default(),
      transition_name: None,
      container: None,
      focus_order: None,
      access: Vec::new(),
      destination: None,
    };
    let mut store = Self {
      entries: Vec::new(),
      free: Vec::new(),
      taffy,
      root: Handle::new(0, 0),
      overlays: Vec::new(),
      focused: None,
      hovered: Vec::new(),
      hovered_set: FxHashSet::default(),
      damaged: Vec::new(),
      damaged_rects: Vec::new(),
      last_frame: None,
      repainted: Vec::new(),
      drag: None,
      press: None,
      left_held: false,
      right_held: false,
      pressed_copy: false,
      selection: None,
      menu: None,
      last_press: None,
      last_pointer: (0, 0),
      scroll_latch: None,
      pending_copy: None,
      pressed_link: None,
      pending_open: None,
      last_size: DEFAULT_SIZE,
      detected_dark: true,
      choice: None,
      clipboard: true,
      reveal: None,
      container_sizes: FxHashMap::default(),
      title: None,
      title_dirty: false,
      generation: 1,
      layout_generation: 0,
      set_style_calls: 0,
      style_resolutions: 0,
      structure_dirty: true,
      layout_dirty: true,
      paint_dirty: true,
      quit: false,
      clock: Clock::Manual(0.),
      reduced_motion: false,
      motion_active: false,
      media: None,
    };

    store.root = store.create(Kind::Box(BoxRecord::new(root_props)), Some(root_taffy));

    store
  }

  /// Mints a record over `taffy` and returns its handle.
  pub(crate) fn create(&mut self, kind: Kind, taffy: Option<taffy::NodeId>) -> Handle {
    let record = Record::new(kind, taffy);

    self.structure_dirty = true;

    if let Some(index) = self.free.pop() {
      let entry = &mut self.entries[index as usize];

      entry.record = Some(record);

      return Handle::new(index, entry.generation);
    }

    let index = u32::try_from(self.entries.len()).expect("a store holds at most 2^32 records");

    self.entries.push(Entry {
      generation: 0,
      record: Some(record),
    });

    Handle::new(index, 0)
  }

  /// Mints a record over a fresh taffy leaf.
  pub(crate) fn create_laid_out(&mut self, kind: Kind) -> Handle {
    let taffy = self
      .taffy
      .new_leaf(taffy::style::Style::default())
      .expect("create a taffy node");

    self.create(kind, Some(taffy))
  }

  /// The record of `handle`, if it is live.
  #[inline(always)]
  pub(crate) fn record(&self, handle: Handle) -> Option<&Record> {
    self
      .entries
      .get(handle.index as usize)
      .filter(|entry| entry.generation == handle.generation)
      .and_then(|entry| entry.record.as_ref())
  }

  /// The record of `handle`, mutably, if it is live.
  #[inline(always)]
  pub(crate) fn record_mut(&mut self, handle: Handle) -> Option<&mut Record> {
    self
      .entries
      .get_mut(handle.index as usize)
      .filter(|entry| entry.generation == handle.generation)
      .and_then(|entry| entry.record.as_mut())
  }

  /// The live record at creation index `index`.
  #[inline(always)]
  pub(crate) fn handle_at(&self, index: u32) -> Option<Handle> {
    self
      .entries
      .get(index as usize)
      .filter(|entry| entry.record.is_some())
      .map(|entry| Handle::new(index, entry.generation))
  }

  /// Every live handle, in creation order.
  pub(crate) fn handles(&self) -> Vec<Handle> {
    self
      .entries
      .iter()
      .enumerate()
      .filter(|(_, entry)| entry.record.is_some())
      .map(|(index, entry)| Handle::new(u32::try_from(index).expect("indexed"), entry.generation))
      .collect()
  }

  /// Releases the record of `handle`, retiring the handle and every reference to it.
  pub(crate) fn destroy(&mut self, handle: Handle) {
    let Some(entry) = self
      .entries
      .get_mut(handle.index as usize)
      .filter(|entry| entry.generation == handle.generation)
    else {
      return;
    };
    let Some(record) = entry.record.take()
    else {
      return;
    };

    entry.generation = entry.generation.wrapping_add(1);
    self.free.push(handle.index);

    if let Some(taffy) = record.taffy {
      let _ = self.taffy.remove(taffy);
    }

    if let Some(parent) = record.parent
      && let Some(parent) = self.record_mut(parent)
    {
      parent.children.retain(|child| *child != handle);
    }

    self.overlays.retain(|overlay| *overlay != handle);

    if self.hovered_set.remove(&handle) {
      self.hovered.retain(|hovered| *hovered != handle);
    }

    if let Some(rect) = record.painted {
      self.damaged_rects.push(rect);
    }

    if self.focused == Some(handle) {
      self.focused = None;
    }

    if self.reveal == Some(handle) {
      self.reveal = None;
    }

    if self
      .scroll_latch
      .as_ref()
      .is_some_and(|latch| latch.target == handle)
    {
      self.scroll_latch = None;
    }

    if self
      .press
      .as_ref()
      .is_some_and(|press| press.captured == handle)
    {
      self.press = None;
    }

    self.invalidate_selection(handle);
    self.structure_dirty = true;
    self.paint_dirty = true;
  }

  /// The children of `handle`, in order; empty for a leaf or a stale handle.
  #[inline(always)]
  pub(crate) fn children(&self, handle: Handle) -> &[Handle] {
    self.record(handle).map_or(&[], |record| &record.children)
  }

  /// The parent of `handle`.
  #[inline(always)]
  pub(crate) fn parent(&self, handle: Handle) -> Option<Handle> {
    self.record(handle).and_then(|record| record.parent)
  }

  /// The path from `handle` up to the root, `handle` first.
  pub(crate) fn ancestors(&self, handle: Handle) -> Vec<Handle> {
    let mut path = Vec::new();
    let mut current = Some(handle);

    while let Some(visited) = current {
      path.push(visited);
      current = self.parent(visited);
    }

    path
  }

  /// Whether walking up from `handle` reaches the root.
  pub(crate) fn is_reachable(&self, handle: Handle) -> bool {
    let mut current = Some(handle);

    while let Some(visited) = current {
      if visited == self.root {
        return true;
      }

      current = self.parent(visited);
    }

    false
  }

  /// The nearest laid-out ancestor of `handle`: the node whose taffy children include
  /// it.
  pub(crate) fn layout_parent(&self, handle: Handle) -> Option<Handle> {
    let mut current = self.parent(handle);

    while let Some(visited) = current {
      if self
        .record(visited)
        .is_some_and(|record| record.kind.is_laid_out())
      {
        return Some(visited);
      }

      current = self.parent(visited);
    }

    None
  }

  /// The laid-out descendants of `handle` as the layout tree sees them: anchors and
  /// scopes flattened into their content, overlays left out.
  pub(crate) fn layout_children(&self, handle: Handle) -> Vec<Handle> {
    let mut out = Vec::new();

    self.collect_layout_children(handle, &mut out);

    out
  }

  fn collect_layout_children(&self, handle: Handle, out: &mut Vec<Handle>) {
    for child in self.children(handle) {
      let Some(record) = self.record(*child)
      else {
        continue;
      };

      if record.is_overlay() {
        continue;
      }

      if record.kind.is_laid_out() {
        out.push(*child);
      }
      else {
        self.collect_layout_children(*child, out);
      }
    }
  }

  /// The nearest control at or above `handle`.
  pub(crate) fn enclosing_control(&self, handle: Handle) -> Option<Handle> {
    let mut current = Some(handle);

    while let Some(visited) = current {
      let record = self.record(visited)?;

      if record.is_control() {
        return Some(visited);
      }

      current = record.parent;
    }

    None
  }

  /// The node a press or focus on `handle` lands on: itself when focusable, else its
  /// enclosing control.
  pub(crate) fn focus_holder(&self, handle: Handle) -> Option<Handle> {
    if self.record(handle).is_some_and(Record::is_focusable) {
      return Some(handle);
    }

    self.enclosing_control(handle)
  }

  /// The interaction state `handle`'s conditions resolve against now.
  pub(crate) fn interaction(&self, handle: Handle) -> Interaction {
    let control = self.enclosing_control(handle);
    let holder = control.unwrap_or(handle);

    Interaction {
      hovered: self.hovered_set.contains(&handle),
      focused: self.focused == Some(holder),
      pressed: self.left_held
        && self
          .press
          .as_ref()
          .is_some_and(|press| press.target == Some(holder)),
      control: control.and_then(|control| match &self.record(control)?.kind {
        Kind::Control { state, .. } => Some(*state),
        _ => None,
      }),
    }
  }

  /// The named containers above `handle`, nearest first, with their last measured
  /// sizes.
  pub(crate) fn containers_above(&self, handle: Handle) -> Vec<(ContainerName, Option<Size<u32>>)> {
    let mut names = Vec::new();
    let mut current = self.parent(handle);

    while let Some(ancestor) = current {
      if let Some(name) = self.record(ancestor).and_then(Record::container) {
        names.push((name, self.container_sizes.get(&name).copied()));
      }

      current = self.parent(ancestor);
    }

    names
  }

  /// The hover path, from the root to the deepest node under the mouse.
  #[inline(always)]
  pub(crate) fn hovered(&self) -> &[Handle] {
    &self.hovered
  }

  /// Replaces the hover path, returning the previous one.
  pub(crate) fn set_hovered(&mut self, next: Vec<Handle>) -> Vec<Handle> {
    self.hovered_set.clear();
    self.hovered_set.extend(next.iter().copied());

    std::mem::replace(&mut self.hovered, next)
  }

  /// Marks `handle` for repaint: the rows it covered at the last paint and the rows it
  /// covers now are rewritten at the next frame.
  #[inline(always)]
  pub(crate) fn damage(&mut self, handle: Handle) {
    self.damaged.push(handle);
  }

  /// Takes the nodes and rectangles marked for repaint.
  pub(crate) fn take_damage(&mut self) -> (Vec<Handle>, Vec<CellRect>) {
    (
      std::mem::take(&mut self.damaged),
      std::mem::take(&mut self.damaged_rects),
    )
  }

  /// Whether anything is left to paint: a whole-frame repaint, a layout or structure
  /// change, or a damaged node.
  #[inline(always)]
  pub(crate) fn needs_frame(&self) -> bool {
    self.paint_dirty
      || self.layout_dirty
      || self.structure_dirty
      || self.motion_active
      || !self.damaged.is_empty()
      || !self.damaged_rects.is_empty()
  }

  /// The clock reading motion samples against, in seconds.
  #[inline(always)]
  pub(crate) fn now(&self) -> f64 {
    self.clock.now()
  }

  /// Whether the scheme in force is dark: the app's choice when it has made one, and
  /// the background the terminal reported otherwise.
  #[inline(always)]
  pub(crate) fn dark(&self) -> bool {
    match self.choice {
      Some(ColorScheme::Light) => false,
      Some(ColorScheme::Dark) => true,
      None => self.detected_dark,
    }
  }

  /// The color scheme in force.
  #[inline(always)]
  pub(crate) fn scheme(&self) -> ColorScheme {
    if self.dark() {
      ColorScheme::Dark
    }
    else {
      ColorScheme::Light
    }
  }

  /// The viewport as the last layout measured it.
  #[inline(always)]
  pub(crate) fn viewport(&self) -> Size<u32> {
    Size::new(u32::from(self.last_size.0), u32::from(self.last_size.1))
  }

  /// Marks every resolved style stale and the frame in need of layout.
  #[inline(always)]
  pub(crate) fn invalidate_styles(&mut self) {
    self.generation += 1;
    self.layout_dirty = true;
    self.paint_dirty = true;
  }

  /// The environment's signals, created on first use under a root owner of their own.
  pub(crate) fn media(&mut self) -> &MediaState {
    let viewport = self.viewport();
    let scheme = self.scheme();
    let reduced = self.reduced_motion;

    self.media.get_or_insert_with(|| {
      let owner = Owner::new();

      owner.enter(|| {
        let (viewport, set_viewport) = Signal::opaque(viewport).split();
        let (scheme, set_scheme) = Signal::opaque(scheme).split();
        let (reduced_motion, set_reduced_motion) = Signal::opaque(reduced).split();

        MediaState {
          owner,
          set_viewport,
          set_scheme,
          set_reduced_motion,
          signals: MediaSignals {
            viewport,
            scheme,
            reduced_motion,
            container_sizes: ContainerSizes::new(),
          },
        }
      })
    })
  }

  /// The owner of the environment's signals, for a teardown to dispose.
  pub(crate) fn take_media_owner(&mut self) -> Option<Owner> {
    self.media.take().map(|media| media.owner)
  }

  /// A node's laid-out size, in cells.
  pub(crate) fn node_size(&self, handle: Handle) -> (i64, i64) {
    let Some(taffy) = self.record(handle).and_then(|record| record.taffy)
    else {
      return (0, 0);
    };

    match self.taffy.layout(taffy) {
      #[expect(clippy::cast_possible_truncation)]
      Ok(layout) => (
        layout.size.width.round().max(0.) as i64,
        layout.size.height.round().max(0.) as i64,
      ),
      Err(_) => (0, 0),
    }
  }

  /// A node's laid-out position within its layout parent, in cells.
  pub(crate) fn node_location(&self, handle: Handle) -> (i64, i64) {
    let Some(taffy) = self.record(handle).and_then(|record| record.taffy)
    else {
      return (0, 0);
    };

    match self.taffy.layout(taffy) {
      #[expect(clippy::cast_possible_truncation)]
      Ok(layout) => (
        layout.location.x.round() as i64,
        layout.location.y.round() as i64,
      ),
      Err(_) => (0, 0),
    }
  }

  /// A node's border widths, in cells: left, top, right, bottom.
  pub(crate) fn node_border(&self, handle: Handle) -> (i64, i64, i64, i64) {
    let Some(taffy) = self.record(handle).and_then(|record| record.taffy)
    else {
      return (0, 0, 0, 0);
    };

    match self.taffy.layout(taffy) {
      #[expect(clippy::cast_possible_truncation)]
      Ok(layout) => (
        layout.border.left.round() as i64,
        layout.border.top.round() as i64,
        layout.border.right.round() as i64,
        layout.border.bottom.round() as i64,
      ),
      Err(_) => (0, 0, 0, 0),
    }
  }

  /// A node's size inside its border, in cells: what its size observers hear.
  pub(crate) fn client_size(&self, handle: Handle) -> Size<u32> {
    let (width, height) = self.node_size(handle);
    let (left, top, right, bottom) = self.node_border(handle);

    Size::new(
      u32::try_from(width - left - right).unwrap_or(0),
      u32::try_from(height - top - bottom).unwrap_or(0),
    )
  }
}
