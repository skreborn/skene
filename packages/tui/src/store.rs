//! The record store beside the scene: what the platform keeps per node, the taffy tree,
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
  Shortcut,
  Size,
  Slot,
  TextSelection,
  Timer,
};
use skene_reactive::{Owner, ReadSignal, Signal, WriteSignal};

use crate::{
  Clipboard,
  Tui,
  TuiExt,
  field::Area,
  layout::Measured,
  motion::{BoxMotion, RunMotion},
  paint::{Buffer, Image},
  selection::Selection,
  text::{self, Clusters},
};

/// The size the platform reports until a frame is rendered.
pub(crate) const DEFAULT_SIZE: (u16, u16) = (80, 24);

/// The colors an unset cell shows: the ink and the base a paint mixes against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Contrast {
  pub(crate) ink: (u8, u8, u8),
  pub(crate) base: (u8, u8, u8),
}

/// A cell's size in pixels where the terminal reports none.
pub(crate) const DEFAULT_CELL_PIXELS: (u16, u16) = (10, 20);

/// A box's style resolved against live state.
pub(crate) type PlainBox = BoxStyle<u32, Plain, TuiExt>;

/// A run's style resolved against live state.
pub(crate) type PlainRun = RunStyle<Plain, TuiExt>;

/// A rectangle of cells on the screen: its origin, then its size.
pub(crate) type CellRect = (i64, i64, i64, i64);

/// The platform's sink: the index of a node's record and the generation it was minted
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

/// A property's value erased over the property; see [`skene_core::SlotValue`].
pub(crate) type SlotValue = skene_core::SlotValue<u32, TuiExt>;

/// What a derived leaf last wrote into a node's slot; see [`skene_core::SlotLeaf`].
pub(crate) type SlotLeaf = skene_core::SlotLeaf<u32, TuiExt>;

pub(crate) use skene_core::{Focus, Interaction};

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

/// The center of a cell, where a pointer reported by cell alone is taken to be.
pub(crate) const CELL_CENTER: (f32, f32) = (0.5, 0.5);

/// How many undo steps an entry keeps.
const HISTORY_LIMIT: usize = 100;

/// One state of an entry an undo or redo returns to.
struct Snapshot {
  value: String,
  caret: usize,
  anchor: Option<usize>,
}

/// A kind of edit whose consecutive occurrences make one undo step.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EditRun {
  /// A cluster typed at the caret.
  Typed,
  /// A cluster removed beside the caret.
  Removed,
}

/// A text entry's editing state; the caret and the anchor are cluster positions in
/// the value, `0..=count`.
pub(crate) struct Field {
  pub(crate) value: String,
  /// The value's clusters, kept with it.
  pub(crate) clusters: Clusters,
  /// The selection's moving end, and the insertion point.
  pub(crate) caret: usize,
  /// The selection's fixed end, `None` while the selection is collapsed.
  pub(crate) anchor: Option<usize>,
  /// The selection last reported or written from outside.
  reported: TextSelection,
  /// The states an undo returns to, oldest first, at most [`HISTORY_LIMIT`].
  undo: Vec<Snapshot>,
  /// The states a redo returns to, oldest first; emptied by an edit.
  redo: Vec<Snapshot>,
  /// The kind of the last edit, which the next edit of the same kind joins as one undo
  /// step; `None` once the caret has moved or the selection changed since.
  run: Option<EditRun>,
  /// The first visible column of a field, or the first visible wrapped row of an
  /// editor; moves only to keep the caret in view, under an autoscroll, or under an
  /// editor's wheel.
  pub(crate) window: usize,
  /// The area the window was last settled or scrolled in, `None` before the first; in
  /// any other the window settles around the caret before it shows.
  pub(crate) window_area: Option<Area>,
  /// The cell column vertical motion aims for, held across consecutive vertical moves.
  pub(crate) goal: Option<usize>,
  /// The text shown while the value is empty.
  pub(crate) placeholder: Option<String>,
  /// The rows the entry asks the layout for; more than one makes it an editor.
  pub(crate) lines: u16,
}

