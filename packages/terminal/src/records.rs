//! The records beside the scene: what the platform keeps per node, the layout tree,
//! the interaction state, and the environment it reports.

mod environment;
mod field;
mod frames;
mod geometry;
mod input;
mod record;
mod structure;

pub(crate) use skene_foundation::Interaction;
use skene_foundation::{
  Axis,
  BoxFacts,
  ColorScheme,
  Layout,
  NodeId,
  NodeMap,
  PointerButtons,
  Scene,
  Shortcut,
  Timers,
};
use skene_layout::{Restyle, Tree};

use self::environment::MediaState;
pub(crate) use self::{
  environment::Contrast,
  field::{EntryEditor, EntryState, Field, Visible},
  frames::{Clock, FRAME_INTERVAL},
  input::{AUTOSCROLL_INTERVAL, CELL_CENTER, Drag, Press, SCROLL_GESTURE, ScrollLatch},
  record::{
    Backdrop,
    BackdropKey,
    BoxCache,
    BoxRecord,
    KeptStyle,
    Kind,
    PictureRecord,
    PlainBox,
    PlainRun,
    Record,
    RunCache,
  },
};
use crate::{Clipboard, Terminal, layout::CellLeaf, paint::Buffer, selection::Selection};

/// The size the platform reports until a frame is rendered.
pub(crate) const DEFAULT_SIZE: (u16, u16) = (80, 24);

/// A cell's size in pixels where the terminal reports none.
pub(crate) const DEFAULT_CELL_PIXELS: (u16, u16) = (10, 20);

/// A rectangle of cells on the screen: its origin, then its size.
pub(crate) type CellRect = (i64, i64, i64, i64);

/// A node's record's key: the scene's own id for the node.
pub(crate) type Handle = NodeId;

/// Every record the platform holds, the layout tree, the interaction state and the
/// environment.
#[expect(
  clippy::struct_excessive_bools,
  reason = "each flag is an independent fact about the frame or the pointer"
)]
pub(crate) struct Records {
  /// The records, by node.
  by_node: NodeMap<Record>,
  /// The layout tree; a text leaf carries its flow as context, which measures it and
  /// which painting and hit testing read the text's layout from.
  pub(crate) tree: Tree<CellLeaf>,
  /// The root box, filling the screen.
  pub(crate) root: Handle,
  /// Every overlay, in build order; later entries stack on top.
  pub(crate) overlays: Vec<Handle>,
  /// Every text entry, in build order.
  pub(crate) entries: Vec<Handle>,
  /// The nodes whose paint changed since the last frame; each repaints the rows it
  /// covered and covers.
  damaged: Vec<Handle>,
  /// Screen rectangles to repaint that no live node covers any more.
  damaged_rects: Vec<CellRect>,
  /// The last painted frame, which the next paints over where nothing changed.
  pub(crate) last_frame: Option<Buffer>,
  /// The rows the last paint rewrote.
  pub(crate) repainted: Vec<usize>,
  /// The drag in progress.
  pub(crate) drag: Option<Drag>,
  /// The left press awaiting its release.
  pub(crate) press: Option<Press>,
  /// The buttons whose press has been delivered and not yet released; a release
  /// arriving without its press stands in for a click whose press the terminal
  /// swallowed.
  pub(crate) buttons: PointerButtons,
  /// The selection over the document's runs; a text entry's lives in its [`Field`].
  pub(crate) selection: Option<Selection>,
  /// The last press, for counting multi-click sequences: its clock reading, cell, and
  /// count.
  pub(crate) last_press: Option<(f64, u16, u16, u8)>,
  /// The pointer's last known cell.
  pub(crate) last_pointer: (u16, u16),
  /// Where the pointer sits within its cell, `0..1` along each axis: the center unless
  /// the terminal reports pixels.
  pub(crate) pointer_within: (f32, f32),
  /// The wheel gesture's latched scroll target.
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
  /// The scheduled actions, each due at a clock reading.
  pub(crate) timers: Timers<Box<dyn FnOnce()>>,
  /// The node to scroll into view once the next layout is computed.
  pub(crate) reveal: Option<Handle>,
  /// The window title the document asks for.
  pub(crate) title: Option<String>,
  /// Whether the title changed since the terminal was last told it.
  pub(crate) title_dirty: bool,
  /// How many layouts have been computed.
  pub(crate) layout_generation: u64,
  /// How many taffy styles have been written.
  pub(crate) set_style_calls: u64,
  /// How many node styles have been resolved from their declarations.
  pub(crate) style_resolutions: u64,
  /// Whether anything the layout reads changed since it was last computed.
  pub(crate) layout_dirty: bool,
  /// Whether the whole frame repaints; a change confined to a node damages it instead.
  pub(crate) paint_dirty: bool,
  /// Whether a node's screen rectangle may have moved since the last paint measured
  /// them: the layout, a scroll offset or a shadow's reach changed.
  pub(crate) covers_stale: bool,
  /// What moved since the last restyle: the resolution generation, moving when
  /// definitions, the environment or a control's state change, and the nodes the next
  /// restyle revisits.
  pub(crate) restyle: Restyle,
  /// Whether a key asked the app to quit.
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
  /// The environment's signals, once first asked for.
  media: Option<MediaState>,
  /// Whether the terminal proved a graphics protocol, so pictures render and place.
  pub(crate) graphics: bool,
  /// Whether the terminal's clipboard answers (mode 5522), so every operation is
  /// offered from its reply rather than the launch choice.
  pub(crate) clipboard_answers: bool,
  /// Whether the terminal reports key releases (the kitty keyboard protocol's event
  /// types, confirmed by the flags query), so a release no longer follows each press
  /// at once.
  pub(crate) key_releases: bool,
  /// A cell's size in pixels: the window's where the terminal reports it, else a
  /// stand-in the terminal scales a picture from.
  pub(crate) cell_pixels: (u16, u16),
  /// The generation the last rendered picture took; each render takes the next.
  pub(crate) picture_generation: u64,
}

