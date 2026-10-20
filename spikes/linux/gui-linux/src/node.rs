//! The realized tree: one [`Drawn`] per realized node, its children in the scene's order, and
//! what the realization keeps of it (its resolved look and run properties, its layout, its
//! text, its content values, where it was laid out).

use std::{
  cell::{Cell, Ref, RefCell},
  rc::{Rc, Weak},
};

use skene_foundation::{
  Color, NodeId, StateSet,
  layout::{Axis, ContainerLayout, Measurement, Needed, Rect, ResolvedLayout},
  seam::{
    access::AccessFacts,
    content::{Content, Edit, OverlayPlacement, PropValue, Run},
    node::{Interaction, LayoutKind, NodeKind, NodeSpec},
    picture::Picture,
    report::{Interest, Size},
  },
};

use crate::{
  layout::Host,
  look::{Look, RunProps},
  text::TextCell,
};

/// A node's content values, as Skene last wrote them or the reader last changed them.
#[derive(Clone, Default, Debug)]
pub struct Contents {
  /// A text's plain text.
  pub text: Option<Rc<str>>,
  /// A text's rich text.
  pub runs: Option<Rc<[Run]>>,
  /// The texts a text lays out as wide as.
  pub reserve: Option<Rc<[Rc<str>]>>,
  /// A text entry's text and selection.
  pub edit: Option<Edit>,
  /// What an empty entry shows.
  pub placeholder: Option<Rc<str>>,
  /// Whether its text is read only.
  pub read_only: bool,
  /// Whether it hides what is typed.
  pub secure: bool,
  /// The keyboard it asks the input method for, by name.
  pub keyboard: Option<Rc<str>>,
  /// The picture an image shows.
  pub source: Option<Rc<str>>,
  /// Where an acting box leads.
  pub destination: Option<Rc<str>>,
  /// What a painted node shows.
  pub picture: Option<Rc<Picture>>,
  /// Where an overlay is placed.
  pub placement: Option<OverlayPlacement>,
}

impl Contents {
  /// Shows `value` as `content`.
  pub fn write(&mut self, content: Content, value: &PropValue) {
    match (content, value) {
      (Content::Text, PropValue::Text(text)) => self.text = Some(Rc::clone(text)),
      (Content::Runs, PropValue::Runs(runs)) => self.runs = Some(Rc::clone(runs)),
      (Content::Reserve, PropValue::Texts(texts)) => self.reserve = Some(Rc::clone(texts)),
      (Content::Edit, PropValue::Edit(edit)) => self.edit = Some(edit.clone()),
      (Content::Placeholder, PropValue::Text(text)) => self.placeholder = Some(Rc::clone(text)),
      (Content::ReadOnly, PropValue::Bool(read_only)) => self.read_only = *read_only,
      (Content::Secure, PropValue::Bool(secure)) => self.secure = *secure,
      (Content::Keyboard, PropValue::Text(keyboard)) => self.keyboard = Some(Rc::clone(keyboard)),
      (Content::Source, PropValue::Text(text)) => self.source = Some(Rc::clone(text)),
      (Content::Destination, PropValue::Text(text)) => self.destination = Some(Rc::clone(text)),
      (Content::Picture, PropValue::Picture(picture)) => self.picture = Some(Rc::clone(picture)),
      (Content::Placement, PropValue::Placement(placement)) => self.placement = Some(*placement),
      _ => {}
    }
  }

  /// The value shown as `content`.
  pub fn read(&self, content: Content) -> Option<PropValue> {
    let text = |text: &Option<Rc<str>>| text.clone().map(PropValue::Text);

    match content {
      Content::Text => text(&self.text),
      Content::Runs => self.runs.clone().map(PropValue::Runs),
      Content::Reserve => self.reserve.clone().map(PropValue::Texts),
      Content::Edit => Some(PropValue::Edit(
        self
          .edit
          .clone()
          .unwrap_or_else(|| Edit::text(Rc::<str>::from(""))),
      )),
      Content::Placeholder => text(&self.placeholder),
      Content::ReadOnly => Some(PropValue::Bool(self.read_only)),
      Content::Secure => Some(PropValue::Bool(self.secure)),
      Content::Keyboard => text(&self.keyboard),
      Content::Source => text(&self.source),
      Content::Destination => text(&self.destination),
      Content::Picture => self.picture.clone().map(PropValue::Picture),
      Content::Placement => self.placement.map(PropValue::Placement),
      Content::ScrollOffset | Content::Door(_) => None,
    }
  }
}