impl Field {
  /// A fresh, empty entry showing `lines` rows.
  pub(crate) const fn new(placeholder: Option<String>, lines: u16) -> Self {
    Self {
      value: String::new(),
      clusters: Clusters::EMPTY,
      caret: 0,
      anchor: None,
      reported: TextSelection::collapsed(0),
      undo: Vec::new(),
      redo: Vec::new(),
      run: None,
      window: 0,
      window_area: None,
      goal: None,
      placeholder,
      lines: if lines == 0 { 1 } else { lines },
    }
  }

  /// The value's length in clusters.
  pub(crate) fn count(&self) -> usize {
    self.clusters.count()
  }

  /// Measures the value's clusters again, after the value or the terminal's
  /// clustering changed.
  pub(crate) fn reflow(&mut self) {
    self.clusters = Clusters::of(&self.value);
  }

  /// The selected cluster range in document order, `None` while collapsed.
  pub(crate) fn selection(&self) -> Option<(usize, usize)> {
    let anchor = self.anchor?;

    (anchor != self.caret).then(|| (anchor.min(self.caret), anchor.max(self.caret)))
  }

  /// The selected text, `None` while collapsed.
  pub(crate) fn selected_text(&self) -> Option<String> {
    let (from, to) = self.selection()?;

    Some(self.value[self.byte_of(from)..self.byte_of(to)].to_owned())
  }

  /// The selection as the app sees it: from the anchor, or the caret while collapsed,
  /// to the caret.
  pub(crate) fn selection_offsets(&self) -> TextSelection {
    TextSelection::new(self.anchor.unwrap_or(self.caret), self.caret)
  }

  /// The selection, when it differs from the one last reported or written from
  /// outside, which it becomes.
  pub(crate) fn selection_report(&mut self) -> Option<TextSelection> {
    let selection = self.selection_offsets();

    if selection == self.reported {
      return None;
    }

    self.reported = selection;

    Some(selection)
  }

  /// Places the selection from outside the entry's editing, each index clamped to the
  /// value; `false` when it is the selection already.
  pub(crate) fn set_selection(&mut self, selection: TextSelection) -> bool {
    let selection = selection.clamped(self.count());
    let caret = selection.end;
    let anchor = (selection.start != caret).then_some(selection.start);

    if (caret, anchor) == (self.caret, self.anchor) {
      self.reported = self.selection_offsets();

      return false;
    }

    self.caret = caret;
    self.anchor = anchor;
    self.run = None;
    self.goal = None;
    self.reported = self.selection_offsets();

    true
  }

  /// Collapses the selection at the caret.
  pub(crate) const fn collapse(&mut self) {
    self.anchor = None;
    self.run = None;
    self.goal = None;
  }

  /// Selects `from..to`, the caret at `to`.
  pub(crate) fn select(&mut self, from: usize, to: usize) {
    let count = self.count();

    self.caret = to.min(count);
    self.anchor = (from != to).then_some(from.min(count));
    self.run = None;
    self.goal = None;
  }

  /// Sets the value from outside the entry's editing, sanitized as
  /// [`sanitized`](Self::sanitized) says, as one undo step, the caret clamped and the
  /// selection collapsed; a value over an empty entry with no history is the initial
  /// one and records nothing, and nothing happens when `value` is the value already.
  pub(crate) fn set_value(&mut self, value: &str) -> bool {
    let value = self.sanitized(value);

    if self.value == value {
      return false;
    }

    if !(self.value.is_empty() && self.undo.is_empty() && self.redo.is_empty()) {
      self.record(None);
    }

    self.value = value;
    self.reflow();
    self.caret = self.caret.min(self.count());
    self.anchor = None;
    self.goal = None;

    true
  }

  /// Records the state before an edit of `kind` as the next undo step, unless the
  /// edit joins the run of the last one; either way the redo states are dropped.
  fn record(&mut self, kind: Option<EditRun>) {
    self.redo.clear();

    if kind.is_none() || kind != self.run {
      if self.undo.len() == HISTORY_LIMIT {
        self.undo.remove(0);
      }

      self.undo.push(self.snapshot());
    }

    self.run = kind;
  }

  /// Returns to the state before the last undo step, the state left becoming a redo
  /// step; `false` with nothing to undo.
  pub(crate) fn undo(&mut self) -> bool {
    let Some(snapshot) = self.undo.pop()
    else {
      return false;
    };

    self.redo.push(self.snapshot());
    self.restore(snapshot);

    true
  }