impl Records {
  /// Records whose root box, filling the screen, is the scene's node `root`.
  pub(crate) fn new(root: NodeId) -> Self {
    let root_facts = BoxFacts {
      layout: Layout::Flex {
        axis: Axis::Vertical,
        reversed: false,
      },
      container: None,
      transition_name: None,
      link: false,
      acting: false,
    };
    let mut records = Self {
      by_node: NodeMap::new(),
      tree: Tree::new(true),
      root,
      overlays: Vec::new(),
      entries: Vec::new(),
      damaged: Vec::new(),
      damaged_rects: Vec::new(),
      last_frame: None,
      repainted: Vec::new(),
      drag: None,
      press: None,
      buttons: PointerButtons::new(),
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
      timers: Timers::new(),
      reveal: None,
      title: None,
      title_dirty: false,
      layout_generation: 0,
      set_style_calls: 0,
      style_resolutions: 0,
      layout_dirty: true,
      paint_dirty: true,
      covers_stale: true,
      restyle: Restyle::new(),
      quit: false,
      clock: Clock::Manual(0.),
      reduced_motion: false,
      motion_active: false,
      autoscroll_at: None,
      media: None,
      graphics: false,
      clipboard_answers: false,
      key_releases: false,
      cell_pixels: DEFAULT_CELL_PIXELS,
      picture_generation: 0,
    };

    records.insert(root, Kind::Box(BoxRecord::new(None, None, root_facts)));

    records
  }

  /// Stores a record of `kind` as the record of `node`, a fresh leaf of the layout
  /// tree.
  pub(crate) fn insert(&mut self, node: NodeId, kind: Kind) {
    let mut record = Record::new(kind);

    record.node = Some(node);
    self.tree.insert(node);
    self.restyle.move_structure();

    let _replaced = self.by_node.insert(node, record);
  }

  /// The record of `handle`, if it is live.
  #[inline(always)]
  pub(crate) fn record(&self, handle: Handle) -> Option<&Record> {
    self.by_node.get(handle)
  }

  /// The record of `handle`, mutably, if it is live.
  #[inline(always)]
  pub(crate) fn record_mut(&mut self, handle: Handle) -> Option<&mut Record> {
    self.by_node.get_mut(handle)
  }

  /// Replaces the run `handle`'s text, its selection, style and layout made afresh;
  /// nothing happens where it reads `text` already.
  pub(crate) fn write_text(&mut self, handle: Handle, text: &str) {
    let Some(record) = self.record_mut(handle)
    else {
      return;
    };

    if record.text == text {
      return;
    }

    text.clone_into(&mut record.text);
    self.invalidate_selection(handle);
    self.queue_restyle(handle);
    self.layout_dirty = true;
    self.damage(handle);
  }

  /// Queues `handle` for the next restyle, which resolves it where its resolution is
  /// stale and hands the layout what it measures by.
  #[inline(always)]
  pub(crate) fn queue_restyle(&mut self, handle: Handle) {
    self.restyle.queue(handle);
  }

  /// Measures every node's text again, after the terminal's clustering or sizing
  /// changed: each entry's value laid out afresh at its next query, and each laid-out
  /// node's flow made afresh at its next restyle.
  pub(crate) fn remeasure(&mut self) {
    self.restyle.revisit_every();

    for handle in self.handles() {
      let Some(record) = self.record_mut(handle)
      else {
        continue;
      };

      if record.kind.field().is_some()
        && let Some(entry) = Scene::<Terminal>::entry_of(handle)
        && let Ok(mut editor) = entry.editor().try_borrow_mut()
      {
        editor.relayout();
      }

      self.tree.mark_dirty(handle);
    }
  }

  /// Every live handle, in index order.
  pub(crate) fn handles(&self) -> Vec<Handle> {
    self.by_node.iter().map(|(handle, _)| handle).collect()
  }

  /// How many nodes the platform holds a record of.
  pub(crate) fn record_count(&self) -> usize {
    self.by_node.values().count()
  }

  /// Releases the record of `handle`, retiring the handle and every reference to it.
  pub(crate) fn destroy(&mut self, handle: Handle) {
    let Some(record) = self.by_node.remove(handle)
    else {
      return;
    };

    self.tree.remove(handle);

    self.overlays.retain(|overlay| *overlay != handle);
    self.entries.retain(|entry| *entry != handle);
    self.restyle.read_interaction(handle, false);

    if let Some(rect) = record.painted {
      self.damaged_rects.push(rect);
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
      .is_some_and(|press| press.pressed == handle || press.captured == Some(handle))
    {
      self.press = None;
    }

    self.invalidate_selection(handle);
    self.restyle.move_structure();
    self.paint_dirty = true;
  }
}
