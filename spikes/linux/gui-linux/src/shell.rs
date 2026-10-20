//! The window both backends share: its size, scale and chrome, the realized tree it shows,
//! what is dirty since the last frame, and the reader's input dispatched through the
//! foundation: hit testing, hover, press and focus states, Tab order in the scene's child
//! order, pointer events, clicks, keys and a reader's requests reported to the controls and
//! the boxes that act, each default run unless a verdict prevents it, wheel scrolling, a
//! touchpad's scrolling going on once its fingers lift ([`crate::kinetic`]), and the open
//! overlay's surface.
//!
//! A backend turns its protocol's events into calls here and carries out the
//! [`Request`]s returned (moving or resizing the window, the cursor), and asks for a frame
//! when [`Shell::wants_frame`] says one is due.

use std::{
  cell::RefCell,
  collections::HashMap,
  rc::Rc,
  time::{Duration, Instant},
};

use skene_foundation::{
  NodeId, State,
  behavior::{ActivateKeys, EntryAct, EntryKeys},
  environment::{self, Fact},
  layout::{Axis, Rect},
  seam::{
    content::{Content, PropValue, ScrollOffset, Verdict},
    control::{AccessRequest, Prevent},
    node::{Interaction, NodeKind},
    report::{
      Answer, ContextMenuEvent, KeyEvent, MenuSource, PointerEvent, PointerPhase, Report, Size,
    },
    tree,
  },
  text::{
    Laid as _,
    editing::{Binding, Command, Movement},
  },
};

use crate::{
  Linux,
  adornment::{Adornment, HANDLE_SIZE, Handle, MAGNIFIER_SIZE, Magnifier, Placement, Shown},
  chrome::{Button, Chrome, Edge, Hit},
  entry,
  keys::KeyInput,
  layout,
  node::{Drawn, Node, walk},
  overlay::{self, Open},
  paint::{Marks, Renderer, painted_bounds, union},
  palette::Palette,
  text::Change,
  trace,
};

/// The most nodes whose looks changed for which a frame measures their reach alone, theirs
/// and their ancestors'; past them it measures every node's.
const REACH_ALONE: usize = 32;

/// How many times a frame lays out before it paints, at most, where what laying out reports
/// changes the tree again.
const SETTLE_PASSES: usize = 4;

/// The share of a scroll box's length a page scrolls (`PageDown`, `Space`, a reader's page),
/// as Chromium's page does.
const PAGE: f32 = 0.875;

/// What a backend is asked to do for the window.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Request {
  /// Start an interactive move from the last press.
  Move,
  /// Start an interactive resize from the last press, at `Edge`.
  Resize(Edge),
  /// Maximize the window, or restore it.
  ToggleMaximize,
  /// Minimize it.
  Minimize,
  /// Close it: the app quits.
  Close,
  /// Show the window manager's menu at a point of the surface.
  ShowMenu(f32, f32),
  /// Show the cursor.
  Cursor(Cursor),
  /// Open a popup surface at this placement, or move the open one there; close it where
  /// `None`.
  Popup(Option<overlay::Placement>),
  /// Start a drag of what [`Shell::dragged`] says from the last press.
  StartDrag,
}

/// What a drag offers, by kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Offered {
  /// Text.
  pub text: bool,
  /// A list of URIs (`text/uri-list`).
  pub uris: bool,
}

/// What a drop does with what it takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DropAction {
  /// It copies it: the source keeps it.
  Copy,
  /// It moves it: the source gives it up.
  Move,
}

/// What a drop target takes of what a drag offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wanted {
  /// Its text.
  Text,
  /// Its URIs.
  Uris,
}

/// A drag of a text entry's selection, out of the window or within it.
struct DragOut {
  /// The entry.
  entry: Node,
  /// The selection dragged, bytes of the entry's text.
  range: std::ops::Range<usize>,
  /// Its text.
  text: String,
  /// Whether the entry gives it up where a drop moves it (it is editable).
  moves: bool,
  /// What it shows under the pointer.
  icon: Option<crate::adornment::DragIcon>,
}

/// How far a press on a selection moves, in pixels, before it drags the selection: GTK's drag
/// threshold.
const DRAG_THRESHOLD: f32 = 8.;

/// A pointer cursor, named as CSS names it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cursor {
  /// The arrow.
  Default,
  /// The hand over a link.
  Pointer,
  /// The I-beam over text that is edited.
  Text,
  /// A resize edge's or corner's arrow.
  Resize(Edge),
}

impl Cursor {
  /// The cursor's name in the freedesktop cursor theme.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Default => "default",
      Self::Pointer => "pointer",
      Self::Text => "text",
      Self::Resize(edge) => match edge {
        Edge::Top => "n-resize",
        Edge::Bottom => "s-resize",
        Edge::Left => "w-resize",
        Edge::Right => "e-resize",
        Edge::TopLeft => "nw-resize",
        Edge::TopRight => "ne-resize",
        Edge::BottomLeft => "sw-resize",
        Edge::BottomRight => "se-resize",
      },
    }
  }
}

/// How a wheel turned.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Wheel {
  /// By pixels (a touchpad, a high-resolution wheel's continuous motion).
  Pixels(f32, f32),
  /// By detents: 1 a notch (fractions on a high-resolution wheel).
  Lines(f32, f32),
}

/// What is dirty since the last frame.
#[derive(Default)]
struct Dirty {
  prepare: bool,
  prepare_nodes: Vec<Node>,
  layout: bool,
  all: bool,
  overlays: bool,
  nodes: Vec<Node>,
  rects: Vec<Rect>,
  focus: Option<Option<Node>>,
}

/// How long a touch is held, unmoved, before it is a long press: GTK's `gtk-long-press-time`.
const LONG_PRESS: Duration = Duration::from_millis(500);

/// How far a touch moves, in pixels, before it is a drag rather than a tap: GTK's drag
/// threshold.
const TOUCH_SLOP: f32 = 8.;

/// How soon a second click follows the first to make a double click: GTK's
/// `gtk-double-click-time`.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

thread_local! {
  static INPUTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
  /// Whether a scroll box was scrolled by a write since the last frame: the hover follows.
  static SCROLLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
  /// The touch whose long press is due, by its id.
  static LONG_PRESSED: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
  static FOCUSED: RefCell<Option<std::rc::Weak<crate::node::Drawn>>> = const { RefCell::new(None) };
  static DIRTY: RefCell<Dirty> = RefCell::new(Dirty {
    prepare: true,
    layout: true,
    all: true,
    ..Dirty::default()
  });
}

/// How many presses, releases and wheel turns of the pointer and the keys the windows of this
/// thread have handled (pointer motion aside): what a test driver waits on to know its input
/// arrived.
pub fn inputs() -> u64 {
  INPUTS.with(std::cell::Cell::get)
}

fn count_input() {
  INPUTS.with(|inputs| inputs.set(inputs.get() + 1));
}

/// Forgets the focus and the work pending of a window that closed, before another opens.
pub fn reset() {
  FOCUSED.with(|focused| *focused.borrow_mut() = None);
  DIRTY.with(|dirty| {
    *dirty.borrow_mut() = Dirty {
      prepare: true,
      layout: true,
      all: true,
      ..Dirty::default()
    };
  });
}

/// Whether `node` holds focus now.
pub fn is_focused(node: &Node) -> bool {
  FOCUSED.with(|focused| {
    focused
      .borrow()
      .as_ref()
      .and_then(std::rc::Weak::upgrade)
      .is_some_and(|focused| Rc::ptr_eq(&focused, node))
  })
}

/// Paragraphs need their texts and inherited styles again.
pub fn invalidate_prepare() {
  DIRTY.with(|dirty| dirty.borrow_mut().prepare = true);
}

/// The paragraphs in `node`'s subtree need their inherited styles again: its own run
/// properties changed and nothing else did.
pub fn invalidate_prepare_of(node: &Node) {
  DIRTY.with(|dirty| dirty.borrow_mut().prepare_nodes.push(Rc::clone(node)));
}

/// The tree needs laying out again.
pub fn invalidate_layout() {
  DIRTY.with(|dirty| dirty.borrow_mut().layout = true);
}

/// `node` needs painting again where it is.
pub fn invalidate_paint(node: &Node) {
  DIRTY.with(|dirty| dirty.borrow_mut().nodes.push(Rc::clone(node)));
}

/// `rect` (window pixels) needs painting again: where a node painted before it moved or its
/// look changed.
pub fn invalidate_rect(rect: Rect) {
  DIRTY.with(|dirty| dirty.borrow_mut().rects.push(rect));
}

impl Drawn {
  /// What it and its subtree paint now, in window pixels.
  pub fn reach_rect(self: &Rc<Self>) -> Rect {
    let at = self.window_rect();
    let reach = self.reach.get();

    Rect {
      x: at.x + reach.x,
      y: at.y + reach.y,
      ..reach
    }
  }
}

/// The whole window needs painting again.
pub fn invalidate_all() {
  DIRTY.with(|dirty| dirty.borrow_mut().all = true);
}

/// An overlay was shown, hidden or placed again: the open one is synced at the next turn.
pub fn invalidate_overlays() {
  DIRTY.with(|dirty| dirty.borrow_mut().overlays = true);
}

/// Reports each scroll box whose offset the last layouts clamped, as the reader's change.
fn report_clamped() {
  for node in layout::take_clamped() {
    if node.is_alive() {
      report_scroll(&node);
    }
  }
}

/// Reports the scroll box `node`'s offset as a change the platform applied (the reader
/// scrolled), and shows the truth the answer gives where it differs (a lazy list keeping its
/// anchor).
fn report_scroll(node: &Node) {
  let (x, y) = node.scroll.get();
  let answer = tree::report::<Linux>(
    node.id.get(),
    Report::Change {
      content: Content::ScrollOffset,
      proposed: PropValue::Offset(ScrollOffset::new(x, y)),
      applied: true,
    },
  );

  if let Answer::Verdict(Verdict::Replace(PropValue::Offset(offset))) = answer {
    node.scroll.set((offset.x, offset.y));
    invalidate_layout();
  }
}

/// Whether a layout is pending: what a write of a scroll offset waits on to clamp it.
fn layout_pending() -> bool {
  DIRTY.with(|dirty| dirty.borrow().layout)
}

/// Moves focus to `node` at the next opportunity, or takes it away.
pub fn request_focus(node: Option<Node>) {
  DIRTY.with(|dirty| dirty.borrow_mut().focus = Some(node));
}

/// The device rectangle covering `a` and `b`.
fn union_damage(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
  let left = a.0.min(b.0);
  let top = a.1.min(b.1);
  let right = (a.0 + a.2).max(b.0 + b.2);
  let bottom = (a.1 + a.3).max(b.1 + b.3);

  (left, top, right - left, bottom - top)
}

fn wants_frame() -> bool {
  DIRTY.with(|dirty| {
    let dirty = dirty.borrow();

    dirty.prepare
      || !dirty.prepare_nodes.is_empty()
      || dirty.layout
      || dirty.all
      || !dirty.nodes.is_empty()
      || !dirty.rects.is_empty()
  })
}

impl Drawn {
  /// Whether it is `ancestor` or beneath it.
  fn is_within(self: &Rc<Self>, ancestor: &Node) -> bool {
    Rc::ptr_eq(self, ancestor)
      || self
        .ancestors()
        .iter()
        .any(|candidate| Rc::ptr_eq(candidate, ancestor))
  }
}

/// A press in progress: where it started, on what.
struct Press {
  /// The deepest node under the press.
  target: Option<Node>,
  /// The control or acting box the press is on, which a release over it clicks.
  owner: Option<Node>,
  /// The title-bar press waiting to become a move: its position.
  title: Option<(f32, f32)>,
  /// The title-bar button pressed.
  button: Option<Button>,
  /// The pointer button that pressed (0 primary, 1 middle, 2 secondary).
  button_pressed: u8,
  /// Whether the owner keeps the pointer until the release (a control that drags).
  captured: bool,
  /// Where a press on a text entry's selection went down: it drags the selection once it moves
  /// past the threshold, and places the caret where it is released before that.
  on_selection: Option<(f32, f32)>,
}

/// A press on a scroll bar in progress: it scrolls its box and nothing else hears it.
struct BarPress {
  /// The scroll box the bar scrolls.
  scroll: Node,
  /// The axis the bar scrolls along.
  axis: Axis,
  /// Where the thumb starts along the axis, in window pixels, with the box at its start.
  base: f32,
  /// How far the thumb moves along its track.
  travel: f32,
  /// How far into the thumb the pointer holds it, while the press drags it; none while it
  /// paged.
  grab: Option<f32>,
  /// The nodes the press set pressed.
  chain: Vec<Node>,
}

/// What a touch became.
enum TouchRole {
  /// Down on the content, not yet a tap, a scroll or a long press.
  Pending,
  /// Placing the caret and selecting in the text entry it went down in, as GTK's entry does
  /// for a touchscreen: a tap places the caret (two select a word), a drag selects from where
  /// it went down, and a tap on the selection opens the context menu (GTK's selection
  /// popover).
  Selecting {
    /// The entry.
    entry: Node,
    /// Whether it went down on the entry's selection.
    on_selection: bool,
  },
  /// Moving a text entry's touch selection handle, as GTK's handle moves: the end it moves
  /// follows the finger from where the handle pointed as it went down.
  Handle {
    /// The handle.
    handle: Handle,
    /// The entry.
    entry: Node,
    /// Where the handle pointed as the finger went down: its x and its line's middle.
    from: (f32, f32),
    /// Whether the finger moved the caret or the selection.
    shifted: bool,
  },
  /// Driving the control that drags it went down on, as a pointer's press does.
  Dragging(Node),
  /// Scrolling this scroll box with the finger.
  Scrolling(Node),
  /// On the title bar: moving the window once it moves.
  Title,
  /// On a title-bar button: acting where it lifts on it.
  Button(Button),
  /// Nothing more: a long press happened, or it moved where nothing scrolls.
  Spent,
}

/// A text entry's touch selection handles, shown while a touch put them there, as GTK's
/// entry shows them: from a finger down in the entry until a key, a pointer's press or focus
/// moving elsewhere; hidden while an overlay is open.
struct Handles {
  /// The entry.
  entry: Node,
  /// When a tap last placed its caret: a tap on its handle soon after selects the word there,
  /// as a double tap would.
  placed: Option<Instant>,
  /// The handle a finger is moving, shown even where it points outside the text.
  dragged: Option<Handle>,
}

/// A finger on the window: the first one down, which alone acts.
struct Touch {
  /// The compositor's id of the touch point.
  id: i32,
  /// Where it went down (window pixels).
  start: (f32, f32),
  /// The compositor's time of its last event, in milliseconds.
  time: u32,
  /// Where it is now.
  at: (f32, f32),
  /// Whether it moved past the slop.
  moved: bool,
  /// The deepest node it went down on, with its ancestors.
  chain: Vec<Node>,
  /// What it became.
  role: TouchRole,
  /// Its long press, while one is pending.
  long_press: Option<skene_foundation::seam::runtime::Cancel>,
  /// Its recent motion, for a scroll's fling.
  tracker: crate::kinetic::Tracker,
}

impl Touch {
  /// Stops its pending long press.
  fn cancel_long_press(&mut self) {
    if let Some(cancel) = self.long_press.take() {
      cancel.cancel();
    }
  }
}

/// One painted frame: its pixels are the renderer's, `damage` the part that changed (device
/// pixels: x, y, width, height), unless the GPU presented it already.
pub struct Frame<'a> {
  /// The frame's pixels, on the CPU path.
  pub pixmap: &'a vello_cpu::Pixmap,
  /// The rectangle that changed.
  pub damage: (u32, u32, u32, u32),
  /// Whether the GPU presented the frame through the surface's swapchain: nothing is left to
  /// copy or attach.
  pub presented: bool,
}