  /// Returns to the state the last undo left, that state becoming an undo step again;
  /// `false` with nothing to redo.
  pub(crate) fn redo(&mut self) -> bool {
    let Some(snapshot) = self.redo.pop()
    else {
      return false;
    };

    self.undo.push(self.snapshot());
    self.restore(snapshot);

    true
  }

  fn snapshot(&self) -> Snapshot {
    Snapshot {
      value: self.value.clone(),
      caret: self.caret,
      anchor: self.anchor,
    }
  }

  fn restore(&mut self, snapshot: Snapshot) {
    self.value = snapshot.value;
    self.reflow();
    self.caret = snapshot.caret;
    self.anchor = snapshot.anchor;
    self.run = None;
    self.goal = None;
  }

  /// Selects the whole value.
  pub(crate) fn select_all(&mut self) {
    self.select(0, self.count());
  }

  /// Moves the caret to `caret`, extending the selection from the anchor—the caret's
  /// old position when none—or collapsing it.
  pub(crate) fn move_caret(&mut self, caret: usize, extend: bool) {
    let count = self.count();

    if extend {
      let anchor = self.anchor.unwrap_or(self.caret);

      self.anchor = (anchor != caret).then_some(anchor);
    }
    else {
      self.anchor = None;
    }

    self.caret = caret.min(count);
    self.run = None;
    self.goal = None;
  }

  /// Replaces the selection—or inserts at the caret—with `text`, leaving the caret
  /// after it. One cluster inserted at the caret joins the run of clusters typed
  /// before it as one undo step; anything else is a step of its own.
  pub(crate) fn replace_selection(&mut self, text: &str) {
    let selection = self.selection();
    let (from, to) = selection.unwrap_or((self.caret, self.caret));
    let typed = selection.is_none() && text::count(text) == 1;
    let start = self.byte_of(from);

    self.record(typed.then_some(EditRun::Typed));
    self.value.replace_range(start..self.byte_of(to), text);
    self.reflow();
    self.caret = self.clusters.index_of_byte(start + text.len());
    self.anchor = None;
    self.goal = None;
  }

  /// Removes `from..to`, leaving the caret at `from`. One cluster removed beside a
  /// collapsed caret joins the run of clusters removed before it as one undo step;
  /// anything else is a step of its own.
  pub(crate) fn remove(&mut self, from: usize, to: usize) {
    let single = self.selection().is_none() && to == from + 1;
    let start = self.byte_of(from);

    self.record(single.then_some(EditRun::Removed));
    self.value.replace_range(start..self.byte_of(to), "");
    self.reflow();
    self.caret = self.clusters.index_of_byte(start);
    self.anchor = None;
    self.goal = None;
  }

  /// The byte offset of the cluster at `position`, or the value's end.
  pub(crate) fn byte_of(&self, position: usize) -> usize {
    self.clusters.byte_of(position)
  }
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
    /// The cell the overlay's top-start corner is pinned to, if it is.
    position: Option<(i64, i64)>,
    /// The node focused when the overlay opened, for its close to return to.
    return_focus: Option<Handle>,
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
  /// Forgets what the node presented and the motion it ran, so it presents afresh: its
  /// animations from their start, and no transition into its style.
  pub(crate) fn forget_presentation(&mut self) {
    match self {
      Self::Run { cache, motion, .. } => {
        *cache = None;
        *motion = RunMotion::default();
      }
      kind => {
        if let Some(inner) = kind.box_record_mut() {
          inner.cache = None;
          inner.motion = BoxMotion::default();
        }
      }
    }
  }

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
  /// The text and scales the layout tree carries for a run or a paragraph.
  pub(crate) measured_text: Option<Measured>,
  pub(crate) applied: Option<taffy::Style>,
  pub(crate) text: String,
  pub(crate) slots: FxHashMap<Slot, SlotLeaf>,
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
  /// Whether the node was out of the display at the last restyle: inside a closed
  /// overlay, or detached from the root.
  pub(crate) hidden: bool,
  /// The painter a low-level view runs inside the box on every repaint.
  pub(crate) painter: Option<Box<crate::cells::Painter>>,
  /// The picture a painted box draws through the terminal's graphics.
  pub(crate) picture: Option<PictureRecord>,
  /// The box's decoration painted in pixels beneath its cells, where the terminal
  /// shows graphics and the decoration asks for more than cells draw.
  pub(crate) backdrop: Option<Backdrop>,
}

