//! The window both backends share: its size, scale and chrome, the realized tree it shows,
//! what is dirty since the last frame, and the reader's input dispatched through the
//! foundation: hit testing, hover, press and focus states, Tab order in the scene's child
//! order, pointer events, clicks, keys and a reader's requests reported to the controls and
//! the boxes that act, each default run unless a verdict prevents it, wheel scrolling, and
//! the open overlay's surface.
//!
//! A backend turns its protocol's events into calls here and carries out the
//! [`Request`]s returned (moving or resizing the window, the cursor), and asks for a frame
//! when [`Shell::wants_frame`] says one is due.

use std::{
  cell::RefCell,
  rc::Rc,
  time::{Duration, Instant},
};

use skene_foundation::{
  State, StateSet,
  behavior::{ActivateKeys, EntryAct, EntryKeys},
  environment::{self, Fact},
  layout::Rect,
  seam::{
    content::{Content, PropValue},
    control::{AccessRequest, Prevent},
    node::{Interaction, NodeKind},
    report::{Answer, KeyEvent, PointerEvent, PointerPhase, Report, Size},
    tree,
  },
  text::{
    Laid as _,
    editing::{Binding, Command, Movement},
  },
};

use crate::{
  Linux,
  chrome::{Button, Chrome, Edge, Hit},
  entry,
  keys::KeyInput,
  layout,
  node::{Node, walk},
  overlay::{self, Open},
  paint::{Marks, Renderer, painted_bounds, union},
  palette::Palette,
  text::Change,
  trace,
};

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
}

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