/// What the last input was: whether focus moved now shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Modality {
  /// No input yet: a focus the window or a popup gives as it opens shows, as Chromium shows a
  /// focus a page gives before any interaction.
  Unknown,
  /// A pointer press or a touch: focus moved shows only in a text entry.
  Pointer,
  /// A key: focus moved shows.
  Keyboard,
}

impl Modality {
  /// Whether focus moved under this modality shows.
  const fn shows_focus(self) -> bool {
    !matches!(self, Self::Pointer)
  }
}

/// The window.
pub struct Shell {
  /// The root of the window's tree, once mounted.
  root: Option<Node>,
  /// The geometry's width and height, in pixels (the title bar included).
  pub size: (u32, u32),
  /// Device pixels per pixel.
  pub scale: f64,
  /// The chrome.
  pub chrome: Chrome,
  /// The window's title.
  title: String,
  /// What paints the window's frames.
  renderer: Renderer,
  /// Where the pointer is over the window's content, while it is.
  pointer: Option<(f32, f32)>,
  /// The nodes under the pointer, deepest first.
  hovered: Vec<Node>,
  /// The nodes holding [`State::Hovered`]: the nodes under the pointer and, over the open
  /// overlay, the overlay's ancestors in the scene too.
  hover_states: Vec<Node>,
  /// The press in progress.
  press: Option<Press>,
  /// The press on a scroll bar in progress.
  bar: Option<BarPress>,
  /// The node with focus.
  focus: Option<Node>,
  /// The focused text's link run with focus, by its index.
  link: Option<usize>,
  /// Whether focus shows.
  focus_visible: bool,
  /// Whether the next frame scrolls the focused node into view.
  reveal: bool,
  /// When and where the title bar was last clicked, for a double click.
  title_click: Option<(Instant, (f32, f32))>,
  /// The pointer's shape.
  cursor: Cursor,
  /// The open overlay.
  overlay: Option<Open>,
  /// Whether the pointer is over the open overlay rather than the window.
  pointer_in_overlay: bool,
  /// What the last input was, so whether focus moved now shows.
  modality: Modality,
  /// The node focus was on outside every overlay, last: what focus returns to when the open
  /// overlay holding it closes.
  outside_focus: Option<Node>,
  /// Whether a control that asked for focus as the window opens took it: one does, once.
  autofocused: bool,
  /// The acting box shown pressed from the keyboard (`Space` held).
  key_pressed: Option<Node>,
  /// What the shell asks of its platform, until it takes them.
  pending: Vec<Request>,
  /// The modifiers held now, as the platform reports them.
  modifiers: skene_foundation::seam::report::Modifiers,
  /// Whether the window has the keyboard.
  window_focused: bool,
  /// When, where and on what the last click was, and how many it was of a run.
  last_click: Option<(Instant, (f32, f32), u8, Node)>,
  /// The damage painted for a reader without presenting it, which the next frame presents.
  owed: Option<(u32, u32, u32, u32)>,
  /// The touchpad's recent motion.
  fingers: crate::kinetic::Tracker,
  /// The scroll box a touchpad's scroll moves, while its fingers are down.
  latched: Option<Node>,
  /// The scroll box going on by itself, and how.
  fling: Option<(Node, crate::kinetic::Fling)>,
  /// The fling the fingers caught again, which a fling the same way adds to.
  caught: Option<crate::kinetic::Fling>,
  /// The finger acting on the window, while one is down.
  touch: Option<Touch>,
  /// The drag of a text entry's selection in progress.
  drag_out: Option<DragOut>,
  /// A text entry's touch selection handles, while a touch shows them.
  handles: Option<Handles>,
  /// The magnifier, while a finger moves a text entry's caret or a selection's end.
  magnifier: Option<Magnifier>,
  /// What paints each adornment shown, kept while it shows.
  adornment_renderers: HashMap<Adornment, Renderer>,
  /// What a drag over the window would drop on: the node, and whether it is a text entry.
  drop_over: Option<(Node, bool)>,
  /// Frames painted.
  pub frames: u64,
  /// When the last frame was painted, the window's, its open overlay's or an adornment's.
  pub painted_at: Option<Instant>,
  /// Whether the last frame went to the screen through the GPU's swapchain.
  presented: bool,
  /// The part of the frame the last frame drew, in device pixels.
  drawn: (u32, u32, u32, u32),
  /// Whether the app asked to quit.
  pub quit: bool,
}

impl Shell {
  /// A window `width` by `height` titled `title`.
  pub fn new(title: &str, width: u32, height: u32) -> Self {
    let chrome = Chrome::default();

    chrome.set_title(title, &Palette::now());

    Self {
      root: None,
      size: (width, height),
      scale: 1.,
      chrome,
      title: title.to_owned(),
      renderer: Renderer::new(),
      pointer: None,
      hovered: Vec::new(),
      hover_states: Vec::new(),
      press: None,
      bar: None,
      focus: None,
      link: None,
      focus_visible: false,
      reveal: false,
      title_click: None,
      cursor: Cursor::Default,
      overlay: None,
      pointer_in_overlay: false,
      modality: Modality::Unknown,
      outside_focus: None,
      autofocused: false,
      key_pressed: None,
      pending: Vec::new(),
      modifiers: skene_foundation::seam::report::Modifiers::default(),
      window_focused: false,
      last_click: None,
      owed: None,
      fingers: crate::kinetic::Tracker::default(),
      latched: None,
      fling: None,
      caught: None,
      touch: None,
      drag_out: None,
      handles: None,
      magnifier: None,
      adornment_renderers: HashMap::new(),
      drop_over: None,
      frames: 0,
      painted_at: None,
      presented: false,
      drawn: (0, 0, 0, 0),
      quit: false,
    }
  }

  /// What the backend is asked to do beside what an event's call returned: a popup opened by
  /// a key, closed by a focus move.
  pub fn take_requests(&mut self) -> Vec<Request> {
    std::mem::take(&mut self.pending)
  }

  /// Shows `root`'s tree.
  pub fn set_root(&mut self, root: Node) {
    self.root = Some(root);
    invalidate_prepare();
    invalidate_layout();
  }

  /// The root, where one is shown.
  pub const fn root(&self) -> Option<&Node> {
    self.root.as_ref()
  }

  /// The window's title.
  pub fn title(&self) -> &str {
    &self.title
  }

  /// The content's size: the geometry below the title bar.
  pub fn content_size(&self) -> (f32, f32) {
    let header = self.chrome.header();

    (self.size.0 as f32, (self.size.1 as f32 - header).max(0.))
  }

  /// The frame's size in device pixels: the surface, shadow included.
  pub fn device_size(&self) -> (u32, u32) {
    let (width, height) = self.chrome.surface_size(self.size.0, self.size.1);

    (
      (f64::from(width) * self.scale).round() as u32,
      (f64::from(height) * self.scale).round() as u32,
    )
  }

  /// The window was given a size, its chrome's mode and states: the content is laid out
  /// again and the environment hears the content's size.
  pub fn configure(&mut self, width: u32, height: u32) {
    if (width, height) != self.size {
      self.size = (width.max(1), height.max(1));
    }

    let (content_width, content_height) = self.content_size();

    environment::report(Fact::Width(content_width));
    environment::report(Fact::Height(content_height));
    invalidate_layout();
    invalidate_all();
  }

  /// The window's scale changed: the window is laid out again, so a box that draws at its
  /// size and scale (a `Painted` one) hears the new scale before the frame that draws it.
  pub fn set_scale(&mut self, scale: f64) {
    if (scale - self.scale).abs() > f64::EPSILON {
      self.scale = scale;
      environment::report(Fact::Scale(scale));
      layout::mark_all();
      invalidate_layout();
      invalidate_all();
      // The reader's extents are device pixels.
      crate::access::invalidate();
    }
  }

  /// Whether a frame is due: something changed since the last one, or something is in
  /// motion.
  pub fn wants_frame(&self) -> bool {
    wants_frame() || self.owed.is_some() || self.in_motion()
  }

  /// Whether a frame is due for motion alone (a transition, a fling): what the frame clock
  /// paces.
  pub fn in_motion(&self) -> bool {
    crate::motion::running() || self.fling.is_some()
  }

  /// Applies a focus move the app or a behavior asked for, focus shown as the last input
  /// shows it: after a key, not after a press.
  pub fn settle_focus(&mut self) {
    let request = DIRTY.with(|dirty| dirty.borrow_mut().focus.take());

    if let Some(node) = request {
      let visible = self.modality.shows_focus();

      self.set_focus(node, visible);
    }
  }

  /// Focuses the first control beneath `root` that asked for focus as it opens
  /// ([`tree::autofocus_in`]) and takes focus now, its focus showing as the last input
  /// shows it; whether one did.
  fn autofocus(&mut self, root: &Node) -> bool {
    let Some(node) = tree::autofocus_in::<Linux>(root.id.get())
      .into_iter()
      .filter_map(crate::node_of)
      .find(Self::focusable)
    else {
      return false;
    };
    let visible = self.modality.shows_focus();

    trace::log(|| format!("[focus] autofocus {:?}", node.id.get()));
    self.set_focus(Some(node), visible);

    true
  }

  /// Lays out and paints what changed: the frame and the part of it that changed since the
  /// last frame presented, `None` where nothing did.
  pub fn frame(&mut self) -> Option<Frame<'_>> {
    let drawn = self.draw();

    if let Some(damage) = drawn
      && self.presented
    {
      return Some(Frame {
        pixmap: &self.renderer.pixmap,
        damage,
        presented: true,
      });
    }

    let damage = match (drawn, self.owed.take()) {
      (Some(drawn), Some(owed)) => union_damage(drawn, owed),
      (Some(damage), None) | (None, Some(damage)) => damage,
      (None, None) => return None,
    };
    let (width, height) = (
      u32::from(self.renderer.pixmap.width()),
      u32::from(self.renderer.pixmap.height()),
    );
    let left = damage.0.min(width);
    let top = damage.1.min(height);