/// A box's decoration as pixels: what it was rendered from, and the image.
pub(crate) struct Backdrop {
  pub(crate) key: BackdropKey,
  pub(crate) image: Rc<Image>,
}

/// What a backdrop is rendered from; a change to any part renders it afresh.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BackdropKey {
  /// The box's cells, and one more each way for the shadow's band.
  pub(crate) columns: u16,
  pub(crate) rows: u16,
  pub(crate) cell_pixels: (u16, u16),
  pub(crate) background: Option<(u8, u8, u8)>,
  pub(crate) border: Option<(u8, u8, u8)>,
  pub(crate) radius: f32,
  pub(crate) shadow: Option<[f32; 4]>,
}

/// A painted box's picture: its painter, and the image last rendered from it, dropped
/// when the painter is replaced and stale when the box's pixel size differs.
pub(crate) struct PictureRecord {
  pub(crate) painter: Rc<skene_common::Painter>,
  pub(crate) image: Option<Rc<Image>>,
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
      hidden: false,
      painter: None,
      picture: None,
      backdrop: None,
    }
  }

  /// The box props of a box, a control or an overlay.
  #[inline(always)]
  pub(crate) fn box_props(&self) -> Option<&BoxProps<Tui>> {
    self.kind.box_record().map(|inner| &inner.props)
  }

  /// The record's place in focus traversal: a box's declared order, a link run's
  /// automatic one.
  #[inline(always)]
  pub(crate) fn focus_order(&self) -> Option<FocusOrder> {
    match &self.kind {
      Kind::Run { props, .. } => props.link.as_ref().map(|_| FocusOrder::Auto),
      _ => self.box_props().and_then(|props| props.focus_order),
    }
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

  /// Whether the record is a modal overlay.
  #[inline(always)]
  pub(crate) const fn is_modal_overlay(&self) -> bool {
    matches!(self.kind, Kind::Overlay { modal: true, .. })
  }

  /// The anchor an open overlay is placed against.
  #[inline(always)]
  pub(crate) const fn overlay_anchor(&self) -> Option<Handle> {
    match self.kind {
      Kind::Overlay { anchor, .. } => anchor,
      _ => None,
    }
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
    /// The grabbed position within the thumb, in track cells, exact where the
    /// terminal reports pixels.
    grab: f32,
  },
  /// A text selection: motion extends it to the position under the pointer.
  Selection,
  /// A text entry's caret: motion moves it to the position under the pointer, the
  /// anchor held.
  Caret(Handle),
}

/// The pause that ends a wheel gesture: the next wheel event re-targets, as on the
/// web.
pub(crate) const SCROLL_GESTURE: std::time::Duration = std::time::Duration::from_millis(500);

/// How long the loop waits for input between frames while the frame clock runs: the
/// frame rate's cap.
pub(crate) const FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

/// The interval between the steps of a caret drag's autoscroll while the pointer is
/// held past the entry's edge; each step scrolls by the pointer's distance past it.
pub(crate) const AUTOSCROLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

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

/// The environment as signals, with the write halves the platform drives.
pub(crate) struct MediaState {
  pub(crate) owner: Owner,
  pub(crate) set_viewport: WriteSignal<Size<u32>>,
  pub(crate) set_scheme: WriteSignal<ColorScheme>,
  pub(crate) set_reduced_motion: WriteSignal<bool>,
  /// Whether the terminal proved a graphics protocol.
  pub(crate) graphics: ReadSignal<bool>,
  pub(crate) set_graphics: WriteSignal<bool>,
  pub(crate) signals: MediaSignals<u32>,
}

