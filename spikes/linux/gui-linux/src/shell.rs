//! The window both backends share: its size, scale and chrome, the realized tree it shows,
//! what is dirty since the last frame, and the reader's input dispatched through the
//! foundation: hit testing, hover, press and focus states, Tab order in the scene's child
//! order, clicks and keys to the controls and the boxes that act, wheel scrolling.
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
  behavior::{Activate, Behavior as _, Collection, EntryAct, Orientation, PopupAct, TextEntry},
  environment::{self, Fact},
  layout::Rect,
  seam::{
    access::{Action, Role},
    content::{Content, PropValue, Verdict},
    node::{ControlKind, NodeKind},
    report::{Answer, KeyEvent, PointerEvent, PointerPhase, Report, Size},
    tree,
  },
  text::{
    Laid as _,
    editing::{Binding, Command},
  },
};

use crate::{
  Linux,
  chrome::{Button, Chrome, Edge, Hit},
  controls::{self, Act, KeyResult},
  entry,
  keys::KeyInput,
  layout,
  look::RunProps,
  node::{Node, walk},
  paint::{Marks, Renderer, painted_bounds, union},
  palette::Palette,
  popup::OpenList,
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
  Popup(Option<crate::popup::Placement>),
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
  layout: bool,
  all: bool,
  nodes: Vec<Node>,
  focus: Option<Option<Node>>,
}

thread_local! {
  static INPUTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
  static TYPE_AHEAD: RefCell<skene_foundation::behavior::TypeAhead> =
    const { RefCell::new(skene_foundation::behavior::TypeAhead::new()) };
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

/// The control kind of `node`, where it is a control.
pub fn controls_kind(node: &Node) -> Option<ControlKind> {
  match node.spec.kind {
    NodeKind::Control(kind) => Some(kind),
    _ => None,
  }
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

/// The tree needs laying out again.
pub fn invalidate_layout() {
  DIRTY.with(|dirty| dirty.borrow_mut().layout = true);
}

/// `node` needs painting again where it is.
pub fn invalidate_paint(node: &Node) {
  DIRTY.with(|dirty| dirty.borrow_mut().nodes.push(Rc::clone(node)));
}

/// The whole window needs painting again.
pub fn invalidate_all() {
  DIRTY.with(|dirty| dirty.borrow_mut().all = true);
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

    dirty.prepare || dirty.layout || dirty.all || !dirty.nodes.is_empty()
  })
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
}