/// A realized node, shared.
pub type Node = Rc<Drawn>;

/// A node's measurements in one layout epoch.
#[derive(Default)]
pub struct Measures {
  /// The epoch they were made in.
  pub epoch: u64,
  /// Each along an axis, given the other axis's size (its bits) where known, with which of
  /// its sizes it holds.
  pub kept: Vec<(Axis, Option<u32>, Measurement, Needed)>,
  /// Which of its sizes do not depend on the other axis, where found.
  pub rigid: Option<Rigid>,
}

/// Which of a node's sizes do not depend on the other axis's: what a measure need not ask
/// again.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rigid {
  /// Its least height at a width is its natural height at that width.
  pub height: bool,
  /// Its widths do not depend on a height it is given.
  pub width: bool,
}

/// What the realization keeps per node.
pub struct Drawn {
  /// The scene node it realizes.
  pub id: Cell<NodeId>,
  /// What it is.
  pub spec: NodeSpec,
  parent: RefCell<Weak<Self>>,
  children: RefCell<Vec<Node>>,
  hidden: Cell<bool>,
  alive: Cell<bool>,
  /// Its box properties, resolved.
  pub look: RefCell<Look>,
  /// Its own run properties, which the texts beneath inherit.
  pub run: RefCell<RunProps>,
  /// Its own layout and its placement in its parent.
  pub layout: RefCell<ResolvedLayout>,
  /// The engine laying its children out, where it is a container.
  pub engine: RefCell<Option<ContainerLayout<Host>>>,
  /// Its paragraph: a text's, or a text entry's placeholder.
  pub text: RefCell<Option<Rc<TextCell>>>,
  /// Its content values.
  pub contents: RefCell<Contents>,
  /// Where it was laid out, in its parent's coordinates (its parent's border box), in whole
  /// pixels.
  pub frame: Cell<Rect>,
  /// The width the engine laid it out at before rounding: what its paragraph is laid out
  /// within.
  pub laid_width: Cell<f32>,
  /// What the shared half asked to hear from it.
  pub interest: Cell<Interest>,
  /// The states the realization last reported as holding.
  pub states: Cell<StateSet>,
  /// How far its content is scrolled, where it scrolls.
  pub scroll: Cell<(f32, f32)>,
  /// The size of what it scrolls, where it scrolls.
  pub extent: Cell<(f32, f32)>,
  /// Where it is a scroll bar's thumb: the axis it moves along and how far it can move along
  /// its track, as last laid out.
  pub thumb: Cell<Option<(Axis, f32)>>,
  /// Its accessibility facts, as last applied.
  pub access: RefCell<AccessFacts>,
  /// Its measurements in the current layout epoch.
  pub measures: RefCell<Measures>,
  /// The size last reported ([`Interest::size`]).
  pub reported: Cell<Option<Size>>,
  /// The layout epoch its engine's children were last given in.
  pub engine_epoch: Cell<u64>,
  /// What it and its subtree paint, relative to its frame's origin.
  pub reach: Cell<Rect>,
  /// Its editing state, where it is a text entry.
  pub entry: RefCell<Option<Box<crate::entry::Entry>>>,
  /// Whether an input method composes in it, as of its entry's last change.
  pub composing: Cell<bool>,
  /// Whether a write of its text waits for an operation on its entry to end.
  pub edit_pending: Cell<bool>,
  /// The text color it inherits, as last prepared: what its border, its ring and a painted
  /// node's ink default to.
  pub ink: Cell<Color>,
  /// Its motion, where its style names transitions or animations.
  pub motion: RefCell<Option<Box<crate::motion::Moving>>>,
}

impl Drawn {
  /// A detached node of `spec` realizing `id`.
  pub fn new(id: NodeId, spec: NodeSpec) -> Node {
    Rc::new(Self {
      id: Cell::new(id),
      spec,
      parent: RefCell::new(Weak::new()),
      children: RefCell::new(Vec::new()),
      hidden: Cell::new(false),
      alive: Cell::new(true),
      look: RefCell::new(Look::default()),
      run: RefCell::new(RunProps::default()),
      layout: RefCell::new(ResolvedLayout::default()),
      engine: RefCell::new(None),
      text: RefCell::new(None),
      contents: RefCell::new(Contents::default()),
      frame: Cell::new(Rect::default()),
      laid_width: Cell::new(0.),
      interest: Cell::new(Interest::default()),
      states: Cell::new(StateSet::EMPTY),
      scroll: Cell::new((0., 0.)),
      extent: Cell::new((0., 0.)),
      thumb: Cell::new(None),
      access: RefCell::new(AccessFacts::default()),
      measures: RefCell::new(Measures::default()),
      reported: Cell::new(None),
      engine_epoch: Cell::new(0),
      reach: Cell::new(Rect::default()),
      entry: RefCell::new(None),
      composing: Cell::new(false),
      edit_pending: Cell::new(false),
      ink: Cell::new(Color::BLACK),
      motion: RefCell::new(None),
    })
  }

