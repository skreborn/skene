//! The terminal's state beside the scene: the drawn runtime with its records and its
//! layout tree, each node's cell state carried in its record, the interaction state, and
//! the environment it reports.

mod cell;
mod environment;
mod field;
mod frames;
mod geometry;
mod input;

use skene_foundation::{NodeId, PointerButtons, Scene, Shortcut, Timers};
use skene_layout::runtime::Runtime;

pub(crate) use self::{
  cell::{Backdrop, BackdropKey, CellNode, PictureRecord},
  environment::{Contrast, OwnColors},
  field::{EntryEditor, Field, Visible},
  frames::FRAME_INTERVAL,
  geometry::CellFrame,
  input::{AUTOSCROLL_INTERVAL, CELL_CENTER, Drag, Press, SCROLL_GESTURE, ScrollLatch},
};
use crate::{Clipboard, Terminal, detect::Palette, paint::Buffer, selection::Selection};

/// The size the platform reports until a frame is rendered.
pub(crate) const DEFAULT_SIZE: (u16, u16) = (80, 24);

/// A cell's size in pixels where the terminal reports none.
pub(crate) const DEFAULT_CELL_PIXELS: (u16, u16) = (10, 20);

/// A rectangle of cells on the screen: its origin, then its size.
pub(crate) type CellRect = (i64, i64, i64, i64);

/// One realized node, as the drawn runtime keeps it.
pub(crate) type Record = skene_layout::runtime::Record<Terminal>;

/// What a record is, with everything its creation carried.
pub(crate) type Kind = skene_layout::runtime::Kind<Terminal>;

/// A box's style resolved against live state.
pub(crate) type PlainBox = skene_layout::runtime::PlainBox<Terminal>;

/// A run's style resolved against live state.
pub(crate) type PlainRun = skene_layout::runtime::PlainRun<Terminal>;

/// The drawn runtime with each node's cell state carried in its record, the interaction
/// state and the environment.
#[expect(
  clippy::struct_excessive_bools,
  reason = "each flag is an independent fact about the frame or the pointer"
)]
pub(crate) struct Records {
  /// The drawn runtime: a record per node carrying the terminal's own state of it (its
  /// field, scroll offsets, the cells it covers and what it paints beyond its style), the
  /// root filling the screen, the overlays and entries in build order, the restyle with
  /// its presentation's counters, the layout tree, what the next frame lays out and
  /// repaints, the environment the terminal reports with the app's choice of scheme, and
  /// the frame clock.
  pub(crate) runtime: Runtime<Terminal>,
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
  /// The selection over the document's runs; a text entry's lives in the editing state
  /// the scene keeps for it ([`EntryState`]).
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
  pub(crate) paste_target: Option<NodeId>,
  /// The link pressed at its cell, opened when the release lands on the same cell.
  pub(crate) pressed_link: Option<(u16, u16, String)>,
  /// A link destination staged for the terminal loop to open.
  pub(crate) pending_open: Option<String>,
  /// The terminal's own foreground, where it reported one (OSC 10).
  pub(crate) detected_foreground: Option<(u8, u8, u8)>,
  /// The terminal's own background, where it reported one (OSC 11).
  pub(crate) detected_background: Option<(u8, u8, u8)>,
  /// The terminal's sixteen base colors, as it reported them (OSC 4).
  pub(crate) palette: Palette,
  /// The scheduled actions, each due at a clock reading.
  pub(crate) timers: Timers<Box<dyn FnOnce()>>,
  /// The node to scroll into view once the next layout is computed.
  pub(crate) reveal: Option<NodeId>,
  /// The window title the document asks for.
  pub(crate) title: Option<String>,
  /// Whether the title changed since the terminal was last told it.
  pub(crate) title_dirty: bool,
  /// Whether a key asked the app to quit.
  pub(crate) quit: bool,
  /// The clock reading the caret drag's autoscroll last stepped at, `None` while no
  /// drag is held past its entry's edge.
  pub(crate) autoscroll_at: Option<f64>,
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
  /// Records whose root box, filling the screen, is the scene's node `root`, the app
  /// taking the mouse where `mouse` holds.
  pub(crate) fn new(root: NodeId, mouse: bool) -> Self {
    Self {
      runtime: Runtime::new(root, Self::launch_environment(mouse), false),
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
      mouse,
      mouse_key: None,
      mouse_released: false,
      pending_copy: None,
      pending_paste: false,
      paste_target: None,
      pressed_link: None,
      pending_open: None,
      detected_foreground: None,
      detected_background: None,
      palette: Palette::DEFAULT,
      timers: Timers::new(),
      reveal: None,
      title: None,
      title_dirty: false,
      quit: false,
      autoscroll_at: None,
      clipboard_answers: false,
      key_releases: false,
      cell_pixels: DEFAULT_CELL_PIXELS,
      picture_generation: 0,
    }
  }