/// Every record the platform holds, the layout tree, the interaction state and the
/// environment.
// Each flag is an independent fact about the frame or the pointer.
#[expect(clippy::struct_excessive_bools)]
pub(crate) struct Store {
  entries: Vec<Entry>,
  free: Vec<u32>,
  /// The layout tree; a text run's leaf carries its text and scales as context,
  /// wrapped by the layout's measure function.
  pub(crate) taffy: taffy::TaffyTree<Measured>,
  pub(crate) root: Handle,
  /// Every overlay, in build order; later entries stack on top.
  pub(crate) overlays: Vec<Handle>,
  pub(crate) focused: Option<Handle>,
  /// Whether focus shows while held: set by a key, cleared by a pointer press.
  pub(crate) focus_visible: bool,
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
  /// The selection over the document's runs; a text entry's lives in its [`Field`].
  pub(crate) selection: Option<Selection>,
  /// The last press, for counting multi-click sequences: its time, cell, and count.
  pub(crate) last_press: Option<(Instant, u16, u16, u8)>,
  /// The pointer's last known cell.
  pub(crate) last_pointer: (u16, u16),
  /// Where the pointer sits within its cell, `0..1` along each axis: the center unless
  /// the terminal reports pixels.
  pub(crate) pointer_within: (f32, f32),
  pub(crate) scroll_latch: Option<ScrollLatch>,
  /// The clipboard operations the launch offers.
  pub(crate) clipboard: Clipboard,
  /// Whether the launch takes the mouse; off, every mouse event is ignored.
  pub(crate) mouse: bool,
  /// The shortcut handing the mouse to the terminal at run time and taking it back.
  pub(crate) mouse_key: Option<Shortcut>,
  /// Whether the mouse key has handed the mouse to the terminal; every mouse event is
  /// ignored until a key takes it back.
  pub(crate) mouse_released: bool,
  /// Text staged for the terminal loop's clipboard write.
  pub(crate) pending_copy: Option<String>,
  /// Whether a clipboard read is staged for the terminal loop.
  pub(crate) pending_paste: bool,
  /// The entry the last clipboard read was asked for, where its reply lands.
  pub(crate) paste_target: Option<Handle>,
  /// The link pressed at its cell, opened when the release lands on the same cell.
  pub(crate) pressed_link: Option<(u16, u16, String)>,
  /// A link destination staged for the terminal loop to open.
  pub(crate) pending_open: Option<String>,
  /// The size of the last computed layout, which hit-testing resolves against.
  pub(crate) last_size: (u16, u16),
  /// Whether the terminal background is dark, as detected.
  pub(crate) detected_dark: bool,
  /// The terminal's own foreground, where it reported one (OSC 10).
  pub(crate) detected_foreground: Option<(u8, u8, u8)>,
  /// The terminal's own background, where it reported one (OSC 11).
  pub(crate) detected_background: Option<(u8, u8, u8)>,
  /// The color scheme the app has chosen, overriding the detected background.
  pub(crate) choice: Option<ColorScheme>,
  /// The scheduled actions, each with the clock reading it is due at.
  pub(crate) timers: Vec<Scheduled>,
  pub(crate) next_timer: u64,
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
  /// Whether a node's interaction state moved since the last restyle, which may change
  /// its style though nothing else changed.
  pub(crate) restyle_pending: bool,
  pub(crate) quit: bool,
  /// The clock motion samples against.
  pub(crate) clock: Clock,
  /// Whether the reader prefers reduced motion.
  pub(crate) reduced_motion: bool,
  /// Whether a tween or an animation was still running at the last restyle, so the
  /// next frame is due at the frame rate.
  pub(crate) motion_active: bool,
  /// The clock reading the caret drag's autoscroll last stepped at, `None` while no
  /// drag is held past its entry's edge.
  pub(crate) autoscroll_at: Option<f64>,
  media: Option<MediaState>,
  /// Whether the terminal proved a graphics protocol, so pictures render and place.
  pub(crate) graphics: bool,
  /// Whether the terminal's clipboard answers (mode 5522), so every operation is
  /// offered from its reply rather than the launch choice.
  pub(crate) clipboard_answers: bool,
  /// A cell's size in pixels: the window's where the terminal reports it, else a
  /// stand-in the terminal scales a picture from.
  pub(crate) cell_pixels: (u16, u16),
  /// The generation the last rendered picture took; each render takes the next.
  pub(crate) picture_generation: u64,
}