  /// Its layout kind, where it is a container.
  pub const fn layout_kind(&self) -> Option<LayoutKind> {
    match self.spec.kind {
      NodeKind::Layout(kind) => Some(kind),
      _ => None,
    }
  }

  /// Whether it is a Skene container the engine lays out: a layout, or an overlay, which lays
  /// its content out as a box does.
  pub const fn is_container(&self) -> bool {
    matches!(self.spec.kind, NodeKind::Layout(_) | NodeKind::Overlay)
  }

  /// Whether it is an overlay: out of its parent's flow, drawn on a surface of its own.
  pub fn is_overlay(&self) -> bool {
    self.spec.kind == NodeKind::Overlay
  }

  /// The axis it scrolls its scroll box along, where it is a scroll bar's track: out of the
  /// box's scrolling content and unscrolled.
  pub const fn bar_axis(&self) -> Option<Axis> {
    match self.spec.interaction {
      Interaction::ScrollBar { axis } => Some(axis),
      _ => None,
    }
  }

  /// How far its content is scrolled under `child`: its scroll offset, and none under a scroll
  /// bar's track, which stays where it was laid.
  pub fn scroll_for(&self, child: &Self) -> (f32, f32) {
    if child.bar_axis().is_some() {
      (0., 0.)
    } else {
      self.scroll.get()
    }
  }

  /// Where along its scrollable length a scroll box is, `0` at the start to `1` at the end,
  /// along `axis`.
  pub fn scrolled_fraction(&self, axis: Axis) -> f32 {
    let frame = self.frame.get();
    let (width, height) = self.extent.get();
    let (sx, sy) = self.scroll.get();
    let (scroll, room) = match axis {
      Axis::Horizontal => (sx, width - frame.width),
      Axis::Vertical => (sy, height - frame.height),
    };

    if room <= 0. {
      0.
    } else {
      (scroll / room).clamp(0., 1.)
    }
  }

  /// Whether it is the realization's own text view (a text entry).
  pub const fn is_entry(&self) -> bool {
    matches!(self.spec.kind, NodeKind::Editable { .. })
  }

  /// The overlay it is inside, where it is inside one (an overlay is inside itself).
  pub fn overlay(self: &Rc<Self>) -> Option<Node> {
    std::iter::once(Rc::clone(self))
      .chain(self.ancestors())
      .find(|node| node.is_overlay())
  }

  /// Its parent, where it is attached.
  pub fn parent(&self) -> Option<Node> {
    self.parent.borrow().upgrade()
  }

  /// Its children, in order.
  pub fn children(&self) -> Vec<Node> {
    self.children.borrow().clone()
  }