thread_local! {
  static INPUTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
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

/// What `node` and its subtree paint now, in window pixels.
pub fn reach_rect(node: &Node) -> Rect {
  let at = node.window_rect();
  let reach = node.reach.get();

  Rect {
    x: at.x + reach.x,
    y: at.y + reach.y,
    ..reach
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

/// Whether `node` is `ancestor` or beneath it.
fn is_within(node: &Node, ancestor: &Node) -> bool {
  Rc::ptr_eq(node, ancestor)
    || node
      .ancestors()
      .iter()
      .any(|candidate| Rc::ptr_eq(candidate, ancestor))
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

/// The window.
pub struct Shell {
  root: Option<Node>,
  /// The geometry's width and height, in pixels (the title bar included).
  pub size: (u32, u32),
  /// Device pixels per pixel.
  pub scale: f64,
  /// The chrome.
  pub chrome: Chrome,
  title: String,
  renderer: Renderer,
  pointer: Option<(f32, f32)>,
  hovered: Vec<Node>,
  press: Option<Press>,
  focus: Option<Node>,
  link: Option<usize>,
  focus_visible: bool,
  reveal: bool,
  title_click: Option<(Instant, (f32, f32))>,
  cursor: Cursor,
  overlay: Option<Open>,
  pointer_in_overlay: bool,
  keyboard_modality: bool,
  key_pressed: Option<Node>,
  pending: Vec<Request>,
  modifiers: skene_foundation::seam::report::Modifiers,
  window_focused: bool,
  last_click: Option<(Instant, (f32, f32), u8, Node)>,
  owed: Option<(u32, u32, u32, u32)>,
  /// Frames painted.
  pub frames: u64,
  /// When the last frame was painted.
  pub painted_at: Option<Instant>,
  presented: bool,
  /// How many times the window was given a size.
  pub configures: u64,
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
      press: None,
      focus: None,
      link: None,
      focus_visible: false,
      reveal: false,
      title_click: None,
      cursor: Cursor::Default,
      overlay: None,
      pointer_in_overlay: false,
      keyboard_modality: false,
      key_pressed: None,
      pending: Vec::new(),
      modifiers: skene_foundation::seam::report::Modifiers::default(),
      window_focused: false,
      last_click: None,
      owed: None,
      frames: 0,
      painted_at: None,
      presented: false,
      configures: 0,
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
  pub fn root(&self) -> Option<&Node> {
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
    self.configures += 1;

    if (width, height) != self.size {
      self.size = (width.max(1), height.max(1));
    }

    let (content_width, content_height) = self.content_size();

    environment::report(Fact::Width(content_width));
    environment::report(Fact::Height(content_height));
    invalidate_layout();
    invalidate_all();
  }

  /// The window's scale changed.
  pub fn set_scale(&mut self, scale: f64) {
    if (scale - self.scale).abs() > f64::EPSILON {
      self.scale = scale;
      environment::report(Fact::Scale(scale));
      invalidate_all();
    }
  }

  /// Whether a frame is due: something changed since the last one, or something is in
  /// motion.
  pub fn wants_frame(&self) -> bool {
    wants_frame() || self.owed.is_some() || crate::motion::running()
  }

  /// Whether a frame is due for motion alone: what the frame clock paces.
  pub fn in_motion(&self) -> bool {
    crate::motion::running()
  }

  /// Applies a focus move the app or a behavior asked for, focus shown as the last input
  /// shows it: after a key, not after a press.
  pub fn settle_focus(&mut self) {
    let request = DIRTY.with(|dirty| dirty.borrow_mut().focus.take());

    if let Some(node) = request {
      let visible = self.keyboard_modality;

      self.set_focus(node, visible);
    }
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
  /// The handles in `target` stay valid while the window lives.
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

  /// Loses the GPU's device, as a driver reset would; whether the window painted on the GPU.
  pub fn lose_device(&mut self) -> bool {
    let lost = self.renderer.lose_device();

    if lost {
      invalidate_all();
    }

    lost
  }

  /// Lays out and paints what changed; the device rectangle painted.
  fn draw(&mut self) -> Option<(u32, u32, u32, u32)> {
    self.settle_focus();

    let root = self.root.clone()?;
    let started = Instant::now();
    let palette = Palette::now();

    crate::motion::advance(crate::motion::now());

    let mut dirty = DIRTY.with(|dirty| std::mem::take(&mut *dirty.borrow_mut()));

    if dirty.overlays {
      // An overlay's change waits for the turn that syncs it.
      invalidate_overlays();
    }

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
        if node.is_alive()
          && layout::prepare_subtree(&node, &palette, &mut repainted) == Change::Layout
        {
          dirty.layout = true;
        }
      }

      dirty.nodes.extend(repainted);
    }

    let prepared = started.elapsed();

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
    }

    self.mark_overlay(&dirty);

    if std::mem::take(&mut self.reveal) && self.reveal_focus() {
      dirty.all = true;
    }

    layout::reach(&root);

    if dirty.layout {
      // Before the paint, so a box that draws at its size draws in the frame that sized it.
      self.report_sizes(&root);
    }

    let laid_out = started.elapsed();
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
        .map(|node| union(painted_bounds(node, node.window_rect()), reach_rect(node)))
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

    // A frame that fell back to the CPU as the device was lost painted the whole surface.
    let region = if gpu && !cost.presented {
      surface
    } else {
      region
    };

    self.frames += 1;
    self.painted_at = Some(started);
    skene_foundation::seam::activity::frame();

    if dirty.layout {
      self.refresh_hover();
    }

    if dirty.layout && trace::nodes_on() {
      let mut scene = 0;

      walk(&root, |_| scene += 1);
      trace::nodes(scene, crate::access::count(self));
    }

    trace::frame(trace::FrameCost {
      prepare: prepared,
      layout: laid_out - prepared,
      record: Duration::from_micros(cost.record),
      raster: Duration::from_micros(cost.raster),
      glyphs: Duration::from_micros(cost.glyphs),
      masks: cost.masks,
      images: cost.images,
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

  /// Reports each node that asked for its size and was laid out at another.
  fn report_sizes(&self, root: &Node) {
    let mut changed = Vec::new();
    let scale = self.scale;

    walk(root, |node| {
      if node.interest.get().size {
        let frame = node.frame.get();
        let size = (frame.width, frame.height, scale);

        if node.reported.get() != Some(size) {
          node.reported.set(Some(size));
          changed.push((node.id.get(), size));
        }
      }
    });

    for (id, (width, height, scale)) in changed {
      tree::report::<Linux>(
        id,
        Report::Size(Size {
          width,
          height,
          scale,
        }),
      );
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
      let inside =
        x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height;
      let clips = node.look.borrow().clip
        || matches!(
          node.layout_kind(),
          Some(skene_foundation::seam::node::LayoutKind::Scroll { .. })
        );

      if !inside && clips {
        return false;
      }

      let (sx, sy) = node.scroll.get();

      for child in node.children().iter().rev() {
        if child.is_overlay() {
          continue;
        }

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

    if next.len() == self.hovered.len()
      && next
        .iter()
        .zip(&self.hovered)
        .all(|(a, b)| Rc::ptr_eq(a, b))
    {
      return;
    }

    let previous = std::mem::replace(&mut self.hovered, next.clone());

    Self::set_chain(&previous, &next, State::Hovered);
  }

  /// The control or acting box a press on `chain` belongs to: the deepest.
  fn owner(chain: &[Node]) -> Option<Node> {
    chain
      .iter()
      .find(|node| node.spec.is_interactive())
      .cloned()
  }

  /// Whether `node` takes focus now: a shown control whose facts say it does (a radio while
  /// it is its group's stop), or a shown box that acts.
  fn focusable(node: &Node) -> bool {
    node.is_shown()
      && match node.spec.interaction {
        Interaction::Control { .. } => node.access.borrow().focusable,
        Interaction::Acting => true,
        Interaction::None | Interaction::Answering => false,
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
      && !is_within(next, &open.node)
      && !is_within(&open.node, next)
    {
      tree::report::<Linux>(open.node.id.get(), Report::Dismiss);
    }

    Self::set_chain(&previous_chain, &next_chain, State::FocusWithin);
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
    let (next, link) = order[next].clone();

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
    let mut moved = false;

    for ancestor in focus.ancestors() {
      if !matches!(
        ancestor.layout_kind(),
        Some(skene_foundation::seam::node::LayoutKind::Scroll { .. })
      ) {
        continue;
      }

      let target = focus.window_rect();
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

    let page = |node: &Node| node.frame.get().height * 0.875;
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
    let chain = match &self.focus {
      Some(focus) => std::iter::once(Rc::clone(focus))
        .chain(focus.ancestors())
        .collect(),
      None if !self.hovered.is_empty() => self.hovered.clone(),
      None => {
        let mut first = Vec::new();

        if let Some(root) = &self.root {
          walk(root, |node| {
            if first.is_empty()
              && matches!(
                node.layout_kind(),
                Some(skene_foundation::seam::node::LayoutKind::Scroll { .. })
              )
            {
              first.push(Rc::clone(node));
            }
          });
        }

        first
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
    self.report_sizes(&node);
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

    self.report_sizes(&node);
    crate::access::invalidate();
  }

  /// Closes the open overlay's surface; focus inside it returns to the nearest node before it
  /// that takes focus (its anchor).
  fn close_overlay(&mut self) {
    let Some(open) = self.overlay.take() else {
      return;
    };

    trace::log(|| "[popup] close".to_owned());
    self.pending.push(Request::Popup(None));
    self.pointer_in_overlay = false;

    if let Some(focus) = self.focus.clone()
      && is_within(&focus, &open.node)
    {
      let anchor = open.node.ancestors().into_iter().find(Self::focusable);
      let visible = self.keyboard_modality;

      self.set_focus(anchor, visible);
    }

    if self
      .hovered
      .first()
      .is_some_and(|hovered| is_within(hovered, &open.node))
    {
      let previous = std::mem::take(&mut self.hovered);

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
    } else if (dirty.all || dirty.nodes.iter().any(|node| is_within(node, &open.node)))
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

  /// The open overlay's frame, where it changed.
  pub fn popup_frame(&mut self) -> Option<Frame<'_>> {
    let scale = self.scale;
    let marks = Marks {
      focus: self.focus.clone().filter(|_| self.focus_visible),
      focused: self.focus.clone(),
      link: None,
    };

    self.overlay.as_mut()?.frame(scale, &marks)
  }

  /// Whether the open overlay wants a frame.
  pub fn popup_wants_frame(&self) -> bool {
    self.overlay.as_ref().is_some_and(Open::wants_frame)
  }

  /// The pointer moved to `x`, `y` over the open overlay (its surface's coordinates).
  pub fn popup_motion(&mut self, x: f32, y: f32) {
    let Some(open) = &self.overlay else {
      return;
    };
    let frame = open.node.frame.get();
    let at = (x + frame.x, y + frame.y);

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
          && now.duration_since(*then) < Duration::from_millis(400)
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
          });
          invalidate_all();
        }
        (_, Hit::Content(..)) => {
          let chain = self.hovered.clone();

          self.content_press(button, chain);
        }
        _ => {}
      }

      requests.extend(self.take_requests());

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

  /// A press of `button` on `chain` (the deepest node under the pointer first): the pressed
  /// states, focus moved to what the press lands on (a caption beside a control focusing the
  /// control), a text entry's caret placed, and the press reported; a control that drags
  /// keeps the pointer where its handler answered it.
  fn content_press(&mut self, button: u8, chain: Vec<Node>) {
    self.keyboard_modality = false;

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

        owner.press_entry(owner.window_rect(), (x, y), clicks, extend);
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
    });

    let prevented = self.report_pointer(PointerPhase::Down, button);
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

  /// The wheel turned over the window.
  pub fn wheel(&mut self, wheel: Wheel) {
    count_input();

    let (dx, dy) = match wheel {
      Wheel::Pixels(dx, dy) => (dx, dy),
      Wheel::Lines(dx, dy) => (dx * 48., dy * 48.),
    };

    for node in self.hovered.clone() {
      if (node.is_entry() && node.scroll_entry(dy)) || self.scroll_by(&node, dx, dy) {
        return;
      }
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

    self.modifiers = event.modifiers;

    if event.down {
      self.keyboard_modality = true;
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

    if self.scroll_key(event) {
      return;
    }

    if event.down && event.key == "Escape" {
      self.set_focus(None, false);
    }
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

  /// An input method deletes `range` (bytes of the focused entry's text without its
  /// composition).
  pub fn ime_delete(&mut self, range: std::ops::Range<usize>) {
    if let Some(node) = self.focused_entry() {
      node.ime_delete(range);
    }
  }

  /// Whether `state` holds at `node`, as the realization tracks it.
  pub fn holds(node: &Node, state: State) -> bool {
    node.states.get().contains(state)
  }

  /// The states the realization tracks at every node: what a newly watched node is told.
  pub fn states_of(node: &Node) -> StateSet {
    node.states.get()
  }

  /// The last frame painted (on the GPU path, read back from the GPU).
  pub fn last_frame(&mut self) -> Option<&vello_cpu::Pixmap> {
    (self.frames > 0).then(|| self.renderer.last_frame())
  }

  /// Whether the window has the keyboard.
  pub const fn is_window_focused(&self) -> bool {
    self.window_focused
  }

  /// The node with focus.
  pub fn focus(&self) -> Option<&Node> {
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
      invalidate_prepare();
      entry::ime_dirty(false);
    }
    Content::Placement if node.is_overlay() => invalidate_overlays(),
    _ => {}
  }

  crate::access::invalidate();
  invalidate_paint(node);
}