/// A scheduled action: the clock reading it is due at, its timer, and the action.
pub(crate) type Scheduled = (f64, Timer, Box<dyn FnOnce()>);

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
      focus_visible: true,
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
      selection: None,
      last_press: None,
      last_pointer: (0, 0),
      pointer_within: CELL_CENTER,
      scroll_latch: None,
      clipboard: Clipboard::ALL,
      mouse: true,
      mouse_key: None,
      mouse_released: false,
      pending_copy: None,
      pending_paste: false,
      paste_target: None,
      pressed_link: None,
      pending_open: None,
      last_size: DEFAULT_SIZE,
      detected_dark: true,
      detected_foreground: None,
      detected_background: None,
      choice: None,
      timers: Vec::new(),
      next_timer: 0,
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
      restyle_pending: false,
      quit: false,
      clock: Clock::Manual(0.),
      reduced_motion: false,
      motion_active: false,
      autoscroll_at: None,
      media: None,
      graphics: false,
      clipboard_answers: false,
      cell_pixels: DEFAULT_CELL_PIXELS,
      picture_generation: 0,
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

  /// The selection of every entry whose selection changed since it was last reported
  /// or written from outside, with the entry's node.
  pub(crate) fn selection_reports(&mut self) -> Vec<(NodeId, TextSelection)> {
    let mut reports = Vec::new();

    for handle in self.handles() {
      let Some(record) = self.record_mut(handle)
      else {
        continue;
      };
      let node = record.node;

      if let Some(selection) = record.kind.field_mut().and_then(Field::selection_report)
        && let Some(node) = node
      {
        reports.push((node, selection));
      }
    }

    reports
  }

  /// Measures every node's text again, after the terminal's clustering changed: each
  /// entry's clusters, and each laid-out node's measurement.
  pub(crate) fn remeasure(&mut self) {
    for handle in self.handles() {
      let Some(record) = self.record_mut(handle)
      else {
        continue;
      };
      let taffy = record.taffy;

      if let Some(field) = record.kind.field_mut() {
        field.reflow();
      }

      if let Some(taffy) = taffy {
        let _ = self.taffy.mark_dirty(taffy);
      }
    }
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

  /// The focused link run's node and destination, while a link holds focus.
  pub(crate) fn focused_link(&self) -> Option<(NodeId, String)> {
    let record = self.record(self.focused?)?;

    match &record.kind {
      Kind::Run { props, .. } => Some((record.node?, props.link.clone()?)),
      _ => None,
    }
  }

  /// Whether `handle` holds focus that shows: after a key, or as a text entry, and not
  /// since a pointer press.
  pub(crate) fn focus_shows(&self, handle: Handle) -> bool {
    self.focused == Some(handle) && (self.focus_visible || self.is_field(handle))
  }

  /// Records whether focus shows, repainting the focused node when that changes.
  pub(crate) fn set_focus_visible(&mut self, visible: bool) {
    if self.focus_visible == visible {
      return;
    }

    self.focus_visible = visible;

    if let Some(focused) = self.focused {
      self.damage(focused);
    }
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
      focus: Focus::new(self.focused == Some(holder), self.focus_shows(holder)),
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

  /// Whether the app holds the mouse now: taken at launch and not handed to the
  /// terminal since.
  #[inline(always)]
  pub(crate) const fn mouse_taken(&self) -> bool {
    self.mouse && !self.mouse_released
  }

  /// Every node in the display: reachable from the root through open overlays alone.
  pub(crate) fn displayed(&self) -> FxHashSet<Handle> {
    let mut displayed = FxHashSet::default();
    let mut stack = vec![self.root];

    while let Some(current) = stack.pop() {
      let Some(record) = self.record(current)
      else {
        continue;
      };

      if record.is_overlay() && !record.is_open_overlay() {
        continue;
      }

      displayed.insert(current);
      stack.extend(record.children.iter().copied());
    }

    displayed
  }

  /// The hover path, from the root to the deepest node under the mouse.
  #[inline(always)]
  pub(crate) fn hovered(&self) -> &[Handle] {
    &self.hovered
  }

  /// Replaces the hover path, returning the previous one; a changed path asks for a
  /// restyle, which repaints the nodes whose style it changed.
  pub(crate) fn set_hovered(&mut self, next: Vec<Handle>) -> Vec<Handle> {
    self.restyle_pending |= self.hovered != next;
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

  /// Whether anything is left to paint: a whole-frame repaint, a pending restyle, a
  /// layout or structure change, or a damaged node.
  #[inline(always)]
  pub(crate) fn needs_frame(&self) -> bool {
    self.paint_dirty
      || self.restyle_pending
      || self.layout_dirty
      || self.structure_dirty
      || self.frame_clock_running()
      || !self.damaged.is_empty()
      || !self.damaged_rects.is_empty()
  }

  /// Whether the next frame is due at the frame rate: a tween or an animation runs, or
  /// a caret drag is held past its entry's edge.
  #[inline(always)]
  pub(crate) fn frame_clock_running(&self) -> bool {
    self.motion_active || self.autoscrolls()
  }

  /// The scheduled actions whose reading has come, taken out in due order.
  pub(crate) fn take_due_timers(&mut self) -> Vec<Box<dyn FnOnce()>> {
    let now = self.now();
    let (due, pending): (Vec<_>, Vec<_>) = std::mem::take(&mut self.timers)
      .into_iter()
      .partition(|(at, ..)| *at <= now);

    self.timers = pending;

    due.into_iter().map(|(_, _, action)| action).collect()
  }

  /// How long until the earliest scheduled action is due, if any is; a delay past
  /// what a duration holds is the longest one.
  pub(crate) fn next_timer_due(&self) -> Option<std::time::Duration> {
    let now = self.now();

    self
      .timers
      .iter()
      .map(|(at, ..)| {
        std::time::Duration::try_from_secs_f64((at - now).max(0.))
          .unwrap_or(std::time::Duration::MAX)
      })
      .min()
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

  /// The colors an unset cell shows, which a paint mixes against: the terminal's
  /// own foreground and background where it reported them and the scheme in force is
  /// the detected one, else white on black for a dark scheme and black on white for a
  /// light one.
  pub(crate) fn contrast(&self) -> Contrast {
    let dark = self.dark();
    let defaults = if dark {
      Contrast {
        ink: (255, 255, 255),
        base: (0, 0, 0),
      }
    }
    else {
      Contrast {
        ink: (0, 0, 0),
        base: (255, 255, 255),
      }
    };

    if dark != self.detected_dark {
      return defaults;
    }

    Contrast {
      ink: self.detected_foreground.unwrap_or(defaults.ink),
      base: self.detected_background.unwrap_or(defaults.base),
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
    let graphics = self.graphics;

    self.media.get_or_insert_with(|| {
      let owner = Owner::new();

      owner.enter(|| {
        let (viewport, set_viewport) = Signal::opaque(viewport).split();
        let (scheme, set_scheme) = Signal::opaque(scheme).split();
        let (reduced_motion, set_reduced_motion) = Signal::opaque(reduced).split();
        let (graphics, set_graphics) = Signal::opaque(graphics).split();

        MediaState {
          owner,
          set_viewport,
          set_scheme,
          set_reduced_motion,
          graphics,
          set_graphics,
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

  /// A childless box's laid-out span in eighth cells from its parent's origin: where
  /// it begins and how far it extends, unrounded, so boxes beside each other meet
  /// where the layout put them. `None` for a box with laid-out children, or one of no
  /// width.
  pub(crate) fn smooth_span(&self, handle: Handle) -> Option<(isize, isize)> {
    let record = self.record(handle)?;
    let laid_out = |handle: Handle| {
      self
        .record(handle)
        .is_some_and(|record| record.kind.is_laid_out())
    };

    if record.children.iter().any(|child| laid_out(*child)) {
      return None;
    }

    let layout = self.taffy.unrounded_layout(record.taffy?);
    #[expect(clippy::cast_possible_truncation)]
    let start = (layout.location.x * 8.).round() as isize;
    #[expect(clippy::cast_possible_truncation)]
    let extent = (layout.size.width.max(0.) * 8.).round() as isize;

    (extent > 0).then_some((start, extent))
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

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use skene_core::Platform as _;

  use crate::{Tui, with_store};

  /// A delay past what a duration holds waits the longest a duration can.
  #[test]
  fn a_huge_delay_waits_the_longest() {
    crate::install();

    let _ = Tui.schedule(Duration::MAX, Box::new(|| {}));

    assert_eq!(
      with_store(|store| store.next_timer_due()),
      Some(Duration::MAX)
    );

    crate::uninstall();
  }
}