/// One painted frame: its pixels are the renderer's, `damage` the part that changed (device
/// pixels: x, y, width, height).
pub struct Frame<'a> {
  /// The frame's pixels.
  pub pixmap: &'a vello_cpu::Pixmap,
  /// The rectangle that changed.
  pub damage: (u32, u32, u32, u32),
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
  popup: Option<OpenList>,
  pending: Vec<Request>,
  modifiers: skene_foundation::seam::report::Modifiers,
  window_focused: bool,
  last_click: Option<(Instant, (f32, f32), u8, Node)>,
  owed: Option<(u32, u32, u32, u32)>,
  /// Frames painted.
  pub frames: u64,
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
      popup: None,
      pending: Vec::new(),
      modifiers: skene_foundation::seam::report::Modifiers::default(),
      window_focused: false,
      last_click: None,
      owed: None,
      frames: 0,
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

  /// Whether a frame is due: something changed since the last one.
  pub fn wants_frame(&self) -> bool {
    wants_frame() || self.owed.is_some()
  }

  /// Applies a focus move the app asked for.
  pub fn settle_focus(&mut self) {
    let request = DIRTY.with(|dirty| dirty.borrow_mut().focus.take());

    if let Some(node) = request {
      self.set_focus(node, true);
    }
  }

  /// Lays out and paints what changed: the frame and the part of it that changed since the
  /// last frame presented, `None` where nothing did.
  pub fn frame(&mut self) -> Option<Frame<'_>> {
    let drawn = self.draw();
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
    if let Some(drawn) = self.draw() {
      self.owed = Some(self.owed.map_or(drawn, |owed| union_damage(owed, drawn)));
    }
  }

  /// Lays out and paints what changed; the device rectangle painted.
  fn draw(&mut self) -> Option<(u32, u32, u32, u32)> {
    self.settle_focus();

    let root = self.root.clone()?;
    let started = Instant::now();
    let palette = Palette::now();
    let mut dirty = DIRTY.with(|dirty| std::mem::take(&mut *dirty.borrow_mut()));

    if dirty.prepare {
      if self.chrome.set_title(&self.title, &palette) != Change::None {
        dirty.all = true;
      }

      match layout::prepare(&root, &palette) {
        Change::Layout => dirty.layout = true,
        Change::Paint => dirty.all = true,
        Change::None => {}
      }
    }

    let prepared = started.elapsed();

    if dirty.layout {
      let (width, height) = self.content_size();

      layout::layout(&root, (0., self.chrome.header()), width, height);
      crate::access::invalidate();
      walk(&root, |node| {
        if controls_kind(node).is_some_and(controls::is_entry) {
          node.fit_entry();
        }
      });
      dirty.all = true;
    }

    if std::mem::take(&mut self.reveal) && self.reveal_focus() {
      dirty.all = true;
    }

    layout::reach(&root);

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
        .map(|node| painted_bounds(node, node.window_rect()))
        .fold(Rect::default(), union)
    };

    if region.width <= 0. || region.height <= 0. {
      return None;
    }

    let device = self.device_size();
    let marks = Marks {
      focus: self.focus.clone().filter(|_| self.focus_visible),
      link: self
        .focus
        .as_ref()
        .zip(self.link)
        .and_then(|(focus, link)| focus.links().get(link).map(|(range, _)| range.clone())),
    };
    let chrome = self.chrome.clone();
    let (width, height) = self.size;
    let cost = self.renderer.paint(
      Some(&root),
      (device.0 as u16, device.1 as u16),
      self.scale,
      region,
      (ox, oy),
      &marks,
      &mut |canvas| chrome.paint(canvas, width, height),
    );

    self.frames += 1;
    skene_foundation::seam::activity::frame();

    if dirty.layout {
      self.report_sizes(&root);
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

  /// The deepest shown node at `x`, `y` (window pixels), with its ancestors, deepest first.
  fn hit(&self, x: f32, y: f32) -> Vec<Node> {
    fn go(node: &Node, origin: (f32, f32), x: f32, y: f32, out: &mut Vec<Node>) -> bool {
      if node.is_hidden() {
        return false;
      }

      let frame = node.frame.get();
      let rect = Rect {
        x: origin.0 + frame.x,
        y: origin.1 + frame.y,
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

    if let Some(root) = &self.root {
      go(root, (0., 0.), x, y, &mut out);
    }

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

    if matches!(node.spec.kind, NodeKind::Control(_)) {
      invalidate_paint(node);
    }

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

  /// The radio group `node` (a radio) belongs to, and its radios in the scene's order.
  fn radio_group(node: &Node) -> Option<(Node, Vec<Node>)> {
    let group = node
      .ancestors()
      .into_iter()
      .find(|ancestor| ancestor.spec.role == Role::RadioGroup)?;
    let mut radios = Vec::new();

    walk(&group, |member| {
      if member.spec.kind == NodeKind::Control(ControlKind::Radio) && member.is_shown() {
        radios.push(Rc::clone(member));
      }
    });

    Some((group, radios))
  }

  fn focusable(node: &Node) -> bool {
    let enabled = node.is_shown()
      && match node.spec.kind {
        NodeKind::Control(kind) => controls::focusable(kind) && !node.contents.borrow().disabled,
        _ => node.spec.acts,
      };

    if !enabled || node.spec.kind != NodeKind::Control(ControlKind::Radio) {
      return enabled;
    }

    // A radio group is one tab stop: its chosen radio, else its first enabled one.
    let Some((_, radios)) = Self::radio_group(node) else {
      return true;
    };
    let stop = radios
      .iter()
      .find(|radio| radio.contents.borrow().on && !radio.contents.borrow().disabled)
      .or_else(|| {
        radios
          .iter()
          .find(|radio| !radio.contents.borrow().disabled)
      });

    stop.is_some_and(|stop| Rc::ptr_eq(stop, node))
  }

  fn set_focus(&mut self, node: Option<Node>, visible: bool) {
    self.set_focus_to(node, visible, false);
  }

  /// Moves focus to `node` (taking it away where `None`), focus shown where `visible`; a
  /// node that takes no focus by Tab is focused anyway where `forced` (a radio the arrows
  /// move to).
  fn set_focus_to(&mut self, node: Option<Node>, visible: bool, forced: bool) {
    let node = node.filter(|node| {
      forced && node.is_shown() && !node.contents.borrow().disabled || Self::focusable(node)
    });
    let visible = visible
      || node.as_ref().is_some_and(|node| {
        matches!(
          node.spec.kind,
          NodeKind::Control(ControlKind::TextField | ControlKind::TextEditor)
        )
      });

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
      Self::release_key(previous);
      invalidate_paint(previous);
      previous.blur_entry();

      if previous.interest.get().focus {
        tree::report::<Linux>(previous.id.get(), Report::Focus(false));
      }
    }

    if self.popup.as_ref().is_some_and(|popup| {
      node
        .as_ref()
        .is_none_or(|node| !Rc::ptr_eq(&popup.owner, node))
    }) {
      self.close_popup();
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
      Self::set_state(node, State::FocusVisible, visible);
      invalidate_paint(node);

      if node.has_entry() && controls_kind(node).is_some_and(controls::is_entry) {
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
    if matches!(next.spec.kind, NodeKind::Control(ControlKind::TextField)) && next.has_entry() {
      let _ = next.perform(Command::SelectAll);
    }
  }

  /// Shows `node` pressed from the keyboard, or not.
  fn press_key(node: &Node, pressed: bool) {
    if node.key_pressed.replace(pressed) != pressed {
      Self::set_state(node, State::Pressed, pressed);
      invalidate_paint(node);
    }
  }

  /// `node` no longer shows pressed from the keyboard.
  fn release_key(node: &Node) {
    Self::press_key(node, false);
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

  /// Reports what the reader did to a control and applies the verdict.
  fn act(&mut self, node: &Node, act: Act) {
    let id = node.id.get();

    match act {
      Act::Activate => {
        trace::activate(node);
        tree::report::<Linux>(id, Report::Activate);
      }
      Act::Change(content, proposed) => {
        let answer = tree::report::<Linux>(
          id,
          Report::Change {
            content,
            proposed: proposed.clone(),
            applied: false,
          },
        );

        trace::change(node, content, &proposed, &answer);

        let shown = match answer {
          Answer::Verdict(Verdict::Accept) => Some(proposed),
          Answer::Verdict(Verdict::Replace(truth)) => Some(truth),
          _ => None,
        };

        if let Some(value) = shown {
          write(node, content, &value);
        }

        crate::access::invalidate();
      }
      Act::Popup(popup) => match popup {
        PopupAct::Open => self.open_popup(node),
        PopupAct::Close => self.close_popup(),
        PopupAct::Highlight(index) => {
          if let Some(list) = &mut self.popup {
            list.highlight(Some(index));
            crate::access::invalidate();
          }
        }
        PopupAct::Choose(index) => {
          if let Some(act) = node.choice_change(index) {
            self.act(node, act);
          }
        }
        PopupAct::ChooseAndClose(index) => {
          self.close_popup();

          if let Some(act) = node.choice_change(index) {
            self.act(node, act);
          }
        }
      },
    }
  }

  /// Opens `node`'s list below it.
  fn open_popup(&mut self, node: &Node) {
    if self.popup.is_some() {
      self.close_popup();
    }

    let mut inherited = RunProps::default();

    for ancestor in node.ancestors().iter().rev() {
      inherited = ancestor.run.borrow().over(&inherited);
    }

    let list = OpenList::new(
      Rc::clone(node),
      node.window_rect(),
      &node.run.borrow().over(&inherited),
      &Palette::now(),
    );

    trace::log(|| {
      format!(
        "[popup] open {:?} {}x{}",
        list.placement.anchor, list.placement.size.0, list.placement.size.1
      )
    });
    self.pending.push(Request::Popup(Some(list.placement)));
    self.popup = Some(list);
    invalidate_paint(node);
    crate::access::invalidate();
  }

  /// Closes the open list, choosing nothing.
  pub fn close_popup(&mut self) {
    if let Some(list) = self.popup.take() {
      trace::log(|| "[popup] close".to_owned());
      self.pending.push(Request::Popup(None));
      invalidate_paint(&list.owner);
      crate::access::invalidate();
    }
  }

  /// The open list, where one is open.
  pub const fn popup(&self) -> Option<&OpenList> {
    self.popup.as_ref()
  }

  /// The open list, where one is open, to change.
  pub const fn popup_mut(&mut self) -> Option<&mut OpenList> {
    self.popup.as_mut()
  }

  /// The open list's state for `node`: its highlight where `node` owns the open list.
  fn open_state(&self, node: &Node) -> Option<Option<usize>> {
    self
      .popup
      .as_ref()
      .filter(|list| Rc::ptr_eq(&list.owner, node))
      .map(|list| list.highlighted)
  }

  /// The open list's frame, where it changed.
  pub fn popup_frame(&mut self) -> Option<Frame<'_>> {
    let scale = self.scale;

    self.popup.as_mut()?.frame(scale)
  }

  /// Whether the open list wants a frame.
  pub fn popup_wants_frame(&self) -> bool {
    self.popup.as_ref().is_some_and(OpenList::wants_frame)
  }

  /// The pointer moved to `x`, `y` over the open list (its own coordinates).
  pub fn popup_motion(&mut self, x: f32, y: f32) {
    if let Some(list) = &mut self.popup
      && let Some(row) = list.row_at(x, y)
    {
      list.highlight(Some(row));
    }
  }

  /// A button was pressed or released at `x`, `y` in the open list's coordinates: a release
  /// over a row chooses it, a press outside the list closes it.
  pub fn popup_button(&mut self, down: bool, x: f32, y: f32) {
    count_input();

    let Some(list) = &self.popup else {
      return;
    };
    let row = list.row_at(x, y);

    match (down, row) {
      (true, None) => self.close_popup(),
      (false, Some(row)) => {
        let owner = Rc::clone(&list.owner);

        self.act(&owner, Act::Popup(PopupAct::ChooseAndClose(row)));
      }
      _ => {}
    }
  }

  /// The wheel turned over the open list.
  pub fn popup_wheel(&mut self, wheel: Wheel) {
    count_input();

    if let Some(list) = &mut self.popup {
      let dy = match wheel {
        Wheel::Pixels(_, dy) => dy,
        Wheel::Lines(_, dy) => dy * 48.,
      };

      list.scroll_by(dy);
    }
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
      && let Some(owner) = &press.owner
      && let NodeKind::Control(kind) = owner.spec.kind
    {
      if controls::is_entry(kind) {
        if press.button_pressed == 0 {
          owner.drag_entry(owner.window_rect(), geometry);
        }
      } else if let Some(act) =
        owner.drag_control(kind, owner.window_rect(), geometry.0, geometry.1)
      {
        let owner = Rc::clone(owner);

        self.act(&owner, act);
      }
    }

    self.report_pointer(PointerPhase::Move, 0);

    requests
  }

  fn content_cursor(&self) -> Cursor {
    for node in &self.hovered {
      match node.spec.kind {
        NodeKind::Control(kind) if controls::is_entry(kind) => return Cursor::Text,
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

  fn report_pointer(&self, phase: PointerPhase, button: u8) {
    let Some(target) = self.hovered.first() else {
      return;
    };

    if !self.hovered.iter().any(|node| node.interest.get().pointer) {
      return;
    }

    let (x, y) = self.pointer.unwrap_or_default();
    let rect = target.window_rect();

    tree::report::<Linux>(
      target.id.get(),
      Report::Pointer(PointerEvent {
        phase,
        x: x - rect.x,
        y: y - rect.y,
        button,
      }),
    );
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

    if down && self.popup.is_some() {
      // A press anywhere but on the open list closes it, and goes no further.
      self.close_popup();
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
          });
          invalidate_all();
        }
        (_, Hit::Content(..)) => {
          let chain = self.hovered.clone();
          // A press on a caption or a link run is the text's, inside an acting box too.
          let link = chain
            .first()
            .zip(self.pointer)
            .and_then(|(target, (x, y))| Self::link_index_at(target, x, y));
          let claimed = link.is_some()
            || chain
              .first()
              .is_some_and(|target| self.captioned_control(target).is_some());
          let owner = if claimed { None } else { Self::owner(&chain) };
          let entry_kind = owner
            .as_ref()
            .and_then(controls_kind)
            .filter(|kind| controls::is_entry(*kind));

          if button == 0 {
            for node in &chain {
              Self::set_state(node, State::Pressed, true);
            }

            // A press focuses the control it lands on, a radio the arrows would not reach by
            // Tab included, as a browser focuses a clicked radio.
            let forced = owner
              .as_ref()
              .and_then(controls_kind)
              .is_some_and(controls::focusable);

            match (&link, chain.first()) {
              // A press on a link run focuses it, as a press on an anchor does.
              (Some(link), Some(target)) => self.focus_link(target, *link, false),
              _ => self.set_focus_to(owner.clone(), false, forced),
            }

            if let Some(owner) = &owner
              && let Some((x, y)) = self.pointer
            {
              if entry_kind.is_some() {
                let clicks = self.count_click(owner, (x, y));
                let extend = self.modifiers.shift;

                owner.press_entry(owner.window_rect(), (x, y), clicks, extend);
              } else if let NodeKind::Control(kind) = owner.spec.kind
                && let Some(act) = owner.press_control(kind, owner.window_rect(), x, y)
              {
                let owner = Rc::clone(owner);

                self.act(&owner, act);
              }
            }
          } else if button == 1
            && entry_kind.is_some()
            && let Some(owner) = &owner
            && let Some((x, y)) = self.pointer
          {
            // A middle press pastes the primary selection where it lands.
            self.set_focus(Some(Rc::clone(owner)), false);
            owner.press_entry(owner.window_rect(), (x, y), 1, false);
            owner.paste(crate::clipboard::Selection::Primary);
          }

          self.press = Some(Press {
            target: chain.first().cloned(),
            owner,
            title: None,
            button: None,
            button_pressed: button,
          });
          self.report_pointer(PointerPhase::Down, button);
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

    if let Some(target) = &press.target {
      for node in std::iter::once(Rc::clone(target)).chain(target.ancestors()) {
        Self::set_state(&node, State::Pressed, false);
      }
    }

    self.report_pointer(PointerPhase::Up, button);

    let over = Self::owner(&self.hovered);

    if press.button_pressed == 0
      && let (Some(owner), Some(over)) = (&press.owner, &over)
      && Rc::ptr_eq(owner, over)
      && let Some((x, y)) = self.pointer
    {
      let owner = Rc::clone(owner);

      match owner.spec.kind {
        NodeKind::Control(kind) if controls::is_entry(kind) => {}
        NodeKind::Control(kind) => {
          let open = self.open_state(&owner);

          if let Some(act) = owner.click_control(kind, owner.window_rect(), x, y, open) {
            self.act(&owner, act);
          }
        }
        _ if owner.spec.acts => self.act(&owner, Act::Activate),
        _ => {}
      }
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
    {
      if let Some(control) = self.captioned_control(target) {
        // A click on a caption that names an on/off control acts as one on the control.
        self.set_focus(Some(Rc::clone(&control)), false);

        let open = self.open_state(&control);

        if let NodeKind::Control(kind) = control.spec.kind
          && let Some(act) = control.click_control(kind, control.window_rect(), x, y, open)
        {
          self.act(&control, act);
        }
      } else if let Some(link) = Self::link_index_at(target, x, y) {
        Self::follow(target, link);
      }
    }

    requests.extend(self.take_requests());

    requests
  }

  /// The on/off control `caption` names, where it names one.
  fn captioned_control(&self, caption: &Node) -> Option<Node> {
    let id = caption.id.get();
    let mut found = None;

    if let Some(root) = &self.root {
      walk(root, |node| {
        if found.is_none()
          && controls_kind(node).is_some_and(|kind| controls::toggle_kind(kind).is_some())
          && node.access.borrow().labelled_by.contains(&id)
        {
          found = Some(Rc::clone(node));
        }
      });
    }

    found
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
      if self.scroll_by(&node, dx, dy) {
        return;
      }
    }
  }

  /// A key was pressed or released.
  pub fn key(&mut self, input: &KeyInput) {
    count_input();

    let event = &input.event;

    // A key reaches only the window with the keyboard: where no window manager says so (a
    // bare X server), the first key does.
    if !self.window_focused {
      self.keyboard_focus(true);
    }

    self.modifiers = event.modifiers;

    if event.down && event.key != "Tab" {
      self.focus_visible_on_key();
    }

    let target = self.focus.clone().or_else(|| self.root.clone());
    let Some(target) = target else {
      return;
    };

    trace::key(event, &target);

    if let NodeKind::Control(kind) = target.spec.kind {
      let consumed = if controls::is_entry(kind) {
        self.entry_key(&target, kind, input)
      } else {
        self.control_key(&target, kind, input)
      };

      if consumed {
        self.after_key();

        return;
      }
    }

    let answer = tree::report::<Linux>(target.id.get(), Report::Key(event.clone()));

    if answer == (Answer::Event { handled: true }) {
      self.after_key();

      return;
    }

    if self.shortcut(event) {
      self.after_key();

      return;
    }

    self.default_action(&target, event);
    self.after_key();
  }

  /// Activates the shown, enabled node whose shortcut `event` presses (Control and the key,
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
      let wanted = node.access.borrow().shortcut;

      if found.is_none()
        && node.is_shown()
        && !node.contents.borrow().disabled
        && wanted.is_some_and(|wanted| wanted.key == key && wanted.shift == modifiers.shift)
      {
        found = Some(Rc::clone(node));
      }
    });

    let Some(node) = found else {
      return false;
    };

    self.act(&node, Act::Activate);

    true
  }

  fn after_key(&mut self) {
    if let Some(focus) = &self.focus
      && controls_kind(focus).is_some_and(controls::is_entry)
    {
      focus.blink_caret();
    }
  }

  /// A key at a control drawn here (not an entry): whether it consumed it.
  fn control_key(&mut self, target: &Node, kind: ControlKind, input: &KeyInput) -> bool {
    let event = &input.event;
    let open = self.open_state(target);

    if kind == ControlKind::Picker
      && event.down
      && !event.modifiers.control
      && !event.modifiers.alt
      && !event.modifiers.meta
      && let Some(text) = input.text.as_deref()
      && (text != " "
        || self
          .popup
          .as_ref()
          .is_some_and(|list| list.type_ahead.is_active(Instant::now())))
    {
      self.type_ahead(target, text);

      return true;
    }

    let KeyResult {
      consumed,
      pressed,
      act,
    } = target.key_control(kind, event, open);

    if let Some(pressed) = pressed {
      Self::press_key(target, pressed);
    }

    if let Some(act) = act {
      self.act(target, act);
    }

    if consumed {
      return true;
    }

    if kind == ControlKind::Radio
      && let Some((_, radios)) = Self::radio_group(target)
      && let Some(current) = radios.iter().position(|radio| Rc::ptr_eq(radio, target))
    {
      let enabled = |index: usize| !radios[index].contents.borrow().disabled;
      let group = Collection::new(radios.len(), Some(current), Orientation::Both)
        .with_wrap(true)
        .with_enabled(&enabled);
      let response = group.key(event);

      if let Some(next) = response.act {
        let next = Rc::clone(&radios[next]);

        self.set_focus_to(Some(Rc::clone(&next)), true, true);

        if let Some(act) = next.click_control(ControlKind::Radio, next.window_rect(), 0., 0., None)
        {
          self.act(&next, act);
        }
      }

      return response.consumed;
    }

    false
  }

  /// Finds the picker's option starting with what the reader typed.
  fn type_ahead(&mut self, target: &Node, text: &str) {
    let now = Instant::now();
    let options = target
      .contents
      .borrow()
      .choice
      .as_ref()
      .map(|choice| Rc::clone(&choice.options));
    let Some(options) = options else {
      return;
    };
    let names: Vec<&str> = options.iter().map(|option| &**option).collect();

    match &mut self.popup {
      Some(list) if Rc::ptr_eq(&list.owner, target) => {
        let current = list.highlighted;

        if let Some(found) = list.type_ahead.type_text(text, now, &names, current) {
          list.highlight(Some(found));
          crate::access::invalidate();
        }
      }
      _ => {
        let current = target
          .contents
          .borrow()
          .choice
          .as_ref()
          .and_then(|choice| choice.chosen)
          .map(|index| index as usize);
        let found =
          TYPE_AHEAD.with(|search| search.borrow_mut().type_text(text, now, &names, current));

        if let Some(found) = found
          && let Some(act) = target.choice_change(found)
        {
          self.act(target, act);
        }
      }
    }
  }

  /// A key at a text entry: whether it consumed it.
  fn entry_key(&mut self, target: &Node, kind: ControlKind, input: &KeyInput) -> bool {
    let event = &input.event;
    let (read_only, secure, disabled) = {
      let contents = target.contents.borrow();

      (contents.read_only, contents.secure, contents.disabled)
    };

    if disabled {
      return false;
    }

    // A compose sequence shows as the entry's composition while it is typed, and its
    // character replaces it, as an input method's would.
    if !read_only && let Some(sequence) = &input.composing {
      target.ime_preedit(sequence, None, Vec::new());

      return true;
    }

    if !read_only && input.composed {
      match &input.text {
        Some(text) => {
          target.ime_commit(text);
          target.blink_caret();
        }
        None => target.ime_preedit("", None, Vec::new()),
      }

      return true;
    }

    let behavior = TextEntry::new(kind == ControlKind::TextEditor)
      .with_read_only(read_only)
      .with_secure(secure);
    let response = skene_foundation::behavior::Behavior::key(&behavior, event);

    match response.act {
      Some(EntryAct::Activate) => {
        tree::report::<Linux>(target.id.get(), Report::Activate);
      }
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
      None => {}
    }

    if !response.consumed
      && event.down
      && input.text.is_some()
      && !event.modifiers.control
      && !event.modifiers.alt
      && !event.modifiers.meta
    {
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

  /// What a key no handler consumed does: Tab moves focus, a focused acting box answers as a
  /// command does ([`Activate`]), `Enter` follows a focused link, a scroll box scrolls, and
  /// `Escape` takes focus away.
  fn default_action(&mut self, target: &Node, event: &KeyEvent) {
    let modifiers = event.modifiers;

    if event.down && event.key == "Tab" && !modifiers.control && !modifiers.alt {
      self.move_focus(modifiers.shift);

      return;
    }

    if target.spec.acts && self.focus.is_some() {
      let response = Activate::new(target.key_pressed.get()).key(event);

      if let Some(pressed) = response.pressed {
        Self::press_key(target, pressed);
      }

      if response.act.is_some() {
        self.act(target, Act::Activate);
      }

      if response.consumed {
        return;
      }
    }

    if let Some(link) = self.link
      && self.focus.is_some()
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
      self.close_popup();

      if let Some(focus) = self.focus.clone() {
        Self::release_key(&focus);
      }
    }

    invalidate_all();
  }

  /// Carries out a reader's action at `node` (from the accessibility layer).
  pub fn access_action(&mut self, node: &Node, action: Action) {
    match node.spec.kind {
      NodeKind::Control(kind) if controls::is_entry(kind) => {
        self.set_focus(Some(Rc::clone(node)), false);
      }
      NodeKind::Control(kind) => {
        let open = self.open_state(node);

        if let Some(act) = node.control_action(kind, action, open) {
          self.act(node, act);
        }
      }
      _ if node.spec.acts && action == Action::Activate => self.act(node, Act::Activate),
      _ => {}
    }
  }

  /// Chooses option `index` of the open list from the accessibility layer.
  pub fn access_choose(&mut self, index: usize) {
    if let Some(list) = &self.popup {
      let owner = Rc::clone(&list.owner);

      self.act(&owner, Act::Popup(PopupAct::ChooseAndClose(index)));
    }
  }

  /// Sets a slider's value from the accessibility layer.
  pub fn access_set_number(&mut self, node: &Node, value: f64) {
    if matches!(
      controls_kind(node),
      Some(ControlKind::Slider | ControlKind::Stepper)
    ) && !node.contents.borrow().disabled
    {
      let range = node.range();
      let slider = skene_foundation::behavior::Slider::new(range);

      if let Some(value) = slider.moved(value) {
        self.act(
          node,
          Act::Change(
            Content::Range,
            PropValue::Range(skene_foundation::seam::content::Range { value, ..range }),
          ),
        );
      }
    }
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
      .filter(|focus| controls_kind(focus).is_some_and(controls::is_entry) && focus.has_entry())
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

  /// The last frame painted.
  pub fn last_frame(&self) -> Option<&vello_cpu::Pixmap> {
    (self.frames > 0).then_some(&self.renderer.pixmap)
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

  let entry = controls_kind(node).is_some_and(controls::is_entry);

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
    Content::Text | Content::Runs | Content::Caption | Content::Choice | Content::Placeholder => {
      invalidate_prepare();
    }
    Content::Secure | Content::ReadOnly if entry => {
      invalidate_prepare();
      entry::ime_dirty(false);
    }
    Content::Range if controls_kind(node) == Some(ControlKind::Stepper) => invalidate_prepare(),
    Content::Prominent | Content::Enabled => invalidate_prepare(),
    _ => {}
  }

  crate::access::invalidate();
  invalidate_paint(node);
}