  /// The terminal's own state of `handle`, if it is live.
  #[inline(always)]
  pub(crate) fn cell(&self, handle: NodeId) -> Option<&CellNode> {
    self.runtime.record(handle).map(|record| &record.platform)
  }

  /// The terminal's own state of `handle`, mutably, if it is live.
  #[inline(always)]
  pub(crate) fn cell_mut(&mut self, handle: NodeId) -> Option<&mut CellNode> {
    self
      .runtime
      .record_mut(handle)
      .map(|record| &mut record.platform)
  }

  /// Measures every node's text again, after the terminal's clustering or sizing
  /// changed: each entry's value laid out afresh at its next query, and each laid-out
  /// node's flow made afresh at its next restyle.
  pub(crate) fn remeasure(&mut self) {
    self.runtime.remeasure();

    for &handle in &self.runtime.entries {
      if self.cell(handle).is_some_and(|cell| cell.field.is_some())
        && let Some(entry) = Scene::<Terminal>::entry_of(handle)
        && let Ok(mut editor) = entry.editor().try_borrow_mut()
      {
        editor.relayout();
      }
    }
  }

  /// Releases the record of `handle`, retiring the handle and every reference to it.
  pub(crate) fn destroy(&mut self, handle: NodeId) {
    let Some(record) = self.runtime.destroy(handle)
    else {
      return;
    };

    if let Some(rect) = record.platform.painted {
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
  }
}

#[cfg(test)]
mod tests {
  use skene_foundation::{
    Activate,
    Column,
    Container,
    Control,
    Grid,
    Layout,
    LayoutBox,
    LazyColumn,
    NodeId,
    Overlay,
    PlainChild,
    RichText,
    Row,
    Stack,
    Text,
    View,
  };

  use crate::{Terminal, install, mount, uninstall, with_records};

  /// Whether the record of `node` is a box created by a view taking decoration, `None`
  /// for a run, and whether the layout tree holds it as a container.
  fn decorated(node: NodeId) -> (Option<bool>, bool) {
    with_records(|records| {
      let decorated = records
        .runtime
        .record(node)
        .expect("a mounted node has a record")
        .kind
        .box_record()
        .map(|inner| inner.facts.decorated);

      (decorated, records.runtime.tree().is_container(node))
    })
  }

  /// The root, a container, an overlay's box and a control's are decorated, and every
  /// layout view, a lazy list's box, a third party's layout box, a paragraph's box and a
  /// run are not, so only the first are containers of the layout tree, laid out by an
  /// engine of their own.
  #[test]
  fn a_box_is_decorated_where_a_view_taking_decoration_created_it() {
    install();

    let container = mount(|| Container::<Terminal>::new().build());
    let overlay = mount(|| {
      Overlay::<Terminal>::popup()
        .set_child(Container::new())
        .build()
    });
    let control = mount(|| Control::<Terminal, Activate>::new(Activate::new()).build());
    let row = mount(|| Row::<Terminal>::new().build());
    let column = mount(|| Column::<Terminal>::new().build());
    let grid = mount(|| Grid::<Terminal>::new().build());
    let stack = mount(|| Stack::<Terminal>::new().build());
    let lazy = mount(|| LazyColumn::<Terminal>::new().build());
    let third_party = mount(|| LayoutBox::<Terminal, PlainChild>::new(Layout::Document).build());
    let paragraph = mount(|| RichText::<Terminal>::new().build());
    let run = mount(|| Text::<Terminal>::new("a").build());

    let root = with_records(|records| records.runtime.root());

    for node in [root, container, overlay, control] {
      assert_eq!(decorated(node), (Some(true), true));
    }

    for node in [row, column, grid, stack, lazy, third_party, paragraph] {
      assert_eq!(decorated(node), (Some(false), false));
    }

    assert_eq!(decorated(run), (None, false));

    uninstall();
  }
}
