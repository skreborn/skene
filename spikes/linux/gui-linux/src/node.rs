//! The realized tree: one [`Drawn`] per realized node, its children in the scene's order, and
//! what the realization keeps of it (its resolved look and run properties, its layout, its
//! text, its control's values, where it was laid out).

use std::{
  cell::{Cell, RefCell},
  rc::{Rc, Weak},
};

use skene_foundation::{
  Color, NodeId, StateSet,
  layout::{Axis, ContainerLayout, Measurement, Rect, ResolvedLayout},
  seam::{
    access::AccessFacts,
    node::{LayoutKind, NodeKind, NodeSpec},
    report::Interest,
  },
};

use crate::{
  controls::Contents,
  layout::Host,
  look::{Look, RunProps},
  text::TextCell,
};

/// A realized node, shared.
pub type Node = Rc<Drawn>;

/// A node's measurements and the layout epoch they were made in: each along an axis, given
/// the other axis's size (its bits) where known.
pub type Measures = (u64, Vec<(Axis, Option<u32>, Measurement)>);

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
  /// Its paragraph: a text's, or a control's caption.
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
  /// Its accessibility facts, as last applied.
  pub access: RefCell<AccessFacts>,
  /// Its measurements in the current layout epoch.
  pub measures: RefCell<Measures>,
  /// The size last reported ([`Interest::size`]).
  pub reported: Cell<Option<(f32, f32, f64)>>,
  /// The layout epoch its engine's children were last given in.
  pub engine_epoch: Cell<u64>,
  /// What it and its subtree paint, relative to its frame's origin.
  pub reach: Cell<Rect>,
  /// Whether `Space` holds it pressed from the keyboard.
  pub key_pressed: Cell<bool>,
  /// Its editing state, where it is a text entry.
  pub entry: RefCell<Option<crate::entry::Entry>>,
  /// Whether an input method composes in it, as of its entry's last change.
  pub composing: Cell<bool>,
  /// Whether a write of its text waits for an operation on its entry to end.
  pub edit_pending: Cell<bool>,
  /// The text color it inherits, as last prepared: what its border, its ring and a painted
  /// node's ink default to.
  pub ink: Cell<Color>,
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
      access: RefCell::new(AccessFacts::default()),
      measures: RefCell::new((0, Vec::new())),
      reported: Cell::new(None),
      engine_epoch: Cell::new(0),
      reach: Cell::new(Rect::default()),
      key_pressed: Cell::new(false),
      entry: RefCell::new(None),
      composing: Cell::new(false),
      edit_pending: Cell::new(false),
      ink: Cell::new(Color::BLACK),
    })
  }

  /// Its layout kind, where it is a container.
  pub const fn layout_kind(&self) -> Option<LayoutKind> {
    match self.spec.kind {
      NodeKind::Layout(kind) => Some(kind),
      _ => None,
    }
  }

  /// Whether it is a Skene container the engine lays out.
  pub const fn is_container(&self) -> bool {
    matches!(self.spec.kind, NodeKind::Layout(_))
  }

  /// Its parent, where it is attached.
  pub fn parent(&self) -> Option<Node> {
    self.parent.borrow().upgrade()
  }

  /// Its children, in order.
  pub fn children(&self) -> Vec<Node> {
    self.children.borrow().clone()
  }

  /// How many children it has.
  pub fn child_count(&self) -> usize {
    self.children.borrow().len()
  }

  /// Whether it is hidden to be shown again.
  pub fn is_hidden(&self) -> bool {
    self.hidden.get()
  }

  /// Hides or shows it.
  pub fn set_hidden(&self, hidden: bool) {
    self.hidden.set(hidden);
  }

  /// Whether it is still part of the tree (not removed for good).
  pub fn is_alive(&self) -> bool {
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

  /// How far it is drawn from where it was laid out, in pixels (its style's offset).
  pub fn offset(&self) -> (f32, f32) {
    let frame = self.frame.get();

    self.look.borrow().offset_at(frame.width, frame.height)
  }

  /// Its frame in window coordinates, where it is drawn: the frames and offsets of its
  /// ancestors and its own added, their scroll offsets taken off.
  pub fn window_rect(self: &Rc<Self>) -> Rect {
    let mut rect = self.frame.get();
    let (dx, dy) = self.offset();

    rect.x += dx;
    rect.y += dy;

    for ancestor in self.ancestors() {
      let frame = ancestor.frame.get();
      let (sx, sy) = ancestor.scroll.get();
      let (dx, dy) = ancestor.offset();

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