    Some(Frame {
      pixmap: &self.renderer.pixmap,
      presented: false,
      damage: (
        left,
        top,
        damage.2.min(width - left),
        damage.3.min(height - top),
      ),
    })
  }

  /// Lays out and paints what changed without presenting it, so geometry reads current: the
  /// next [`frame`](Self::frame) presents it.
  pub fn lay_out(&mut self) {
    if let Some(drawn) = self.draw()
      && !self.presented
    {
      self.owed = Some(self.owed.map_or(drawn, |owed| union_damage(owed, drawn)));
    }
  }

  /// The path the window paints with now.
  pub const fn paint_path(&self) -> crate::paint::PaintPath {
    self.renderer.path()
  }

  /// Whether the window paints on the GPU, which presents each frame itself.
  pub const fn presents(&self) -> bool {
    matches!(self.renderer.path(), crate::paint::PaintPath::Gpu)
  }

  /// Chooses the window's path once, as [`crate::paint::PathChoice::current`] asks: the GPU
  /// where an adapter it admits presents to the surface at `target`, the CPU otherwise.
  ///
  /// # Safety
  ///
  /// The handles in `target` stay valid until [`Shell::release_paths`].
  #[cfg(feature = "gpu")]
  pub unsafe fn choose_path(&mut self, target: crate::gpu::Target) {
    let (width, height) = self.device_size();

    let choice = crate::paint::PathChoice::current();

    // SAFETY: the caller keeps the handles valid while the window lives.
    match unsafe { crate::gpu::Gpu::new(target, (width, height), choice) } {
      Ok(gpu) => {
        self.renderer.use_gpu(gpu);
        invalidate_all();
      }
      Err(why) => trace::log(|| format!("[gpu] the CPU path: {why}")),
    }
  }

  /// Loses the GPU's device, as a driver reset would, unknown to the window: whether the window
  /// painted on the GPU. The loss wakes the loop ([`crate::runtime::Message::DeviceLost`]),
  /// which paints the window and its popup again on the CPU.
  pub fn lose_device(&mut self) -> bool {
    self.renderer.lose_device()
  }

  /// Paints the open overlay on the GPU from now on, on the window's device, presenting to the
  /// surface at `target`, where the window paints on the GPU and its adapter presents to the
  /// surface; on the CPU otherwise.
  ///
  /// # Safety
  ///
  /// The handles in `target` stay valid until [`Shell::release_popup_path`],
  /// [`Shell::release_paths`] or the overlay's close.
  #[cfg(feature = "gpu")]
  pub unsafe fn choose_popup_path(&mut self, target: crate::gpu::Target) {
    let scale = self.popup_scale();
    let (Some(gpu), Some(open)) = (self.renderer.gpu(), self.overlay.as_mut()) else {
      return;
    };
    let size = open.device_size(scale);

    // SAFETY: the caller keeps the handles valid while the overlay paints through them.
    match unsafe { gpu.share(target, (u32::from(size.0), u32::from(size.1))) } {
      Ok(gpu) => open.use_gpu(gpu),
      Err(why) => trace::log(|| format!("[gpu] the popup paints on the CPU: {why}")),
    }
  }

  /// The window's surfaces are going: the window and its open overlay paint on the CPU from
  /// now on, the GPU paths that presented to the surfaces gone first.
  pub fn release_paths(&mut self) {
    self.release_popup_path();
    #[cfg(feature = "gpu")]
    self.renderer.drop_gpu();
  }

  /// The open overlay's surface is going: it paints on the CPU until it is given another.
  pub fn release_popup_path(&mut self) {
    if let Some(open) = self.overlay.as_mut() {
      open.drop_gpu();
    }
  }

  /// Whether the open overlay paints on the GPU, which presents each of its frames itself.
  pub fn popup_presents(&self) -> bool {
    self.overlay.as_ref().is_some_and(Open::presents)
  }

  /// Lays out and paints what changed; the device rectangle painted.
  fn draw(&mut self) -> Option<(u32, u32, u32, u32)> {
    self.settle_focus();

    let root = self.root.clone()?;

    // As HTML processes autofocus: the first control asking for focus as it shows takes it
    // while nothing has focus, once in the window's life.
    if !self.autofocused && self.focus.is_none() && self.autofocus(&root) {
      self.autofocused = true;
    }

    let started = Instant::now();
    let palette = Palette::now();

    crate::motion::advance(crate::motion::now());
    self.advance_fling(Instant::now());

    let mut dirty = DIRTY.with(|dirty| std::mem::take(&mut *dirty.borrow_mut()));
    let mut prepared = Duration::ZERO;
    let mut laid = false;
    // The epoch this frame's first layout runs in: what its layouts place gathers its reach.
    let first_layout = layout::epoch() + 1;

    // Layout settles before the frame paints: what laying out reports (a box's size, a scroll
    // box's offset clamped) may change the tree again (a lazy list realizing the rows its
    // viewport shows, measuring them, keeping its anchor), and that is laid out again, a few
    // times at most.
    for _ in 0..SETTLE_PASSES {
      if dirty.overlays {
        // An overlay's change waits for the turn that syncs it.
        invalidate_overlays();
      }

      let pass = Instant::now();

      if dirty.prepare {
        if self.chrome.set_title(&self.title, &palette) != Change::None {
          dirty.all = true;
        }

        let mut repainted = Vec::new();

        if layout::prepare(&root, &palette, &mut repainted) == Change::Layout {
          dirty.layout = true;
        }

        dirty.nodes.extend(repainted);
      } else if !dirty.prepare_nodes.is_empty() {
        let mut repainted = Vec::new();

        for node in std::mem::take(&mut dirty.prepare_nodes) {
          if node.is_alive() && node.prepare_subtree(&palette, &mut repainted) == Change::Layout {
            dirty.layout = true;
          }
        }

        dirty.nodes.extend(repainted);
      }

      prepared += pass.elapsed();

      if dirty.layout {
        let (width, height) = self.content_size();

        layout::layout(&root, (0., self.chrome.header()), width, height);
        crate::access::invalidate();
        walk(&root, |node| {
          if node.is_entry() {
            node.fit_entry();
          }
        });
        dirty.all = true;
        laid = true;
        // Before the paint, so a box that draws at its size draws in the frame that sized it;
        // and before anything this pass reports changes the tree, so only what was laid out
        // reports a size.
        self.report_sizes(&root, self.chrome.origin());
        report_clamped();
      }

      if std::mem::take(&mut self.reveal) && self.reveal_focus() {
        dirty.all = true;
      }

      let again = DIRTY.with(|dirty| std::mem::take(&mut *dirty.borrow_mut()));
      let settled = !again.prepare && again.prepare_nodes.is_empty() && !again.layout;

      dirty.prepare = again.prepare;
      dirty.prepare_nodes = again.prepare_nodes;
      dirty.layout = again.layout;
      dirty.all |= again.all;
      dirty.overlays |= again.overlays;
      dirty.nodes.extend(again.nodes);
      dirty.rects.extend(again.rects);

      if again.focus.is_some() {
        dirty.focus = again.focus;
      }

      if settled {
        break;
      }
    }

    if dirty.prepare || dirty.layout || !dirty.prepare_nodes.is_empty() {
      // Still unsettled: the next frame takes it up.
      DIRTY.with(|pending| {
        let mut pending = pending.borrow_mut();

        pending.prepare |= dirty.prepare;
        pending.layout |= dirty.layout;
        pending.prepare_nodes.append(&mut dirty.prepare_nodes);
      });
    }

    if let Some(focus) = dirty.focus.take() {
      DIRTY.with(|pending| pending.borrow_mut().focus = Some(focus));
    }

    dirty.layout = laid;

    if dirty.overlays {
      invalidate_overlays();
    }

    self.mark_overlay(&dirty);

    // A frame that changed much measures every node's reach again; one that laid out, what
    // its layouts placed and the few whose looks changed; one that changed a few looks, those
    // nodes' and their ancestors'.
    if dirty.all || dirty.nodes.len() > REACH_ALONE {
      layout::reach(&root, 0);
    } else if dirty.layout {
      layout::reach(&root, first_layout);

      for node in dirty.nodes.iter().filter(|node| node.is_alive()) {
        node.refresh_reach();
      }
    } else {
      for node in dirty.nodes.iter().filter(|node| node.is_alive()) {
        node.refresh_reach();
      }
    }

    let laid_out = started.elapsed() - prepared;
    let (ox, oy) = self.chrome.origin();
    let (surface_width, surface_height) = self.chrome.surface_size(self.size.0, self.size.1);
    let surface = Rect {
      x: -ox,
      y: -oy,
      width: surface_width as f32,
      height: surface_height as f32,
    };
    let region = if dirty.all {
      surface
    } else {
      dirty
        .nodes
        .iter()
        .filter(|node| node.is_shown())
        .map(|node| union(painted_bounds(node, node.window_rect()), node.reach_rect()))
        .chain(dirty.rects.iter().copied())
        .fold(Rect::default(), union)
    };

    if region.width <= 0. || region.height <= 0. {
      return None;
    }

    let device = self.device_size();
    let marks = Marks {
      focus: self.focus.clone().filter(|_| self.focus_visible),
      focused: self.focus.clone(),
      link: self
        .focus
        .as_ref()
        .zip(self.link)
        .and_then(|(focus, link)| focus.links().get(link).map(|(range, _)| range.clone())),
    };
    let chrome = self.chrome.clone();
    let (width, height) = self.size;
    let gpu = self.presents();
    let cost = self.renderer.paint(
      Some(&root),
      (device.0 as u16, device.1 as u16),
      self.scale,
      region,
      (ox, oy),
      &marks,
      &mut |canvas| chrome.paint(canvas, width, height),
    );

    self.presented = cost.presented;
    self.drawn = cost.drawn;

    // A frame that fell back to the CPU as the device was lost painted the whole surface.
    let region = if gpu && !cost.presented {
      surface
    } else {
      region
    };

    self.frames += 1;
    self.painted_at = Some(started);
    skene_foundation::seam::activity::frame();

    if dirty.layout || SCROLLED.with(|scrolled| scrolled.replace(false)) {
      self.refresh_hover();
    }

    if dirty.layout && trace::nodes_on() {
      let mut scene = 0;

      walk(&root, |_| scene += 1);
      trace::nodes(scene, crate::access::count(self));
    }

    trace::frame(trace::FrameCost {
      prepare: prepared,
      layout: laid_out,
      record: Duration::from_micros(cost.record),
      raster: Duration::from_micros(cost.raster),
      glyphs: Duration::from_micros(cost.glyphs),
      masks: cost.masks,
      images: cost.images,
      shadows: cost.shadows,
      total: started.elapsed(),
      full: dirty.all,
      region,
    });

    let scale = self.scale;
    let to_device = |value: f32, origin: f32| (f64::from(value + origin) * scale).max(0.);
    let left = to_device(region.x, ox).floor();
    let top = to_device(region.y, oy).floor();
    let right = to_device(region.x + region.width, ox)
      .ceil()
      .min(f64::from(device.0));
    let bottom = to_device(region.y + region.height, oy)
      .ceil()
      .min(f64::from(device.1));

    Some((
      left as u32,
      top as u32,
      (right - left).max(0.) as u32,
      (bottom - top).max(0.) as u32,
    ))
  }

  /// Reports each node of `root`'s surface that asked for its size and was laid out at
  /// another, or at another device size: its window rectangle, the surface's top left at
  /// `origin` (window pixels), snapped to the surface's device pixels. An overlay beneath
  /// `root` reports on its own surface.
  fn report_sizes(&self, root: &Node, origin: (f32, f32)) {
    fn visit(node: &Node, root: &Node, found: &mut Vec<Node>) {
      if node.is_hidden() || (node.is_overlay() && !Rc::ptr_eq(node, root)) {
        return;
      }

      if node.interest.get().size {
        found.push(Rc::clone(node));
      }

      for child in node.children_ref().iter() {
        visit(child, root, found);
      }
    }

    let mut asking = Vec::new();
    let scale = self.scale;

    visit(root, root, &mut asking);

    let snap = |at: f32, origin: f32| (f64::from(at + origin) * scale).round();
    let changed: Vec<(NodeId, Size)> = asking
      .into_iter()
      .filter_map(|node| {
        let frame = node.frame.get();
        let rect = node.window_rect();
        #[expect(
          clippy::cast_possible_truncation,
          clippy::cast_sign_loss,
          reason = "device pixels"
        )]
        let device = (
          (snap(rect.x + rect.width, origin.0) - snap(rect.x, origin.0)).max(0.) as u32,
          (snap(rect.y + rect.height, origin.1) - snap(rect.y, origin.1)).max(0.) as u32,
        );
        let size = Size {
          width: frame.width,
          height: frame.height,
          scale,
          device,
        };

        (node.reported.get() != Some(size)).then(|| {
          node.reported.set(Some(size));

          (node.id.get(), size)
        })
      })
      .collect();

    for (id, size) in changed {
      tree::report::<Linux>(id, Report::Size(size));
    }
  }

  /// The deepest shown node at `x`, `y` (window pixels), with its ancestors, deepest first:
  /// in the open overlay where the pointer is over it, in the window's tree otherwise.
  fn hit(&self, x: f32, y: f32) -> Vec<Node> {
    match &self.overlay {
      Some(open) if self.pointer_in_overlay => Self::hit_in(&open.node, x, y),
      _ => self
        .root
        .as_ref()
        .map(|root| Self::hit_in(root, x, y))
        .unwrap_or_default(),
    }
  }

  /// The deepest shown node under `root` at `x`, `y` (window pixels), with its ancestors up
  /// to `root`, deepest first; an overlay beneath `root` is not in its flow.
  ///
  /// A node takes a point inside its box rounded by its corners, moved by its offset, as CSS
  /// shapes a hit by `border-radius` and moves it by a transform; its ring and its shadows take
  /// none. Only a box that clips (`clip`, a scroll box's viewport, an overlay's surface) cuts
  /// the hits of what it holds, so a part visibly overflowing its control takes a press as the
  /// control's, as in Chromium. A subtree whose reach (what it paints) misses the point is
  /// skipped.
  fn hit_in(root: &Node, x: f32, y: f32) -> Vec<Node> {
    fn go(node: &Node, origin: (f32, f32), x: f32, y: f32, out: &mut Vec<Node>) -> bool {
      if node.is_hidden() {
        return false;
      }

      let frame = node.frame.get();
      let (dx, dy) = node.offset();
      let rect = Rect {
        x: origin.0 + frame.x + dx,
        y: origin.1 + frame.y + dy,
        ..frame
      };
      let reach = node.reach.get();

      if x < rect.x + reach.x
        || y < rect.y + reach.y
        || x >= rect.x + reach.x + reach.width
        || y >= rect.y + reach.y + reach.height
      {
        return false;
      }

      let (inside, clips) = {
        let look = node.look.borrow();
        let inside = x >= rect.x
          && y >= rect.y
          && x < rect.x + rect.width
          && y < rect.y + rect.height
          && (look.corners == skene_foundation::Corners::all(0.)
            || vello_cpu::kurbo::Shape::contains(
              &vello_cpu::kurbo::RoundedRect::from_rect(
                crate::paint::krect(rect),
                crate::paint::radii(look.corners, rect),
              ),
              vello_cpu::kurbo::Point::new(f64::from(x), f64::from(y)),
            ));
        let clips = look.clip
          || node.is_overlay()
          || matches!(
            node.layout_kind(),
            Some(skene_foundation::seam::node::LayoutKind::Scroll { .. })
          );

        (inside, clips)
      };

      if !inside && clips {
        return false;
      }

      for child in node.children_ref().iter().rev() {
        if child.is_overlay() {
          continue;
        }

        let (sx, sy) = node.scroll_for(child);

        if go(child, (rect.x - sx, rect.y - sy), x, y, out) {
          out.push(Rc::clone(node));

          return true;
        }
      }

      if inside {
        out.push(Rc::clone(node));
      }

      inside
    }

    let mut out = Vec::new();
    let origin = if root.is_overlay() {
      (0., 0.)
    } else {
      let at = root.window_rect();
      let frame = root.frame.get();

      (at.x - frame.x, at.y - frame.y)
    };

    go(root, origin, x, y, &mut out);

    out
  }

  fn set_state(node: &Node, state: State, holds: bool) {
    let states = node.states.get();
    let next = if holds {
      states.with(state)
    } else {
      states.without(state)
    };

    if next == states {
      return;
    }

    node.states.set(next);

    if node.interest.get().states.contains(state) {
      tree::report::<Linux>(node.id.get(), Report::State { state, holds });
      trace::state(node, state, holds);
    }
  }

  fn set_chain(previous: &[Node], next: &[Node], state: State) {
    for node in previous {
      if !next.iter().any(|other| Rc::ptr_eq(other, node)) {
        Self::set_state(node, state, false);
      }
    }

    for node in next {
      Self::set_state(node, state, true);
    }
  }

  fn refresh_hover(&mut self) {
    let next = self
      .pointer
      .map(|(x, y)| self.hit(x, y))
      .unwrap_or_default();
    let mut held = next.clone();

    // A popover the web promotes in place stays a descendant of its anchor's element, so
    // `:hover` holds on the anchor's chain while the pointer is over it: the chain goes on
    // from the open overlay to its ancestors in the scene.
    if self.pointer_in_overlay
      && let Some(open) = &self.overlay
    {
      held.extend(open.node.ancestors());
    }

    self.hovered = next;

    if held.len() == self.hover_states.len()
      && held
        .iter()
        .zip(&self.hover_states)
        .all(|(a, b)| Rc::ptr_eq(a, b))
    {
      return;
    }

    let previous = std::mem::replace(&mut self.hover_states, held.clone());

    Self::set_chain(&previous, &held, State::Hovered);
  }

  /// The control or acting box a press on `chain` belongs to: the deepest.
  fn owner(chain: &[Node]) -> Option<Node> {
    chain
      .iter()
      .find(|node| node.spec.is_interactive())
      .cloned()
  }

  /// Whether `node` takes focus now: a shown control whose facts say it does (a radio while
  /// it is its group's stop), a shown box that acts, or a scroll box that scrolls holding
  /// nothing that takes focus ([`Drawn::is_scroll_stop`]).
  fn focusable(node: &Node) -> bool {
    node.is_shown()
      && match node.spec.interaction {
        Interaction::Control { .. } => node.access.borrow().focusable,
        Interaction::Acting => true,
        Interaction::None => node.is_scroll_stop(),
        Interaction::Answering | Interaction::ScrollBar { .. } => false,
      }
  }

  fn set_focus(&mut self, node: Option<Node>, visible: bool) {
    self.set_focus_to(node, visible, false);
  }

  /// Moves focus to `node` (taking it away where `None`), focus shown where `visible`; a
  /// node that takes no focus by Tab is focused anyway where `forced` (a text whose link run
  /// takes it).
  fn set_focus_to(&mut self, node: Option<Node>, visible: bool, forced: bool) {
    let node = node.filter(|node| {
      forced && node.is_shown() && !node.access.borrow().disabled || Self::focusable(node)
    });
    let visible = visible || node.as_ref().is_some_and(|node| node.is_entry());

    if match (&node, &self.focus) {
      (Some(a), Some(b)) => Rc::ptr_eq(a, b),
      (None, None) => true,
      _ => false,
    } {
      if self.focus_visible != visible {
        self.focus_visible = visible;

        if let Some(focus) = &node {
          Self::set_state(focus, State::FocusVisible, visible);
          invalidate_paint(focus);
        }
      }

      return;
    }

    let previous = self.focus.take();
    let previous_chain: Vec<Node> = previous
      .iter()
      .flat_map(|node| std::iter::once(Rc::clone(node)).chain(node.ancestors()))
      .collect();
    let next_chain: Vec<Node> = node
      .iter()
      .flat_map(|node| std::iter::once(Rc::clone(node)).chain(node.ancestors()))
      .collect();

    if let Some(previous) = &previous {
      Self::set_state(previous, State::FocusVisible, false);
      Self::set_state(previous, State::Focused, false);
      self.release_key(previous);
      invalidate_paint(previous);
      previous.blur_entry();

      if previous.interest.get().focus {
        tree::report::<Linux>(previous.id.get(), Report::Focus(false));
      }
    }

    // Focus moving out of the open overlay, to anything but what holds it, dismisses it.
    if let (Some(open), Some(next)) = (&self.overlay, &node)
      && !next.is_within(&open.node)
      && !open.node.is_within(next)
    {
      tree::report::<Linux>(open.node.id.get(), Report::Dismiss);
    }

    Self::set_chain(&previous_chain, &next_chain, State::FocusWithin);

    if node.as_ref().is_none_or(|node| node.overlay().is_none()) {
      self.outside_focus.clone_from(&node);
    }

    self.focus = node.clone();
    self.link = None;
    self.focus_visible = visible;
    self.reveal = visible;
    FOCUSED.with(|focused| *focused.borrow_mut() = node.as_ref().map(Rc::downgrade));
    entry::ime_dirty(false);
    crate::access::invalidate();

    if let Some(node) = &node {
      Self::set_state(node, State::Focused, true);
      Self::set_state(node, State::FocusVisible, visible);
      invalidate_paint(node);

      if node.is_entry() && node.has_entry() {
        node.blink_caret();
      }

      if node.interest.get().focus {
        tree::report::<Linux>(node.id.get(), Report::Focus(true));
      }

      trace::focus(node);
    }
  }

  /// The tab stops in the scene's child order: each focusable node, and each link run of a
  /// text by its place among the text's links.
  fn focus_order(&self) -> Vec<(Node, Option<usize>)> {
    let mut order = Vec::new();

    if let Some(root) = &self.root {
      walk(root, |node| {
        if Self::focusable(node) {
          order.push((Rc::clone(node), None));
        } else if node.spec.kind == NodeKind::Text && node.is_shown() {
          for link in 0..node.links().len() {
            order.push((Rc::clone(node), Some(link)));
          }
        }
      });
    }

    order
  }

  /// Moves focus to link run `link` of the text `node`, focus shown where `visible`.
  fn focus_link(&mut self, node: &Node, link: usize, visible: bool) {
    self.set_focus_to(Some(Rc::clone(node)), visible, true);

    if self
      .focus
      .as_ref()
      .is_some_and(|focus| Rc::ptr_eq(focus, node))
      && self.link != Some(link)
    {
      self.link = Some(link);
      self.reveal = visible;
      invalidate_paint(node);
      crate::access::invalidate();
    }
  }

  fn move_focus(&mut self, backward: bool) {
    let order = self.focus_order();

    if order.is_empty() {
      return;
    }

    let at = self.focus.as_ref().and_then(|focus| {
      order
        .iter()
        .position(|(node, link)| Rc::ptr_eq(node, focus) && *link == self.link)
    });
    let next = match (at, backward) {
      (None, false) => 0,
      (None, true) => order.len() - 1,
      (Some(at), false) => (at + 1) % order.len(),
      (Some(at), true) => (at + order.len() - 1) % order.len(),
    };
    // Past the last stop (or before the first) the order goes round; a node holding the focus
    // that realizes more stops than it shows (a lazy list) is asked for its next first.
    let wrapped = match (at, backward) {
      (Some(at), false) => at + 1 == order.len(),
      (Some(at), true) => at == 0,
      (None, _) => false,
    };
    let (mut next, mut link) = order[next].clone();

    if let Some(focus) = &self.focus {
      let candidate = (!wrapped).then(|| next.id.get());

      if let Some(found) = tree::next_stop::<Linux>(focus.id.get(), candidate, backward)
        && found != next.id.get()
        && let Some(found) = crate::node_of(found)
      {
        next = found;
        link = None;
      }
    }

    match link {
      Some(link) => self.focus_link(&next, link, true),
      None => self.set_focus(Some(Rc::clone(&next)), true),
    }

    // Tab into a one-line field selects what it holds, as GtkEntry and a browser's input do.
    if next.spec.kind == (NodeKind::Editable { multiline: false }) && next.has_entry() {
      let _ = next.perform(Command::SelectAll);
    }
  }

  /// Shows the box that acts `node` pressed from the keyboard (`Space` held), or not.
  fn press_key(&mut self, node: &Node, pressed: bool) {
    let held = self
      .key_pressed
      .as_ref()
      .is_some_and(|held| Rc::ptr_eq(held, node));

    if held != pressed {
      self.key_pressed = pressed.then(|| Rc::clone(node));
      Self::set_state(node, State::Pressed, pressed);
    }
  }

  /// `node` no longer shows pressed from the keyboard.
  fn release_key(&mut self, node: &Node) {
    self.press_key(node, false);
  }

  /// Scrolls the scroll box `node` by `dx`, `dy` within its extent, on the axes it scrolls;
  /// whether it moved.
  fn scroll_by(&mut self, node: &Node, dx: f32, dy: f32) -> bool {
    let Some(skene_foundation::seam::node::LayoutKind::Scroll {
      horizontal,
      vertical,
    }) = node.layout_kind()
    else {
      return false;
    };
    let frame = node.frame.get();
    let (width, height) = node.extent.get();
    let (sx, sy) = node.scroll.get();
    let next = (
      if horizontal {
        (sx + dx).clamp(0., (width - frame.width).max(0.))
      } else {
        sx
      },
      if vertical {
        (sy + dy).clamp(0., (height - frame.height).max(0.))
      } else {
        sy
      },
    );

    if next == (sx, sy) {
      return false;
    }

    node.scroll.set(next);
    invalidate_paint(node);
    trace::scroll(node, next);
    report_scroll(node);
    self.refresh_hover();
    crate::access::invalidate();

    true
  }

  /// Scrolls the scroll boxes holding the focused node, nearest first, as little as shows
  /// it whole (its start where it is larger than the box); whether one moved.
  fn reveal_focus(&mut self) -> bool {
    let Some(focus) = self.focus.clone() else {
      return false;
    };

    self.reveal(&focus)
  }

  /// Scrolls the scroll boxes holding `node`, nearest first, as little as shows it whole (its
  /// start where it is larger than the box); whether one moved.
  fn reveal(&mut self, node: &Node) -> bool {
    let mut moved = false;

    for ancestor in node.ancestors() {
      if !matches!(
        ancestor.layout_kind(),
        Some(skene_foundation::seam::node::LayoutKind::Scroll { .. })
      ) {
        continue;
      }

      let target = node.window_rect();
      let view = ancestor.window_rect();
      let delta = |start: f32, length: f32, view_start: f32, view_length: f32| {
        let before = start - view_start;
        let after = start + length - (view_start + view_length);

        if before < 0. {
          before
        } else if after > 0. {
          after.min(before)
        } else {
          0.
        }
      };
      let dx = delta(target.x, target.width, view.x, view.width);
      let dy = delta(target.y, target.height, view.y, view.height);

      moved |= self.scroll_by(&ancestor, dx, dy);
    }

    moved
  }

  /// Scrolls by a key, as a browser's page does: the arrows by a line, the page keys and
  /// `Space` by most of the box, `Home` and `End` to its ends. The scroll box holding the
  /// focus, nearest first, else the one under the pointer, else the first in the scene takes
  /// it; whether one moved.
  fn scroll_key(&mut self, event: &KeyEvent) -> bool {
    const LINE: f32 = 40.;

    let modifiers = event.modifiers;

    if !event.down || modifiers.control || modifiers.alt || modifiers.meta {
      return false;
    }

    let page = |node: &Node| node.frame.get().height * PAGE;
    let step = |node: &Node| -> Option<(f32, f32)> {
      Some(match event.key.as_str() {
        "ArrowUp" => (0., -LINE),
        "ArrowDown" => (0., LINE),
        "ArrowLeft" => (-LINE, 0.),
        "ArrowRight" => (LINE, 0.),
        "PageUp" => (0., -page(node)),
        "PageDown" => (0., page(node)),
        " " if modifiers.shift => (0., -page(node)),
        " " => (0., page(node)),
        "Home" => (0., f32::NEG_INFINITY),
        "End" => (0., f32::INFINITY),
        _ => return None,
      })
    };
    let chain: Vec<Node> = match &self.focus {
      Some(focus) => std::iter::once(Rc::clone(focus))
        .chain(focus.ancestors())
        .collect(),
      None => {
        let mut chain = self.hovered.clone();
        let mut first = None;

        if let Some(root) = &self.root {
          walk(root, |node| {
            if first.is_none()
              && matches!(
                node.layout_kind(),
                Some(skene_foundation::seam::node::LayoutKind::Scroll { .. })
              )
            {
              first = Some(Rc::clone(node));
            }
          });
        }

        chain.extend(first);
        chain
      }
    };

    for node in chain {
      if let Some((dx, dy)) = step(&node)
        && self.scroll_by(&node, dx, dy)
      {
        return true;
      }
    }

    false
  }

  /// Reports a click (a primary press and its release over one owner) at `node`, a control or
  /// a box that acts.
  fn click(&mut self, node: &Node) {
    trace::activate(node);
    tree::report::<Linux>(node.id.get(), Report::Click);
    self.settle_focus();
    crate::access::invalidate();
  }

  /// Opens the overlay shown now, closes the open one where it was hidden, and lays the open
  /// one out again where its content changed: the backend is asked to place its surface.
  pub fn sync_overlays(&mut self) {
    let changed = DIRTY.with(|dirty| std::mem::take(&mut dirty.borrow_mut().overlays));
    let closing = self
      .overlay
      .as_ref()
      .is_some_and(|open| !open.node.is_alive() || !open.node.is_shown());

    if closing {
      self.close_overlay();
    }

    if !changed && !closing {
      return;
    }

    let Some(node) = overlay::shown() else {
      return;
    };

    if self
      .overlay
      .as_ref()
      .is_some_and(|open| Rc::ptr_eq(&open.node, &node))
    {
      self.relayout_overlay();

      return;
    }

    if self.overlay.is_some() {
      self.close_overlay();
    }

    // The window is laid out first: the overlay is placed against its anchor's rectangle.
    self.lay_out();

    let open = Open::new(Rc::clone(&node));

    trace::log(|| {
      format!(
        "[popup] open {:?} {}x{}",
        open.placement.anchor, open.placement.size.0, open.placement.size.1
      )
    });
    self.pending.push(Request::Popup(Some(open.placement)));
    self.overlay = Some(open);

    let frame = node.frame.get();

    self.report_sizes(&node, (-frame.x, -frame.y));
    // A popup focuses its first control asking for focus as it opens; with none, focus stays
    // where its behavior put it.
    self.autofocus(&node);
    crate::access::invalidate();
  }

  /// Lays the open overlay out again: its surface is placed again where its size changed.
  fn relayout_overlay(&mut self) {
    let Some(open) = &mut self.overlay else {
      return;
    };

    if open.relayout() {
      self.pending.push(Request::Popup(Some(open.placement)));
    }

    let node = Rc::clone(&open.node);
    let frame = node.frame.get();

    self.report_sizes(&node, (-frame.x, -frame.y));
    crate::access::invalidate();
  }

  /// Closes the open overlay's surface; focus inside it returns to what held focus when it
  /// opened, where that still takes focus, else to the nearest node before it that does (its
  /// anchor), as a popover hiding restores focus.
  fn close_overlay(&mut self) {
    let Some(open) = self.overlay.take() else {
      return;
    };

    trace::log(|| "[popup] close".to_owned());
    self.pending.push(Request::Popup(None));
    self.pointer_in_overlay = false;

    if let Some(focus) = self.focus.clone()
      && focus.is_within(&open.node)
    {
      let back = self
        .outside_focus
        .clone()
        .filter(|node| node.is_alive() && Self::focusable(node))
        .or_else(|| open.node.ancestors().into_iter().find(Self::focusable));
      let visible = self.modality.shows_focus();

      self.set_focus(back, visible);
    }

    if self
      .hovered
      .first()
      .is_some_and(|hovered| hovered.is_within(&open.node))
    {
      self.hovered.clear();

      let previous = std::mem::take(&mut self.hover_states);

      Self::set_chain(&previous, &[], State::Hovered);
    }

    crate::access::invalidate();
  }

  /// Marks the open overlay to be painted (and laid out) again where what changed reaches it.
  fn mark_overlay(&mut self, dirty: &Dirty) {
    let Some(open) = &self.overlay else {
      return;
    };

    if dirty.prepare || dirty.layout {
      self.relayout_overlay();
    } else if (dirty.all || dirty.nodes.iter().any(|node| node.is_within(&open.node)))
      && let Some(open) = &mut self.overlay
    {
      open.invalidate();
    }
  }

  /// The reader dismissed the open overlay (a press outside it, the compositor's own
  /// dismissal): its app is told, and closes it.
  pub fn dismiss_popup(&mut self) {
    if let Some(open) = &self.overlay {
      trace::log(|| "[popup] dismiss".to_owned());
      tree::report::<Linux>(open.node.id.get(), Report::Dismiss);
      self.settle_focus();
    }
  }

  /// The backend placed the open overlay's surface with its top left at `x`, `y` (window
  /// pixels).
  pub fn popup_placed(&mut self, x: f32, y: f32) {
    if let Some(open) = &mut self.overlay {
      open.placed(x, y);
    }
  }

  /// The open overlay, where one is open.
  pub const fn popup(&self) -> Option<&Open> {
    self.overlay.as_ref()
  }

  /// The open overlay, where one is open, to change.
  pub const fn popup_mut(&mut self) -> Option<&mut Open> {
    self.overlay.as_mut()
  }

  /// The scale the open overlay is painted at: its surface's own where the compositor told it
  /// one, the window's otherwise.
  pub fn popup_scale(&self) -> f64 {
    self
      .overlay
      .as_ref()
      .and_then(Open::scale)
      .unwrap_or(self.scale)
  }

  /// The open overlay's surface prefers `scale`, the compositor said: it is painted at it.
  pub fn set_popup_scale(&mut self, scale: f64) {
    if let Some(open) = &mut self.overlay {
      trace::log(|| format!("[popup] scale {scale}"));
      open.set_scale(scale);
    }
  }

  /// The open overlay's frame, where it changed, at [`Shell::popup_scale`].
  pub fn popup_frame(&mut self) -> Option<Frame<'_>> {
    let scale = self.popup_scale();
    let marks = Marks {
      focus: self.focus.clone().filter(|_| self.focus_visible),
      focused: self.focus.clone(),
      link: None,
    };
    let started = Instant::now();
    let frame = self.overlay.as_mut()?.frame(scale, &marks);

    if frame.is_some() {
      self.painted_at = Some(started);
    }

    frame
  }

  /// Whether the open overlay wants a frame.
  pub fn popup_wants_frame(&self) -> bool {
    self.overlay.as_ref().is_some_and(Open::wants_frame)
  }

  /// The adornments the window shows now, each on a surface of its own that takes no input:
  /// a drag's icon, the focused entry's touch selection handles and the magnifier (neither
  /// while an overlay is open).
  pub fn adornments(&self) -> Vec<Shown> {
    let mut shown = Vec::new();

    if let Some(icon) = self.drag_out.as_ref().and_then(|drag| drag.icon.as_ref()) {
      shown.push(Shown {
        adornment: Adornment::DragIcon,
        size: icon.size(),
        placement: Placement::Pointer {
          hotspot: icon.hotspot(),
        },
        version: 0,
      });
    }

    if self.overlay.is_some() {
      return shown;
    }

    if let Some(handles) = &self.handles
      && handles.entry.is_alive()
      && is_focused(&handles.entry)
    {
      for (handle, x, top, height) in handles.entry.handle_points(handles.dragged) {
        shown.push(Shown {
          adornment: Adornment::Handle(handle),
          size: HANDLE_SIZE,
          placement: Placement::Below(handle.anchor(x, top, height)),
          version: 0,
        });
      }
    }

    if let Some(magnifier) = &self.magnifier {
      shown.push(Shown {
        adornment: Adornment::Magnifier,
        size: MAGNIFIER_SIZE,
        placement: magnifier.placement(),
        version: self.frames,
      });
    }

    shown
  }

  /// Paints `adornment` at `scale`, a drag's icon on its entry's fill where `opaque` (the
  /// screen composites nothing through it): its pixels, premultiplied, where it shows.
  pub fn paint_adornment(
    &mut self,
    adornment: Adornment,
    scale: f64,
    opaque: bool,
  ) -> Option<&vello_cpu::Pixmap> {
    let shown = self
      .adornments()
      .into_iter()
      .find(|shown| shown.adornment == adornment)?;
    let (width, height) = shown.size;
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "pixels"
    )]
    let device = (
      (f64::from(width) * scale).round().max(1.) as u16,
      (f64::from(height) * scale).round().max(1.) as u16,
    );
    let region = Rect {
      x: 0.,
      y: 0.,
      width: width as f32,
      height: height as f32,
    };
    let icon = self.drag_out.as_ref().and_then(|drag| drag.icon.as_ref());
    let magnifier = self.magnifier.as_ref();
    let focused = magnifier.is_some_and(|magnifier| is_focused(&magnifier.entry));
    let started = Instant::now();
    let renderer = self.adornment_renderers.entry(adornment).or_default();

    renderer.paint(
      None,
      device,
      scale,
      region,
      (0., 0.),
      &Marks::default(),
      &mut |canvas| match adornment {
        Adornment::DragIcon => {
          if let Some(icon) = icon {
            icon.paint(canvas, opaque);
          }
        }
        Adornment::Handle(handle) => handle.paint(canvas),
        Adornment::Magnifier => {
          if let Some(magnifier) = magnifier {
            magnifier.paint(canvas, focused);
          }
        }
      },
    );

    self.painted_at = Some(started);
    skene_foundation::seam::activity::frame();

    Some(&renderer.pixmap)
  }

  /// The backend took `adornment`'s surface down: what painted it goes, and the screen changes
  /// as a frame changes it.
  pub fn drop_adornment(&mut self, adornment: Adornment) {
    self.adornment_renderers.remove(&adornment);
    self.painted_at = Some(Instant::now());
    skene_foundation::seam::activity::frame();
  }

  /// The touch selection handle a finger down at `at` (window pixels) takes, where one does,
  /// and where it points: its x and its line's middle.
  fn handle_at(&self, at: (f32, f32)) -> Option<(Handle, Node, (f32, f32))> {
    let handles = self.handles.as_ref()?;

    if self.overlay.is_some() || !is_focused(&handles.entry) {
      return None;
    }

    handles
      .entry
      .handle_points(None)
      .into_iter()
      .find(|(handle, x, top, _)| handle.takes(at.0 - x, at.1 - top))
      .map(|(handle, x, top, height)| (handle, Rc::clone(&handles.entry), (x, top + height / 2.)))
  }

  /// Shows the magnifier over `entry`'s text about `x` (window pixels) on the line its caret
  /// is on, as GTK's entry shows it while a finger moves there.
  fn magnify(&mut self, entry: &Node, x: f32) {
    if let Some((top, height)) = entry.caret_line() {
      self.magnifier = Some(Magnifier {
        entry: Rc::clone(entry),
        center: (x, top + height / 2.),
        line: Rect {
          x,
          y: top,
          width: 1.,
          height,
        },
      });
    }
  }

  /// The pointer moved to `x`, `y` over the open overlay (its surface's coordinates).
  pub fn popup_motion(&mut self, x: f32, y: f32) {
    let Some(open) = &self.overlay else {
      return;
    };
    let frame = open.node.frame.get();
    let at = (x + frame.x, y + frame.y);

    // Seen over the popup, the pointer is where it is in the window's geometry too.
    trace::pointer(at.0, at.1);
    self.pointer = Some(at);
    self.pointer_in_overlay = open.contains(at.0, at.1);
    self.refresh_hover();

    if let Some(press) = &self.press
      && press.captured
      && let Some(owner) = press.owner.clone()
    {
      self.report_pointer_at(&owner, PointerPhase::Move, 0);

      return;
    }

    self.report_pointer(PointerPhase::Move, 0);
  }

  /// A button was pressed or released at `x`, `y` in the open overlay's coordinates: on it,
  /// it acts there; a press on its anchor is the anchor's; a press anywhere else dismisses it.
  pub fn popup_button(&mut self, down: bool, x: f32, y: f32) {
    count_input();

    let Some(open) = &self.overlay else {
      return;
    };
    let frame = open.node.frame.get();
    let at = (x + frame.x, y + frame.y);
    let on_overlay = open.contains(at.0, at.1);
    let on_anchor = open.on_anchor(at.0, at.1);

    self.pointer = Some(at);
    self.pointer_in_overlay = on_overlay;
    self.refresh_hover();

    if on_overlay || (!down && self.press.is_some()) {
      if down {
        let chain = self.hovered.clone();

        self.content_press(0, chain);
      } else {
        self.content_release(0);
      }
    } else if down && on_anchor {
      let chain = self.hovered.clone();

      self.content_press(0, chain);
    } else if down {
      self.dismiss_popup();
    }
  }

  /// The wheel turned over the open overlay.
  pub fn popup_wheel(&mut self, wheel: Wheel) {
    self.wheel(wheel);
  }

  /// The pointer moved to `x`, `y` (surface pixels); returns what the backend should do.
  pub fn pointer_motion(&mut self, x: f32, y: f32) -> Vec<Request> {
    let mut requests = Vec::new();
    let (ox, oy) = self.chrome.origin();
    let hit = self.chrome.hit(x, y, self.size.0, self.size.1);
    let hover_button = match hit {
      Hit::Button(button) => Some(button),
      _ => None,
    };

    if self.chrome.hover != hover_button {
      self.chrome.hover = hover_button;
      invalidate_all();
    }

    if let Some(press) = &mut self.press
      && let Some((px, py)) = press.title
      && ((px - x).abs() > 4. || (py - y).abs() > 4.)
    {
      press.title = None;
      self.title_click = None;
      requests.push(Request::Move);
    }

    let geometry = (x - ox, y - oy);

    self.pointer = matches!(hit, Hit::Content(..)).then_some(geometry);
    self.pointer_in_overlay = false;
    self.refresh_hover();

    let cursor = match hit {
      Hit::Edge(edge) => Cursor::Resize(edge),
      Hit::Content(..) => self.content_cursor(),
      _ => Cursor::Default,
    };

    if cursor != self.cursor {
      self.cursor = cursor;
      requests.push(Request::Cursor(cursor));
    }

    if let Some(bar) = &self.bar {
      if let Some(grab) = bar.grab {
        let (scroll, axis, base, travel) = (Rc::clone(&bar.scroll), bar.axis, bar.base, bar.travel);
        let at = match axis {
          Axis::Horizontal => geometry.0,
          Axis::Vertical => geometry.1,
        };

        self.scroll_thumb_to(&scroll, axis, base, travel, at - grab);
      }

      return requests;
    }

    let on_selection = self
      .press
      .as_ref()
      .and_then(|press| press.owner.clone().zip(press.on_selection));

    if let Some((owner, start)) = on_selection {
      if (geometry.0 - start.0).abs() > DRAG_THRESHOLD
        || (geometry.1 - start.1).abs() > DRAG_THRESHOLD
      {
        requests.extend(self.start_text_drag(&owner, start));
      }

      return requests;
    }

    if let Some(press) = &self.press
      && let Some(owner) = press.owner.clone()
    {
      if owner.is_entry() && press.button_pressed == 0 {
        owner.drag_entry(owner.window_rect(), geometry);
      }

      if press.captured {
        self.pointer = Some(geometry);
        self.report_pointer_at(&owner, PointerPhase::Move, 0);

        return requests;
      }
    }

    self.report_pointer(PointerPhase::Move, 0);

    requests
  }

  fn content_cursor(&self) -> Cursor {
    for node in &self.hovered {
      match node.spec.kind {
        NodeKind::Editable { .. } => return Cursor::Text,
        NodeKind::Text => {
          if let Some((x, y)) = self.pointer
            && Self::link_index_at(node, x, y).is_some()
          {
            return Cursor::Pointer;
          }
        }
        _ => {}
      }
    }

    Cursor::Default
  }

  /// Reports a pointer event at the deepest node under the pointer, where a node it climbs
  /// through hears pointer events: the verdict the handlers gave.
  fn report_pointer(&mut self, phase: PointerPhase, button: u8) -> Prevent {
    let Some(target) = self.hovered.first().cloned() else {
      return Prevent::None;
    };

    if !self.hovered.iter().any(|node| node.interest.get().pointer) {
      return Prevent::None;
    }

    self.report_pointer_at(&target, phase, button)
  }

  /// Reports a pointer event at `target`, the pointer where it is: the verdict the handlers
  /// gave.
  fn report_pointer_at(&mut self, target: &Node, phase: PointerPhase, button: u8) -> Prevent {
    let (x, y) = self.pointer.unwrap_or_default();
    let rect = target.window_rect();
    let answer = tree::report::<Linux>(
      target.id.get(),
      Report::Pointer(PointerEvent {
        phase,
        x: x - rect.x,
        y: y - rect.y,
        window_x: x,
        window_y: y,
        button,
        modifiers: self.modifiers,
      }),
    );

    self.settle_focus();

    match answer {
      Answer::Event { prevented } => prevented,
      _ => Prevent::None,
    }
  }

  /// The pointer left the window.
  pub fn pointer_leave(&mut self) {
    self.pointer = None;
    self.chrome.hover = None;
    self.refresh_hover();
    invalidate_all();
  }

  /// How many presses in a row a press at `at` on `node` makes: within the double-click time
  /// and distance of the last one on the same node.
  fn count_click(&mut self, node: &Node, at: (f32, f32)) -> u8 {
    let now = Instant::now();
    let count = match &self.last_click {
      Some((then, (x, y), count, last))
        if Rc::ptr_eq(last, node)
          && now.duration_since(*then) < DOUBLE_CLICK
          && (x - at.0).abs() < 5.
          && (y - at.1).abs() < 5. =>
      {
        count % 3 + 1
      }
      _ => 1,
    };

    self.last_click = Some((now, at, count, Rc::clone(node)));

    count
  }

  /// A button (0 primary, 1 middle, 2 secondary) was pressed or released at `time`.
  pub fn pointer_button(&mut self, button: u8, down: bool, at: (f32, f32)) -> Vec<Request> {
    count_input();

    if down {
      self.stop_fling();
      // A pointer's press ends a touch's selection handles, as in GTK's entry.
      self.handles = None;
    }

    let mut requests = Vec::new();
    let hit = self.chrome.hit(at.0, at.1, self.size.0, self.size.1);

    trace::log(|| {
      format!(
        "[button] {button} {} at {at:?}: {hit:?}, over {:?}",
        if down { "down" } else { "up" },
        self
          .hovered
          .iter()
          .map(|node| node.spec.kind)
          .collect::<Vec<_>>()
      )
    });

    // A press anywhere on the window but the open overlay's anchor dismisses the overlay, and
    // goes no further; a press on the overlay reaches its own surface.
    if down
      && let Some(open) = &self.overlay
      && !matches!(hit, Hit::Content(x, y) if open.on_anchor(x, y))
    {
      self.dismiss_popup();
      requests.extend(self.take_requests());

      return requests;
    }

    if down {
      match (button, hit) {
        (0, Hit::Edge(edge)) => requests.push(Request::Resize(edge)),
        (0, Hit::Title) => {
          let now = Instant::now();
          let double = self.title_click.is_some_and(|(then, (px, py))| {
            now.duration_since(then) < Duration::from_millis(400)
              && (px - at.0).abs() < 6.
              && (py - at.1).abs() < 6.
          });

          if double {
            self.title_click = None;
            requests.push(Request::ToggleMaximize);
          } else {
            self.title_click = Some((now, at));
            self.press = Some(Press {
              target: None,
              owner: None,
              title: Some(at),
              button: None,
              button_pressed: 0,
              captured: false,
              on_selection: None,
            });
          }
        }
        (2, Hit::Title) if self.chrome.capabilities.window_menu => {
          requests.push(Request::ShowMenu(at.0, at.1));
        }
        (0, Hit::Button(pressed)) => {
          self.chrome.pressed = Some(pressed);
          self.press = Some(Press {
            target: None,
            owner: None,
            title: None,
            button: Some(pressed),
            button_pressed: 0,
            captured: false,
            on_selection: None,
          });
          invalidate_all();
        }
        (_, Hit::Content(..)) => {
          let chain = self.hovered.clone();

          if !self.bar_press(button, &chain) {
            self.content_press(button, chain);
          }
        }
        _ => {}
      }

      requests.extend(self.take_requests());

      return requests;
    }

    if let Some(bar) = self.bar.take() {
      for node in &bar.chain {
        Self::set_state(node, State::Pressed, false);
      }

      return requests;
    }

    let Some(press) = self.press.take() else {
      if matches!(hit, Hit::Content(..)) {
        self.report_pointer(PointerPhase::Up, button);
      }

      return requests;
    };

    if let Some(pressed) = press.button {
      self.chrome.pressed = None;
      invalidate_all();

      if hit == Hit::Button(pressed) {
        requests.push(match pressed {
          Button::Close => Request::Close,
          Button::Maximize => Request::ToggleMaximize,
          Button::Minimize => Request::Minimize,
        });
      }

      return requests;
    }

    if press.title.is_some() {
      return requests;
    }

    self.press = Some(press);
    self.content_release(button);
    requests.extend(self.take_requests());

    requests
  }

  /// A press of `button` on a scroll bar in `chain` (the deepest node under the pointer
  /// first), where one is under it: a primary press on the thumb drags it; one on the track
  /// jumps the thumb to the point and drags it from there where the desktop says a primary
  /// press warps ([`crate::environment::primary_warps_slider`]) and pages toward the point
  /// otherwise, a middle press or one with `Shift` doing the other. Focus stays, and nothing
  /// else hears the press. Whether the press was a bar's.
  fn bar_press(&mut self, button: u8, chain: &[Node]) -> bool {
    let Some((track, axis)) = chain
      .iter()
      .find_map(|node| node.bar_axis().map(|axis| (Rc::clone(node), axis)))
    else {
      return false;
    };
    let (Some(scroll), Some(thumb), Some((x, y))) = (
      track.parent(),
      track.children_ref().first().cloned(),
      self.pointer,
    ) else {
      return true;
    };

    if button > 1 {
      return true;
    }

    self.modality = Modality::Pointer;

    let along = |rect: Rect| match axis {
      Axis::Horizontal => (rect.x, rect.width),
      Axis::Vertical => (rect.y, rect.height),
    };
    let at = match axis {
      Axis::Horizontal => x,
      Axis::Vertical => y,
    };
    let (start, length) = along(thumb.window_rect());
    let travel = thumb.thumb.get().map_or(0., |(_, travel)| travel);
    let base = start - travel * scroll.scrolled_fraction(axis);
    let on_thumb = chain.iter().any(|node| Rc::ptr_eq(node, &thumb));
    let primary = button == 0 && !self.modifiers.shift;
    let grab = if on_thumb && button == 0 {
      Some(at - start)
    } else if primary == crate::environment::primary_warps_slider() {
      self.scroll_thumb_to(&scroll, axis, base, travel, at - length / 2.);

      Some(length / 2.)
    } else {
      let page = match axis {
        Axis::Horizontal => scroll.frame.get().width,
        Axis::Vertical => scroll.frame.get().height,
      } * PAGE;
      let delta = if at < start {
        -page
      } else if at >= start + length {
        page
      } else {
        0.
      };

      match axis {
        Axis::Horizontal => self.scroll_by(&scroll, delta, 0.),
        Axis::Vertical => self.scroll_by(&scroll, 0., delta),
      };

      None
    };

    trace::log(|| {
      format!(
        "[bar] {axis:?} press {button} at {at:.1}: {}",
        match grab {
          Some(grab) => format!("drags the thumb held {grab:.1} in"),
          None => "pages".to_owned(),
        }
      )
    });

    for node in chain {
      Self::set_state(node, State::Pressed, true);
    }

    self.bar = Some(BarPress {
      scroll,
      axis,
      base,
      travel,
      grab,
      chain: chain.to_vec(),
    });

    true
  }

  /// Scrolls `scroll` along `axis` so its bar's thumb, which starts at `base` (window pixels)
  /// with the box at its start and moves `travel` along its track, starts at `start`.
  fn scroll_thumb_to(&mut self, scroll: &Node, axis: Axis, base: f32, travel: f32, start: f32) {
    let fraction = if travel > 0. {
      ((start - base) / travel).clamp(0., 1.)
    } else {
      0.
    };
    let frame = scroll.frame.get();
    let (width, height) = scroll.extent.get();
    let (sx, sy) = scroll.scroll.get();

    match axis {
      Axis::Horizontal => {
        let target = fraction * (width - frame.width).max(0.);

        self.scroll_by(scroll, target - sx, 0.);
      }
      Axis::Vertical => {
        let target = fraction * (height - frame.height).max(0.);

        self.scroll_by(scroll, 0., target - sy);
      }
    }
  }

  /// A press of `button` on `chain` (the deepest node under the pointer first): the pressed
  /// states, focus moved to what the press lands on (a caption beside a control focusing the
  /// control), a text entry's caret placed, and the press reported; a control that drags
  /// keeps the pointer where its handler answered it.
  fn content_press(&mut self, button: u8, chain: Vec<Node>) {
    self.modality = Modality::Pointer;

    let mut on_selection = None;

    // A press on a link run is the text's, inside an acting box too.
    let link = chain
      .first()
      .zip(self.pointer)
      .and_then(|(target, (x, y))| Self::link_index_at(target, x, y));
    let caption = if link.is_some() {
      None
    } else {
      chain
        .iter()
        .take_while(|node| !node.spec.is_interactive())
        .find(|node| node.spec.interaction == Interaction::Answering)
        .cloned()
    };
    let owner = if link.is_some() || caption.is_some() {
      None
    } else {
      Self::owner(&chain)
    };

    if button == 0 {
      for node in &chain {
        Self::set_state(node, State::Pressed, true);
      }

      match (&link, chain.first(), &caption) {
        // A press on a link run focuses it, as a press on an anchor does.
        (Some(link), Some(target), _) => self.focus_link(target, *link, false),
        (_, _, Some(caption)) => {
          let control = tree::control_of::<Linux>(caption.id.get()).and_then(crate::node_of);

          self.set_focus(control, false);
        }
        _ => self.set_focus_to(owner.clone(), false, false),
      }

      if let Some(owner) = &owner
        && owner.is_entry()
        && let Some((x, y)) = self.pointer
      {
        let clicks = self.count_click(owner, (x, y));
        let extend = self.modifiers.shift;
        let rect = owner.window_rect();

        // A press on the selection may drag it (GTK's entry, a browser's field); it places
        // the caret only where it is released unmoved.
        if clicks == 1
          && !extend
          && !owner.contents.borrow().secure
          && owner.entry_selection_contains(rect, (x, y))
        {
          on_selection = Some((x, y));
        } else {
          owner.press_entry(rect, (x, y), clicks, extend);
        }
      }
    } else if button == 1
      && let Some(owner) = owner.as_ref().filter(|owner| owner.is_entry())
      && let Some((x, y)) = self.pointer
    {
      // A middle press pastes the primary selection where it lands.
      self.set_focus(Some(Rc::clone(owner)), false);
      owner.press_entry(owner.window_rect(), (x, y), 1, false);
      owner.paste(crate::clipboard::Selection::Primary);
    }

    self.press = Some(Press {
      target: chain.first().cloned(),
      owner: owner.clone(),
      title: None,
      button: None,
      button_pressed: button,
      captured: false,
      on_selection,
    });

    let prevented = self.report_pointer(PointerPhase::Down, button);

    // A secondary press asks for a context menu where it lands, as on a press in GTK and
    // Chromium on Linux.
    if button == 2
      && let (Some(target), Some(at)) = (chain.first(), self.pointer)
    {
      self.context_menu_at(&Rc::clone(target), at, MenuSource::Pointer);
    }

    let drags = owner.as_ref().is_some_and(|owner| {
      matches!(
        owner.spec.interaction,
        Interaction::Control { drag: Some(_) }
      )
    });

    if button == 0
      && drags
      && prevented != Prevent::None
      && let Some(press) = &mut self.press
    {
      press.captured = true;
    }
  }

  /// The release of `button` ending the press in progress: the pressed states cleared, the
  /// release reported, and a click where it lands on the owner the press landed on (or a link
  /// run followed).
  fn content_release(&mut self, button: u8) {
    let Some(press) = self.press.take() else {
      self.report_pointer(PointerPhase::Up, button);

      return;
    };

    if let Some(target) = &press.target {
      for node in std::iter::once(Rc::clone(target)).chain(target.ancestors()) {
        Self::set_state(&node, State::Pressed, false);
      }
    }

    match (&press.owner, press.captured) {
      (Some(owner), true) => {
        let owner = Rc::clone(owner);

        self.report_pointer_at(&owner, PointerPhase::Up, button);
      }
      _ => {
        self.report_pointer(PointerPhase::Up, button);
      }
    }

    if let (Some(owner), Some(start)) = (&press.owner, press.on_selection) {
      // Released on the selection without dragging it: the caret goes where it was pressed.
      owner.press_entry(owner.window_rect(), start, 1, false);
    }

    let over = Self::owner(&self.hovered);

    if press.button_pressed == 0
      && let (Some(owner), Some(over)) = (&press.owner, &over)
      && Rc::ptr_eq(owner, over)
      && !owner.is_entry()
    {
      let owner = Rc::clone(owner);

      self.click(&owner);
    }

    if press.button_pressed == 0
      && press.owner.is_none()
      && let Some(target) = &press.target
      && target.spec.kind == NodeKind::Text
      && let Some((x, y)) = self.pointer
      && self
        .hovered
        .first()
        .is_some_and(|hovered| Rc::ptr_eq(hovered, target))
      && let Some(link) = Self::link_index_at(target, x, y)
    {
      Self::follow(target, link);
    }
  }

  /// The link run of the text `node` at `x`, `y` (window pixels), by its place among the
  /// text's links.
  fn link_index_at(node: &Node, x: f32, y: f32) -> Option<usize> {
    if node.spec.kind != NodeKind::Text {
      return None;
    }

    let cell = node.text.borrow().clone()?;
    let rect = node.window_rect();
    let laid = cell.laid(Some(node.laid_width.get()))?;
    let (x, y) = (x - rect.x, y - rect.y);

    cell.links().iter().position(|(range, _)| {
      laid
        .range_rects(range.clone())
        .iter()
        .any(|run| x >= run.x && y >= run.y && x < run.x + run.width && y < run.y + run.height)
    })
  }

  /// Follows link run `link` of the text `node`.
  fn follow(node: &Node, link: usize) {
    if let Some((_, destination)) = node.links().get(link) {
      trace::log(|| format!("[link] follow {destination}"));
      tree::report::<Linux>(node.id.get(), Report::Follow(Rc::clone(destination)));
    }
  }

  /// Starts dragging the selection of the text entry `entry` pressed at `at` (window pixels):
  /// the press ends without a click, and the backend is asked to start the drag.
  fn start_text_drag(&mut self, entry: &Node, at: (f32, f32)) -> Option<Request> {
    let read_only = entry.contents.borrow().read_only || entry.access.borrow().disabled;
    let (range, text) = entry.with_entry(|state| {
      let range = state.editor.selected();
      let text = state.editor.text()[range.clone()].to_owned();

      (range, text)
    })?;

    if let Some(press) = self.press.take()
      && let Some(target) = &press.target
    {
      for node in std::iter::once(Rc::clone(target)).chain(target.ancestors()) {
        Self::set_state(&node, State::Pressed, false);
      }
    }

    let icon = entry.drag_icon(at, &Palette::now());

    trace::log(|| {
      format!(
        "[dnd] drag {text:?} from an entry, its icon {:?} with the pointer at {:?}",
        icon.as_ref().map(crate::adornment::DragIcon::size),
        icon.as_ref().map(crate::adornment::DragIcon::hotspot),
      )
    });
    self.drag_out = Some(DragOut {
      entry: Rc::clone(entry),
      range,
      text,
      moves: !read_only,
      icon,
    });

    Some(Request::StartDrag)
  }

  /// The text a drag from the window carries, and whether a drop may move it.
  pub fn dragged(&self) -> Option<(&str, bool)> {
    self
      .drag_out
      .as_ref()
      .map(|drag| (drag.text.as_str(), drag.moves))
  }

  /// The drag from the window ended, with what the drop did (`None` where nothing took it):
  /// a move takes the text out of the entry it came from, as GTK's entry does.
  pub fn drag_ended(&mut self, action: Option<DropAction>) {
    let Some(drag) = self.drag_out.take() else {
      return;
    };

    trace::log(|| format!("[dnd] the drag ended: {action:?}"));

    if action == Some(DropAction::Move) && drag.moves && drag.entry.is_alive() {
      drag.entry.with_entry(|entry| {
        entry.editor.replace(
          Some(drag.range.clone()),
          "",
          skene_foundation::seam::content::Unit::Utf8,
        )
      });
    }

    crate::access::invalidate();
  }

  /// The node a drop at `at` (window pixels) offering `offered` lands on: the deepest editable
  /// text entry where text is offered, else the deepest node that takes drops; and whether it
  /// is a text entry.
  fn drop_target(&self, at: (f32, f32), offered: Offered) -> Option<(Node, bool)> {
    let chain = self
      .root
      .as_ref()
      .map(|root| Self::hit_in(root, at.0, at.1))
      .unwrap_or_default();

    for node in chain {
      if node.is_entry()
        && offered.text
        && !node.contents.borrow().read_only
        && !node.access.borrow().disabled
      {
        return Some((node, true));
      }

      if node.interest.get().drop && (offered.text || offered.uris) {
        return Some((node, false));
      }
    }

    None
  }

  /// A drag offering `offered` is over the window at `x`, `y` (surface pixels): what a drop
  /// there would do, `None` where nothing takes it. A text entry shows where the text would go
  /// (no caret over its own selection), as GTK's does; a drag from the same entry moves its
  /// text, any other copies.
  pub fn drag_over(&mut self, x: f32, y: f32, offered: Offered) -> Option<DropAction> {
    let (ox, oy) = self.chrome.origin();
    let at = (x - ox, y - oy);
    let target = self.drop_target(at, offered);

    if let Some((previous, true)) = &self.drop_over
      && target
        .as_ref()
        .is_none_or(|(node, _)| !Rc::ptr_eq(node, previous))
    {
      previous.set_drop_caret(None);
    }

    let action = target.as_ref().map(|(node, entry)| {
      if *entry {
        node.set_drop_caret(Some(at));
      }

      match &self.drag_out {
        Some(drag) if *entry && drag.moves && Rc::ptr_eq(&drag.entry, node) => DropAction::Move,
        _ => DropAction::Copy,
      }
    });

    self.drop_over = target;

    action
  }

  /// The drag left the window: no entry shows where it would drop.
  pub fn drag_left(&mut self) {
    if let Some((node, true)) = self.drop_over.take() {
      node.set_drop_caret(None);
    }
  }

  /// What a drop at `x`, `y` (surface pixels) offering `offered` takes: a text entry its text,
  /// a node that takes drops its URIs where it offers them, else its text.
  pub fn drop_wants(&mut self, x: f32, y: f32, offered: Offered) -> Option<Wanted> {
    let (ox, oy) = self.chrome.origin();
    let (_, entry) = self.drop_target((x - ox, y - oy), offered)?;

    Some(if !entry && offered.uris {
      Wanted::Uris
    } else {
      Wanted::Text
    })
  }

  /// What was dropped at `x`, `y` (surface pixels) arrived: a text entry inserts the text
  /// there (in place of its selection where it lands on it), a node that takes drops is
  /// offered it; what the drop did, `None` where nothing took it.
  pub fn drop_data(
    &mut self,
    x: f32,
    y: f32,
    dropped: skene_foundation::seam::report::Dropped,
  ) -> Option<DropAction> {
    count_input();

    let (ox, oy) = self.chrome.origin();
    let at = (x - ox, y - oy);
    let offered = Offered {
      text: dropped.text.is_some(),
      uris: !dropped.uris.is_empty(),
    };
    let action = self.drag_over(x, y, offered);

    self.drag_left();

    let (node, entry) = self.drop_target(at, offered)?;

    trace::log(|| match &dropped.text {
      Some(text) if entry && node.contents.borrow().secure => {
        let marks = unicode_segmentation::UnicodeSegmentation::graphemes(&**text, true).count();

        format!("[dnd] dropped at {at:?} into a secure entry: {marks} marks")
      }
      _ => format!("[dnd] dropped at {at:?}: {dropped:?}"),
    });

    if entry {
      let text = dropped.text?;
      let inserted = node.drop_text(node.window_rect(), at, &text)?;

      // A move within one entry takes the text out of where it was, after the drop shifted it.
      if let Some(drag) = &mut self.drag_out
        && Rc::ptr_eq(&drag.entry, &node)
        && inserted <= drag.range.start
      {
        drag.range = drag.range.start + text.len()..drag.range.end + text.len();
      }

      crate::access::invalidate();

      return action;
    }

    let deepest = self
      .root
      .as_ref()
      .map(|root| Self::hit_in(root, at.0, at.1))
      .unwrap_or_default()
      .first()
      .cloned()
      .unwrap_or(node);
    let rect = deepest.window_rect();
    let answer = tree::report::<Linux>(
      deepest.id.get(),
      Report::Drop(skene_foundation::seam::report::DropEvent {
        x: at.0 - rect.x,
        y: at.1 - rect.y,
        window_x: at.0,
        window_y: at.1,
        dropped,
      }),
    );

    self.settle_focus();
    crate::access::invalidate();

    match answer {
      Answer::Event { prevented } if prevented != Prevent::None => Some(DropAction::Copy),
      _ => None,
    }
  }

  /// Asks for a context menu at `target`, at `at` (window pixels): it climbs to the handlers;
  /// the verdict they gave.
  fn context_menu_at(&mut self, target: &Node, at: (f32, f32), source: MenuSource) -> Prevent {
    let rect = target.window_rect();

    trace::log(|| format!("[menu] {source:?} at {at:?}"));

    let answer = tree::report::<Linux>(
      target.id.get(),
      Report::ContextMenu(ContextMenuEvent {
        x: at.0 - rect.x,
        y: at.1 - rect.y,
        window_x: at.0,
        window_y: at.1,
        source,
      }),
    );

    self.settle_focus();
    crate::access::invalidate();

    match answer {
      Answer::Event { prevented } => prevented,
      _ => Prevent::None,
    }
  }

  /// A finger went down at `x`, `y` (surface pixels) as touch point `id` at `time` (the
  /// compositor's milliseconds): the first finger down acts, the others wait. What it does is decided as it moves and lifts, as GTK
  /// decides: a tap clicks, a drag scrolls the scroll box under it (flinging on as it lifts) or
  /// selects in a text entry or drives a control that drags, a long press asks for a context
  /// menu; on the title bar it moves the window, on its buttons it acts, on an edge it resizes.
  pub fn touch_down(&mut self, id: i32, x: f32, y: f32, time: u32) -> Vec<Request> {
    count_input();

    let mut requests = Vec::new();

    if self.touch.is_some() {
      return requests;
    }

    self.stop_fling();
    self.modality = Modality::Pointer;

    let hit = self.chrome.hit(x, y, self.size.0, self.size.1);

    trace::log(|| format!("[touch] {id} down at ({x}, {y}): {hit:?}"));

    if let Some(open) = &self.overlay
      && !matches!(hit, Hit::Content(x, y) if open.on_anchor(x, y))
    {
      self.dismiss_popup();
      requests.extend(self.take_requests());

      return requests;
    }

    let (ox, oy) = self.chrome.origin();
    let at = (x - ox, y - oy);
    let mut touch = Touch {
      id,
      start: at,
      time,
      at,
      moved: false,
      chain: Vec::new(),
      role: TouchRole::Spent,
      long_press: None,
      tracker: crate::kinetic::Tracker::default(),
    };

    match hit {
      Hit::Title => touch.role = TouchRole::Title,
      Hit::Button(button) => {
        self.chrome.pressed = Some(button);
        touch.role = TouchRole::Button(button);
        invalidate_all();
      }
      Hit::Edge(edge) => requests.push(Request::Resize(edge)),
      Hit::Content(..) if let Some((handle, entry, from)) = self.handle_at(at) => {
        trace::log(|| format!("[touch] takes the {handle:?} handle"));

        if let Some(handles) = &mut self.handles {
          handles.dragged = Some(handle);
        }

        touch.role = TouchRole::Handle {
          handle,
          entry,
          from,
          shifted: false,
        };
      }
      Hit::Content(..) => {
        let chain = self.hit(at.0, at.1);

        for node in &chain {
          Self::set_state(node, State::Pressed, true);
        }

        touch.role = self.touch_role(&chain, at);
        touch.chain = chain;

        let due = id;

        touch.long_press = Some(crate::runtime::schedule(
          LONG_PRESS,
          Box::new(move || LONG_PRESSED.with(|pressed| pressed.set(Some(due)))),
        ));
      }
      Hit::Outside => {}
    }

    self.touch = Some(touch);
    requests.extend(self.take_requests());

    requests
  }

  /// What a finger down at `at` (window pixels) on `chain` starts as: in a text entry it places
  /// the caret (or keeps a selection it went down on), on a control that drags it presses as
  /// the primary button does.
  fn touch_role(&mut self, chain: &[Node], at: (f32, f32)) -> TouchRole {
    let Some(owner) = Self::owner(chain) else {
      return TouchRole::Pending;
    };

    if owner.is_entry() && !owner.access.borrow().disabled {
      let rect = owner.window_rect();
      let on_selection = is_focused(&owner) && owner.entry_selection_contains(rect, at);

      self.set_focus_to(Some(Rc::clone(&owner)), false, false);

      let mut placed = None;

      if !on_selection {
        let clicks = self.count_click(&owner, at);

        owner.press_entry(rect, at, clicks, false);
        placed = (clicks == 1).then(Instant::now);
      }

      self.handles = Some(Handles {
        entry: Rc::clone(&owner),
        placed,
        dragged: None,
      });

      return TouchRole::Selecting {
        entry: owner,
        on_selection,
      };
    }

    let drags = matches!(
      owner.spec.interaction,
      Interaction::Control { drag: Some(_) }
    );

    if drags {
      let hovered = std::mem::replace(&mut self.hovered, chain.to_vec());

      self.pointer = Some(at);
      self.content_press(0, chain.to_vec());
      self.hovered = hovered;

      if let Some(press) = &mut self.press {
        press.captured = true;
      }

      return TouchRole::Dragging(owner);
    }

    TouchRole::Pending
  }

  /// Touch point `id` moved to `x`, `y` (surface pixels) at `time`.
  pub fn touch_motion(&mut self, id: i32, x: f32, y: f32, time: u32) -> Vec<Request> {
    let mut requests = Vec::new();
    let (ox, oy) = self.chrome.origin();
    let at = (x - ox, y - oy);
    let Some(mut touch) = self.touch.take() else {
      return requests;
    };

    if touch.id != id {
      self.touch = Some(touch);

      return requests;
    }

    let previous = touch.at;

    touch.at = at;
    touch.time = time;

    if let TouchRole::Handle {
      handle,
      entry,
      from,
      shifted,
    } = &mut touch.role
    {
      let to = (from.0 + at.0 - touch.start.0, from.1 + at.1 - touch.start.1);
      let entry = Rc::clone(entry);

      *shifted |= entry.move_handle(*handle, to);
      self.magnify(&entry, to.0);
      self.touch = Some(touch);

      return requests;
    }

    if !touch.moved
      && ((at.0 - touch.start.0).abs() > TOUCH_SLOP || (at.1 - touch.start.1).abs() > TOUCH_SLOP)
    {
      touch.moved = true;
      touch.cancel_long_press();
      self.touch_starts_moving(&mut touch, &mut requests);
    }

    if touch.moved {
      match &touch.role {
        TouchRole::Selecting { entry, .. } => {
          let entry = Rc::clone(entry);

          entry.drag_entry(entry.window_rect(), at);
          self.magnify(&entry, at.0);
        }
        TouchRole::Dragging(owner) => {
          let owner = Rc::clone(owner);

          self.pointer = Some(at);
          self.report_pointer_at(&owner, PointerPhase::Move, 0);
        }
        TouchRole::Scrolling(node) => {
          let (dx, dy) = (previous.0 - at.0, previous.1 - at.1);
          let node = Rc::clone(node);

          touch.tracker.push(time, dx, dy);

          if node.is_entry() {
            node.scroll_entry(dy);
          } else {
            self.scroll_by(&node, dx, dy);
          }
        }
        _ => {}
      }
    }

    self.touch = Some(touch);

    requests
  }

  /// A touch moved past the slop: a pending one becomes a scroll of the nearest scroll box
  /// under it that moves that way (or nothing), a press on the title bar a move of the window.
  fn touch_starts_moving(&mut self, touch: &mut Touch, requests: &mut Vec<Request>) {
    match &touch.role {
      TouchRole::Title => {
        requests.push(Request::Move);
        touch.role = TouchRole::Spent;
      }
      TouchRole::Pending => {
        Self::release_chain(&touch.chain);

        let (dx, dy) = (touch.start.0 - touch.at.0, touch.start.1 - touch.at.1);
        let scrolls = touch
          .chain
          .iter()
          .find(|node| Self::can_scroll(node, dx, dy))
          .cloned();

        touch.role = match scrolls {
          Some(node) => {
            trace::log(|| "[touch] scrolls".to_owned());
            TouchRole::Scrolling(node)
          }
          None => TouchRole::Spent,
        };
      }
      TouchRole::Selecting {
        entry,
        on_selection: true,
      } => {
        // Dragged from a selection, a finger selects afresh from where it went down.
        let entry = Rc::clone(entry);
        let rect = entry.window_rect();

        entry.press_entry(rect, touch.start, 1, false);
        touch.role = TouchRole::Selecting {
          entry,
          on_selection: false,
        };
      }
      _ => {}
    }
  }

  /// Whether `node` is a scroll box that can move by `dx`, `dy`, or a text editor that
  /// scrolls itself vertically.
  fn can_scroll(node: &Node, dx: f32, dy: f32) -> bool {
    if node.spec.kind == (NodeKind::Editable { multiline: true }) {
      return dy != 0.;
    }

    let Some(skene_foundation::seam::node::LayoutKind::Scroll {
      horizontal,
      vertical,
    }) = node.layout_kind()
    else {
      return false;
    };
    let frame = node.frame.get();
    let (width, height) = node.extent.get();
    let (sx, sy) = node.scroll.get();
    let room = |scroll: f32, delta: f32, extent: f32, size: f32| {
      (delta > 0. && scroll < extent - size) || (delta < 0. && scroll > 0.)
    };

    (horizontal && room(sx, dx, width, frame.width))
      || (vertical && room(sy, dy, height, frame.height))
  }

  /// Clears the pressed state a touch set on `chain`.
  fn release_chain(chain: &[Node]) {
    for node in chain {
      Self::set_state(node, State::Pressed, false);
    }
  }

  /// Touch point `id` lifted at `time`.
  pub fn touch_up(&mut self, id: i32, time: u32) -> Vec<Request> {
    count_input();

    let mut requests = Vec::new();
    let Some(mut touch) = self.touch.take() else {
      return requests;
    };

    if touch.id != id {
      self.touch = Some(touch);

      return requests;
    }

    touch.cancel_long_press();
    trace::log(|| format!("[touch] {id} up, moved {}", touch.moved));
    self.magnifier = None;

    match touch.role {
      TouchRole::Handle {
        handle,
        entry,
        from,
        shifted,
      } => {
        let placed = self.handles.as_mut().and_then(|handles| {
          handles.dragged = None;
          handles.placed.take()
        });

        // A tap on a handle: soon after a tap placed the caret it selects the word there, as a
        // double tap; otherwise it asks for the menu, as GTK's handle shows its selection
        // bubble.
        if !shifted {
          if placed.is_some_and(|placed| placed.elapsed() < DOUBLE_CLICK) {
            entry.press_entry(entry.window_rect(), from, 2, false);
          } else {
            trace::log(|| format!("[touch] a tap on the {handle:?} handle"));
            self.context_menu_at(&entry, from, MenuSource::Touch);
          }
        }
      }
      TouchRole::Pending if !touch.moved => {
        Self::release_chain(&touch.chain);
        self.tap(&touch.chain, touch.start);
      }
      TouchRole::Selecting {
        entry,
        on_selection,
      } => {
        Self::release_chain(&touch.chain);

        if on_selection && !touch.moved {
          self.context_menu_at(&entry, touch.start, MenuSource::Touch);
        }
      }
      TouchRole::Dragging(_) => {
        Self::release_chain(&touch.chain);

        let hovered = std::mem::replace(&mut self.hovered, touch.chain.clone());

        self.pointer = Some(touch.at);
        self.content_release(0);
        self.hovered = hovered;
      }
      TouchRole::Scrolling(node) => {
        let velocity = touch.tracker.finish(time);
        let fling = crate::kinetic::Fling::new(velocity, Instant::now(), None);

        trace::log(|| format!("[fling] touch at {velocity:?} px/s"));

        if fling.moves() && !node.is_entry() {
          self.fling = Some((node, fling));
        }
      }
      TouchRole::Button(button) => {
        self.chrome.pressed = None;
        invalidate_all();

        let (ox, oy) = self.chrome.origin();
        let hit = self
          .chrome
          .hit(touch.at.0 + ox, touch.at.1 + oy, self.size.0, self.size.1);

        if hit == Hit::Button(button) {
          requests.push(match button {
            Button::Close => Request::Close,
            Button::Maximize => Request::ToggleMaximize,
            Button::Minimize => Request::Minimize,
          });
        }
      }
      _ => Self::release_chain(&touch.chain),
    }

    requests.extend(self.take_requests());

    requests
  }

  /// The compositor took the touches back (a gesture of its own): what they did is undone
  /// where it can be, and nothing more happens.
  pub fn touch_cancel(&mut self) {
    if let Some(mut touch) = self.touch.take() {
      touch.cancel_long_press();
      Self::release_chain(&touch.chain);
      trace::log(|| "[touch] cancelled".to_owned());
    }

    self.magnifier = None;

    if let Some(handles) = &mut self.handles {
      handles.dragged = None;
    }
  }

  /// A tap at `at` (window pixels) on `chain`: a primary press and release there, as a click.
  fn tap(&mut self, chain: &[Node], at: (f32, f32)) {
    let hovered = std::mem::replace(&mut self.hovered, chain.to_vec());
    let pointer = self.pointer.replace(at);

    trace::log(|| format!("[touch] tap at {at:?}"));
    self.content_press(0, chain.to_vec());
    self.content_release(0);
    self.hovered = hovered;
    self.pointer = pointer;
  }

  /// Runs a long press whose time came: a touch still down and unmoved asks for a context
  /// menu where it is.
  pub fn touch_timers(&mut self) {
    let Some(due) = LONG_PRESSED.with(std::cell::Cell::take) else {
      return;
    };
    let Some(touch) = self.touch.as_mut() else {
      return;
    };

    if touch.id != due || touch.moved {
      return;
    }

    touch.long_press = None;

    let pending = matches!(touch.role, TouchRole::Pending | TouchRole::Selecting { .. });

    if !pending {
      return;
    }

    let chain = std::mem::take(&mut touch.chain);
    let at = touch.at;

    touch.role = TouchRole::Spent;
    Self::release_chain(&chain);
    trace::log(|| format!("[touch] long press at {at:?}"));

    if let Some(target) = chain.first() {
      self.context_menu_at(target, at, MenuSource::Touch);
    }
  }

  /// A finger went down on the open overlay at `x`, `y` (its surface's coordinates): a press
  /// there, as the primary button's.
  pub fn popup_touch_down(&mut self, x: f32, y: f32) {
    self.popup_button(true, x, y);
  }

  /// A finger lifted from the open overlay at `x`, `y`: a release there.
  pub fn popup_touch_up(&mut self, x: f32, y: f32) {
    self.popup_button(false, x, y);
  }

  /// The wheel turned over the window: what is flinging stops, and the nearest box under the
  /// pointer that can scroll that way scrolls.
  pub fn wheel(&mut self, wheel: Wheel) {
    count_input();
    self.stop_fling();

    let (dx, dy) = match wheel {
      Wheel::Pixels(dx, dy) => (dx, dy),
      Wheel::Lines(dx, dy) => (dx * 48., dy * 48.),
    };

    self.scroll_hovered(dx, dy);
  }

  /// Scrolls the nearest box under the pointer that moves by `dx`, `dy`: which one moved.
  fn scroll_hovered(&mut self, dx: f32, dy: f32) -> Option<Node> {
    self
      .hovered
      .clone()
      .into_iter()
      .find(|node| (node.is_entry() && node.scroll_entry(dy)) || self.scroll_by(node, dx, dy))
  }

  /// Two fingers moved on a touchpad by `dx`, `dy` pixels at `time` (the compositor's
  /// milliseconds): what is flinging stops, the box the gesture moves (the first that moved
  /// under the pointer, kept while the fingers are down) scrolls, and the motion is kept for a
  /// fling once they lift.
  pub fn finger_scroll(&mut self, dx: f32, dy: f32, time: u32) {
    count_input();

    if let Some((node, fling)) = self.fling.take() {
      trace::log(|| "[fling] caught".to_owned());
      self.caught = Some(fling);
      self.latched = Some(node);
    }

    self.fingers.push(time, dx, dy);

    let latched = self.latched.clone().filter(|node| node.is_alive());
    let moved = match latched {
      Some(node) => {
        let moved = (node.is_entry() && node.scroll_entry(dy)) || self.scroll_by(&node, dx, dy);

        moved.then_some(node)
      }
      None => self.scroll_hovered(dx, dy),
    };

    if let Some(node) = moved {
      self.latched = Some(node);
    }
  }

  /// The fingers lifted off the touchpad at `time`: the box they moved goes on at their
  /// velocity, slowing down, unless it scrolls itself (a text entry) or they had stopped.
  pub fn finger_scroll_stop(&mut self, time: u32) {
    count_input();

    let velocity = self.fingers.finish(time);
    let Some(node) = self.latched.take() else {
      return;
    };

    if node.is_entry() || !node.is_alive() {
      return;
    }

    let now = Instant::now();
    let fling = crate::kinetic::Fling::new(velocity, now, self.caught.take().as_ref());

    trace::log(|| format!("[fling] start at {velocity:?} px/s"));

    if fling.moves() {
      self.fling = Some((node, fling));
    }
  }

  /// Stops what is flinging, where something is.
  pub fn stop_fling(&mut self) {
    self.caught = None;
    self.fingers.reset();
    self.latched = None;

    if self.fling.take().is_some() {
      trace::log(|| "[fling] stopped".to_owned());
    }
  }

  /// Moves what is flinging to where it is at `now`; it stops at the end of its scroll box.
  fn advance_fling(&mut self, now: Instant) {
    let Some((node, mut fling)) = self.fling.take() else {
      return;
    };
    let ((dx, dy), going) = fling.step(now);
    let moved = self.scroll_by(&node, dx, dy) || dx.abs() < 1. && dy.abs() < 1.;

    if going && moved && node.is_alive() {
      self.fling = Some((node, fling));
    } else {
      trace::log(|| "[fling] at rest".to_owned());
    }
  }

  /// A key was pressed or released: reported at the focus, where it climbs through the
  /// controls' behaviors and the boxes' handlers; its default (a text entry's editing, Tab, a
  /// box that acts answering as a command, a link followed, scrolling, `Escape`) runs unless a
  /// verdict prevents it.
  pub fn key(&mut self, input: &KeyInput) {
    count_input();

    let event = &input.event;

    // A key reaches only the window with the keyboard: where no window manager says so (a
    // bare X server), the first key does.
    if !self.window_focused {
      self.keyboard_focus(true);
    }

    if event.down {
      self.modality = Modality::Keyboard;
      // A key ends a touch's selection handles, as in GTK's entry.
      self.handles = None;
    }

    if event.down && event.key != "Tab" {
      self.focus_visible_on_key();
    }

    let target = self.focus.clone().or_else(|| self.root.clone());
    let Some(target) = target else {
      return;
    };

    trace::key(event, &target);

    if target.is_entry() && self.compose(&target, input) {
      self.after_key();

      return;
    }

    let answer = tree::report::<Linux>(target.id.get(), Report::Key(event.clone()));
    let prevented = match answer {
      Answer::Event { prevented } => prevented,
      _ => Prevent::None,
    };

    self.settle_focus();

    if prevented.prevents_platform() || self.shortcut(event) {
      self.after_key();

      return;
    }

    self.default_action(&target, input);
    self.after_key();
  }

  /// Clicks the shown, enabled node whose shortcut `event` presses (Control and the key,
  /// Shift where the shortcut says), where one has it; whether one did.
  fn shortcut(&mut self, event: &KeyEvent) -> bool {
    let modifiers = event.modifiers;

    if !event.down || event.repeat || !modifiers.control || modifiers.alt || modifiers.meta {
      return false;
    }

    let mut characters = event.key.chars();
    let (Some(key), None) = (characters.next(), characters.next()) else {
      return false;
    };
    let key = key.to_ascii_lowercase();
    let Some(root) = self.root.clone() else {
      return false;
    };
    let mut found = None;

    walk(&root, |node| {
      let facts = node.access.borrow();
      let wanted = facts.shortcut;

      if found.is_none()
        && node.is_shown()
        && !facts.disabled
        && wanted.is_some_and(|wanted| wanted.key == key && wanted.shift == modifiers.shift)
      {
        found = Some(Rc::clone(node));
      }
    });

    let Some(node) = found else {
      return false;
    };

    self.click(&node);

    true
  }

  fn after_key(&mut self) {
    if let Some(focus) = &self.focus
      && focus.is_entry()
    {
      focus.blink_caret();
    }
  }

  /// A compose sequence or an input method's composition at the text entry `target`: whether
  /// it took the key.
  fn compose(&mut self, target: &Node, input: &KeyInput) -> bool {
    let read_only = target.contents.borrow().read_only;

    if read_only || target.access.borrow().disabled {
      return false;
    }

    // A compose sequence shows as the entry's composition while it is typed, and its
    // character replaces it, as an input method's would.
    if let Some(sequence) = &input.composing {
      target.ime_preedit(sequence, None, Vec::new());

      return true;
    }

    if input.composed {
      match &input.text {
        Some(text) => {
          target.ime_commit(text);
          target.blink_caret();
        }
        None => target.ime_preedit("", None, Vec::new()),
      }

      return true;
    }

    false
  }

  /// A text entry's default for a key no handler prevented: its editing ([`EntryKeys`]);
  /// whether it took the key.
  fn entry_default(target: &Node, input: &KeyInput) -> bool {
    let event = &input.event;
    let multiline = target.spec.kind == (NodeKind::Editable { multiline: true });
    let (read_only, secure) = {
      let contents = target.contents.borrow();

      (contents.read_only, contents.secure)
    };

    if target.access.borrow().disabled {
      return false;
    }

    let keys = EntryKeys::new(multiline)
      .with_read_only(read_only)
      .with_secure(secure);
    let response = keys.key(event);

    match response.act {
      Some(EntryAct::Activate) | None => {}
      Some(EntryAct::Edit(Binding::Paste)) => {
        target.paste(crate::clipboard::Selection::Clipboard);
      }
      Some(EntryAct::Edit(Binding::Command(Command::Insert(key)))) => {
        // What is typed is the text the key produced, after compose; a dead key types nothing.
        if let Some(text) = input.text.clone().filter(|text| !text.is_empty()) {
          let _ = target.perform(Command::Insert(text));
        } else if key.chars().count() == 1 && key != "Dead" && input.text.is_some() {
          let _ = target.perform(Command::Insert(key));
        }

        return true;
      }
      Some(EntryAct::Edit(Binding::Command(command))) => {
        let _ = target.perform(command);
      }
    }

    let modifiers = event.modifiers;
    let plain = !modifiers.control && !modifiers.alt && !modifiers.meta;

    if !response.consumed
      && event.down
      && multiline
      && plain
      && matches!(event.key.as_str(), "PageUp" | "PageDown")
    {
      // The page keys move an editor's caret by the lines it shows, as a `<textarea>`'s do.
      let page = target.entry_page();
      let lines = if event.key == "PageUp" { -page } else { page };
      let _ = target.perform(Command::Move {
        movement: Movement::Lines(lines),
        extend: modifiers.shift,
      });

      return true;
    }

    if !response.consumed && event.down && input.text.is_some() && plain {
      // A composed character whose key is named otherwise (a keypad digit) still types.
      if let Some(text) = input.text.clone() {
        let _ = target.perform(Command::Insert(text));

        return true;
      }
    }

    response.consumed
  }

  fn focus_visible_on_key(&mut self) {
    if let Some(focus) = self.focus.clone()
      && !self.focus_visible
    {
      self.set_focus(Some(focus), true);
    }
  }

  /// What a key no verdict prevented does: a focused text entry edits, Tab moves focus, a
  /// focused box that acts answers as a command does ([`ActivateKeys`]), `Enter` follows a
  /// focused link, a scroll box scrolls, and `Escape` takes focus away.
  fn default_action(&mut self, target: &Node, input: &KeyInput) {
    let event = &input.event;
    let modifiers = event.modifiers;
    let focused = self
      .focus
      .as_ref()
      .is_some_and(|focus| Rc::ptr_eq(focus, target));

    if focused && target.is_entry() && Self::entry_default(target, input) {
      return;
    }

    if event.down && event.key == "Tab" && !modifiers.control && !modifiers.alt {
      self.move_focus(modifiers.shift);

      return;
    }

    if focused && target.spec.interaction == Interaction::Acting {
      let held = self
        .key_pressed
        .as_ref()
        .is_some_and(|held| Rc::ptr_eq(held, target));
      let response = ActivateKeys::new(held).key(event);

      if let Some(pressed) = response.pressed {
        self.press_key(target, pressed);
      }

      if response.act.is_some() {
        self.click(target);
      }

      if response.consumed {
        return;
      }
    }

    if let Some(link) = self.link
      && focused
      && event.down
      && event.key == "Enter"
      && !modifiers.control
      && !modifiers.alt
      && !modifiers.meta
    {
      Self::follow(target, link);

      return;
    }

    if event.down
      && !event.repeat
      && (event.key == "ContextMenu"
        || event.key == "F10" && modifiers.shift && !modifiers.control && !modifiers.alt)
    {
      // `Shift-F10` and the menu key ask for a context menu at the focus.
      let rect = target.window_rect();
      let at = (rect.x + rect.width / 2., rect.y + rect.height / 2.);

      self.context_menu_at(target, at, MenuSource::Keyboard);

      return;
    }

    if self.scroll_key(event) {
      return;
    }

    if event.down && event.key == "Escape" {
      self.set_focus(None, false);
    }
  }

  /// The modifiers held now, as the platform reports them changing (Wayland's `modifiers`,
  /// X11's `XkbStateNotify`): what a pointer press reads, a modifier released alone included.
  pub const fn set_modifiers(&mut self, modifiers: skene_foundation::seam::report::Modifiers) {
    self.modifiers = modifiers;
  }

  /// The window gained or lost the keyboard.
  pub fn keyboard_focus(&mut self, focused: bool) {
    trace::log(|| format!("[focus] window {focused}"));
    self.chrome.states.activated = focused || !self.chrome.csd;

    if self.window_focused != focused {
      self.window_focused = focused;
      crate::access::set_window_focus(focused);
    }

    if !focused {
      self.dismiss_popup();

      if let Some(focus) = self.focus.clone() {
        self.release_key(&focus);
      }
    }

    invalidate_all();
  }

  /// Answers a reader's request to scroll at `node` (from the accessibility layer): the
  /// scroll boxes holding it scrolled as little as shows it, or so its top leading corner
  /// shows at a point of the window, or the scroll box it is (else the nearest holding it)
  /// scrolled by a page along an axis, as `PageDown` scrolls it.
  pub fn access_scroll(&mut self, node: &Node, request: AccessRequest) {
    match request {
      AccessRequest::ScrollIntoView => {
        self.reveal(node);
      }
      AccessRequest::ScrollToPoint(x, y) => {
        for ancestor in node.ancestors() {
          if !matches!(
            ancestor.layout_kind(),
            Some(skene_foundation::seam::node::LayoutKind::Scroll { .. })
          ) {
            continue;
          }

          let at = node.window_rect();

          self.scroll_by(&ancestor, at.x - x, at.y - y);
        }
      }
      AccessRequest::ScrollPage { axis, forward } => {
        let sign = if forward { 1. } else { -1. };
        let scroller = std::iter::once(Rc::clone(node))
          .chain(node.ancestors())
          .find(|candidate| {
            matches!(
              candidate.layout_kind(),
              Some(skene_foundation::seam::node::LayoutKind::Scroll { .. })
            )
          });

        if let Some(scroller) = scroller {
          let frame = scroller.frame.get();
          let (dx, dy) = match axis {
            Axis::Horizontal => (sign * frame.width * PAGE, 0.),
            Axis::Vertical => (0., sign * frame.height * PAGE),
          };

          self.scroll_by(&scroller, dx, dy);
        }
      }
      _ => self.access_request(node, request),
    }

    crate::access::invalidate();
  }

  /// Reports a reader's request at `node` (from the accessibility layer) to the control or
  /// the box that acts it is.
  pub fn access_request(&mut self, node: &Node, request: AccessRequest) {
    tree::report::<Linux>(node.id.get(), Report::Access(request));
    self.settle_focus();
    crate::access::invalidate();
  }

  /// Moves focus to `node` from the accessibility layer.
  pub fn access_focus(&mut self, node: &Node) {
    self.set_focus(Some(Rc::clone(node)), true);
  }

  /// Moves focus to link run `link` of the text `node` from the accessibility layer.
  pub fn access_focus_link(&mut self, node: &Node, link: usize) {
    self.focus_link(node, link, true);
  }

  /// Follows link run `link` of the text `node` from the accessibility layer.
  pub fn access_follow(&mut self, node: &Node, link: usize) {
    Self::follow(node, link);
  }

  /// The focused link run of the focused text, by its place among the text's links.
  pub const fn focused_link(&self) -> Option<usize> {
    self.link
  }

  /// The focused text entry, where one has focus.
  pub fn focused_entry(&self) -> Option<Node> {
    self
      .focus
      .clone()
      .filter(|focus| focus.is_entry() && focus.has_entry())
  }

  /// What the input method is told of the focused entry, where an entry has focus.
  pub fn ime_state(&self) -> Option<entry::ImeState> {
    let node = self.focused_entry()?;

    node.ime_state(node.window_rect())
  }

  /// An input method shows `text` as its composition in the focused entry.
  pub fn ime_preedit(
    &mut self,
    text: &str,
    cursor: Option<std::ops::Range<usize>>,
    clauses: Vec<skene_foundation::text::editing::Clause>,
  ) {
    if let Some(node) = self.focused_entry() {
      node.ime_preedit(text, cursor, clauses);
    }
  }

  /// An input method commits `text` in the focused entry.
  pub fn ime_commit(&mut self, text: &str) {
    if let Some(node) = self.focused_entry() {
      node.ime_commit(text);
      node.blink_caret();
    }
  }

  /// An input method deletes `before` bytes before the focused entry's selection and `after`
  /// after it.
  pub fn ime_delete_surrounding(&mut self, before: usize, after: usize) {
    if let Some(node) = self.focused_entry() {
      node.ime_delete_surrounding(before, after);
    }
  }

  /// An input method deletes `length` characters starting `offset` characters from the
  /// focused entry's caret, in the text it was told ([`entry::ImeState`]).
  pub fn ime_delete_chars(&mut self, offset: i64, length: usize) {
    if let Some(node) = self.focused_entry() {
      node.ime_delete_chars(offset, length);
    }
  }

  /// Whether the focused entry is secure: what its trace lines show as marks.
  #[must_use]
  pub fn focused_entry_is_secure(&self) -> bool {
    self
      .focused_entry()
      .is_some_and(|node| node.contents.borrow().secure)
  }

  /// The part of the window's frame its last frame drew, in device pixels: its leading edge,
  /// its top, its width and its height.
  #[must_use]
  pub const fn last_drawn(&self) -> (u32, u32, u32, u32) {
    self.drawn
  }

  /// The last frame painted (on the GPU path, read back from the GPU).
  pub fn last_frame(&mut self) -> Option<&vello_cpu::Pixmap> {
    (self.frames > 0).then(|| self.renderer.last_frame())
  }

  /// The node with focus.
  pub const fn focus(&self) -> Option<&Node> {
    self.focus.as_ref()
  }
}