  /// Its children, in order, borrowed: a walk that changes no node's children reads them
  /// without copying the list.
  pub fn children_ref(&self) -> Ref<'_, [Node]> {
    Ref::map(self.children.borrow(), Vec::as_slice)
  }

  /// How many children it has.
  pub fn child_count(&self) -> usize {
    self.children.borrow().len()
  }

  /// Whether it is hidden to be shown again.
  pub const fn is_hidden(&self) -> bool {
    self.hidden.get()
  }

  /// Hides or shows it.
  pub fn set_hidden(&self, hidden: bool) {
    self.hidden.set(hidden);
  }

  /// Whether it is still part of the tree (not removed for good).
  pub const fn is_alive(&self) -> bool {
    self.alive.get()
  }

  /// Whether it is shown: alive, and neither it nor an ancestor hidden.
  pub fn is_shown(self: &Rc<Self>) -> bool {
    let mut next = Some(Rc::clone(self));

    while let Some(node) = next {
      if !node.is_alive() || node.is_hidden() {
        return false;
      }

      next = node.parent();
    }

    true
  }

  /// Places `child` at `index` among the children, taking it from where it was.
  pub fn insert(self: &Rc<Self>, child: &Node, index: usize) {
    if let Some(old) = child.parent() {
      old
        .children
        .borrow_mut()
        .retain(|node| !Rc::ptr_eq(node, child));
    }

    let mut children = self.children.borrow_mut();
    let index = index.min(children.len());

    children.insert(index, Rc::clone(child));
    *child.parent.borrow_mut() = Rc::downgrade(self);
  }

  /// Removes it and everything beneath it for good.
  pub fn remove(self: &Rc<Self>) {
    if let Some(parent) = self.parent() {
      parent
        .children
        .borrow_mut()
        .retain(|node| !Rc::ptr_eq(node, self));
    }

    *self.parent.borrow_mut() = Weak::new();
    self.retire();
  }

  fn retire(&self) {
    self.alive.set(false);
    *self.engine.borrow_mut() = None;
    *self.text.borrow_mut() = None;
    *self.entry.borrow_mut() = None;

    for child in self.children.take() {
      *child.parent.borrow_mut() = Weak::new();
      child.retire();
    }
  }

  /// Its ancestors, nearest first.
  pub fn ancestors(self: &Rc<Self>) -> Vec<Node> {
    let mut out = Vec::new();
    let mut next = self.parent();

    while let Some(node) = next {
      next = node.parent();
      out.push(node);
    }

    out
  }

  /// Its link runs, in the text's order, where it is a text that has some.
  pub fn links(&self) -> crate::text::Links {
    self
      .text
      .borrow()
      .as_ref()
      .map(|cell| cell.links())
      .unwrap_or_default()
  }

  /// How far it is drawn from where it was laid out, in pixels: its style's offset, and a
  /// scroll bar's thumb moved along its track to where its scroll box is scrolled.
  pub fn offset(&self) -> (f32, f32) {
    let frame = self.frame.get();
    let (dx, dy) = self.look.borrow().offset_at(frame.width, frame.height);
    let Some((axis, travel)) = self.thumb.get() else {
      return (dx, dy);
    };
    let along = self
      .parent()
      .and_then(|track| track.parent())
      .map_or(0., |scroll| travel * scroll.scrolled_fraction(axis));

    match axis {
      Axis::Horizontal => (dx + along, dy),
      Axis::Vertical => (dx, dy + along),
    }
  }

  /// Its frame in window coordinates, where it is drawn: the frames and offsets of its
  /// ancestors and its own added, their scroll offsets taken off, up to the overlay it is in,
  /// whose frame is in window coordinates already.
  /// Whether it is a scroll box that takes focus as a tab stop of its own, as Chromium's
  /// keyboard-focusable scrollers do: shown, its content overflowing it, and nothing shown
  /// inside it taking focus, so its keys can scroll it.
  pub fn is_scroll_stop(self: &Rc<Self>) -> bool {
    let Some(LayoutKind::Scroll {
      horizontal,
      vertical,
    }) = self.layout_kind()
    else {
      return false;
    };
    let frame = self.frame.get();
    let (width, height) = self.extent.get();
    let overflows =
      horizontal && width > frame.width + 0.5 || vertical && height > frame.height + 0.5;

    if !overflows || !self.is_shown() {
      return false;
    }

    let mut holds_stop = false;

    walk(self, |node| {
      holds_stop |= !Rc::ptr_eq(node, self)
        && node.is_shown()
        && match node.spec.interaction {
          Interaction::Control { .. } => node.access.borrow().focusable,
          Interaction::Acting => true,
          Interaction::None | Interaction::Answering | Interaction::ScrollBar { .. } => false,
        };
    });

    !holds_stop
  }

  pub fn window_rect(self: &Rc<Self>) -> Rect {
    let mut rect = self.frame.get();

    if self.is_overlay() {
      return rect;
    }

    let (dx, dy) = self.offset();

    rect.x += dx;
    rect.y += dy;

    let mut child = Rc::clone(self);

    for ancestor in self.ancestors() {
      let frame = ancestor.frame.get();

      if ancestor.is_overlay() {
        rect.x += frame.x;
        rect.y += frame.y;

        break;
      }

      let (sx, sy) = ancestor.scroll_for(&child);
      let (dx, dy) = ancestor.offset();

      child = Rc::clone(&ancestor);

      rect.x += frame.x + dx - sx;
      rect.y += frame.y + dy - sy;
    }

    rect
  }
}

/// Every shown node beneath `root`, `root` first, in the scene's order (depth first).
pub fn walk(root: &Node, mut visit: impl FnMut(&Node)) {
  fn go(node: &Node, visit: &mut dyn FnMut(&Node)) {
    if node.is_hidden() {
      return;
    }

    visit(node);

    for child in node.children.borrow().iter() {
      go(child, visit);
    }
  }

  go(root, &mut visit);
}