/// Shows `value` as `content` at `node`: its contents updated and what it changes invalidated.
pub fn write(node: &Node, content: Content, value: &PropValue) {
  node.contents.borrow_mut().write(content, value);

  let entry = node.is_entry();

  match content {
    Content::Edit if entry => {
      if let PropValue::Edit(edit) = value {
        if node.has_entry() {
          node.write_entry(edit);
        } else {
          invalidate_prepare();
        }
      }
    }
    Content::Text | Content::Runs | Content::Reserve | Content::Placeholder => {
      invalidate_prepare();
    }
    Content::Secure | Content::ReadOnly if entry => {
      node.follow_secure();
      invalidate_prepare();
      entry::ime_dirty(false);
    }
    Content::Placement if node.is_overlay() => invalidate_overlays(),
    Content::ScrollOffset => {
      if let PropValue::Offset(offset) = value {
        let wanted = (offset.x.max(0.), offset.y.max(0.));
        let frame = node.frame.get();
        let (width, height) = node.extent.get();
        // Clamped to the extent last laid out, unless a layout pending lays it out again, or
        // it was never laid out.
        let shown = if layout_pending() || (width, height) == (0., 0.) {
          wanted
        } else {
          (
            wanted.0.min((width - frame.width).max(0.)),
            wanted.1.min((height - frame.height).max(0.)),
          )
        };

        if node.scroll.replace(shown) != shown {
          trace::scroll(node, shown);
          SCROLLED.with(|scrolled| scrolled.set(true));
        }
      }
    }
    _ => {}
  }

  crate::access::invalidate();
  invalidate_paint(node);
}
