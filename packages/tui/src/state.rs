//! The terminal driver's node tree, handlers, and focus.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use rustc_hash::FxHashMap;
use skene_core::{KeyEvent, Layout, MouseCursor, NodeId};

use crate::{
  props::{BoxStyle, Condition, RunStyle},
  style::{Environment, Interaction, effective_box_style},
};

/// The virtual root every mounted view attaches under.
pub(crate) const ROOT_ID: NodeId = NodeId::new(0);

/// What a node realizes as.
pub(crate) enum NodeKind {
  /// A container: a layout mode plus box styling.
  Box {
    layout: Layout,
    style: BoxStyle,
    variants: Vec<(Condition, BoxStyle)>,
  },
  /// A text run, sized by its text.
  Run {
    style: RunStyle,
    variants: Vec<(Condition, RunStyle)>,
    text: String,
  },
  /// A single-line text input.
  Input {
    style: BoxStyle,
    variants: Vec<(Condition, BoxStyle)>,
    value: String,
    placeholder: Option<String>,
  },
  /// A structural anchor with no visuals of its own.
  Group,
}

/// One node's record: its realization, its taffy node, and its place in the tree.
pub(crate) struct NodeRecord {
  pub(crate) kind: NodeKind,
  pub(crate) taffy: taffy::NodeId,
  pub(crate) children: Vec<NodeId>,
  pub(crate) parent: Option<NodeId>,
  pub(crate) in_stack: bool,
}

/// A node's handlers of one event kind.
pub(crate) type Handlers<F> = RefCell<FxHashMap<NodeId, Vec<Rc<F>>>>;

/// The terminal driver's state: the node tree, the taffy tree, handlers, and focus.
pub(crate) struct State {
  pub(crate) nodes: RefCell<FxHashMap<NodeId, NodeRecord>>,
  pub(crate) taffy: RefCell<taffy::TaffyTree<()>>,
  next_id: Cell<u64>,
  pub(crate) click: Handlers<dyn Fn()>,
  pub(crate) focus: Handlers<dyn Fn()>,
  pub(crate) blur: Handlers<dyn Fn()>,
  pub(crate) key_down: Handlers<dyn Fn(&KeyEvent)>,
  pub(crate) input: Handlers<dyn Fn(&str)>,
  pub(crate) pointer_enter: Handlers<dyn Fn()>,
  pub(crate) pointer_leave: Handlers<dyn Fn()>,
  pub(crate) focused: Cell<Option<NodeId>>,
  /// The path from the root to the deepest node under the mouse, when known.
  pub(crate) hovered: RefCell<Vec<NodeId>>,
  /// The size of the last computed layout, which mouse hit-testing resolves against.
  pub(crate) last_size: Cell<(u16, u16)>,
  pub(crate) dirty: Cell<bool>,
  pub(crate) quit: Cell<bool>,
}

impl State {
  pub(crate) fn new() -> Self {
    let mut taffy = taffy::TaffyTree::new();
    let root_taffy = taffy
      .new_leaf(taffy::style::Style {
        display: taffy::style::Display::Flex,
        flex_direction: taffy::style::FlexDirection::Column,
        ..taffy::style::Style::default()
      })
      .expect("create the root taffy node");

    let mut nodes = FxHashMap::default();

    nodes.insert(
      ROOT_ID,
      NodeRecord {
        kind: NodeKind::Group,
        taffy: root_taffy,
        children: Vec::new(),
        parent: None,
        in_stack: false,
      },
    );

    Self {
      nodes: RefCell::new(nodes),
      taffy: RefCell::new(taffy),
      next_id: Cell::new(1),
      click: RefCell::new(FxHashMap::default()),
      focus: RefCell::new(FxHashMap::default()),
      blur: RefCell::new(FxHashMap::default()),
      key_down: RefCell::new(FxHashMap::default()),
      input: RefCell::new(FxHashMap::default()),
      pointer_enter: RefCell::new(FxHashMap::default()),
      pointer_leave: RefCell::new(FxHashMap::default()),
      focused: Cell::new(None),
      hovered: RefCell::new(Vec::new()),
      last_size: Cell::new((80, 24)),
      dirty: Cell::new(true),
      quit: Cell::new(false),
    }
  }

  pub(crate) fn insert_node(&self, kind: NodeKind, taffy_style: taffy::style::Style) -> NodeId {
    let id = NodeId::new(self.next_id.get());

    self.next_id.set(self.next_id.get() + 1);

    let taffy = self
      .taffy
      .borrow_mut()
      .new_leaf(taffy_style)
      .expect("create a taffy node");

    self.nodes.borrow_mut().insert(
      id,
      NodeRecord {
        kind,
        taffy,
        children: Vec::new(),
        parent: None,
        in_stack: false,
      },
    );

    self.dirty.set(true);

    id
  }

  pub(crate) fn attach_child(&self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    let mut nodes = self.nodes.borrow_mut();

    if let Some(old_parent) = nodes.get(&child).and_then(|record| record.parent)
      && let Some(old) = nodes.get_mut(&old_parent)
    {
      old.children.retain(|node| *node != child);
    }

    let (child_taffy, old_parent_taffy) = {
      let record = nodes.get_mut(&child).expect("tui driver: unknown node id");
      let old = record.parent.take();

      record.parent = Some(parent);

      (
        record.taffy,
        old.map(|old| nodes.get(&old).expect("a recorded parent").taffy),
      )
    };

    let parent_record = nodes.get_mut(&parent).expect("tui driver: unknown node id");
    let index = before
      .and_then(|before| {
        parent_record
          .children
          .iter()
          .position(|node| *node == before)
      })
      .unwrap_or(parent_record.children.len());

    parent_record.children.insert(index, child);

    let parent_taffy = parent_record.taffy;

    drop(nodes);

    let mut taffy = self.taffy.borrow_mut();

    if let Some(old) = old_parent_taffy {
      let _ = taffy.remove_child(old, child_taffy);
    }

    taffy
      .insert_child_at_index(parent_taffy, index, child_taffy)
      .expect("attach a taffy child");

    self.dirty.set(true);
  }

  pub(crate) fn detach_child(&self, parent: NodeId, child: NodeId) {
    let mut nodes = self.nodes.borrow_mut();

    if let Some(record) = nodes.get_mut(&parent) {
      record.children.retain(|node| *node != child);
    }

    let parent_taffy = nodes.get(&parent).map(|record| record.taffy);
    let child_taffy = nodes.get(&child).map(|record| record.taffy);

    if let Some(record) = nodes.get_mut(&child) {
      record.parent = None;
    }

    drop(nodes);

    if let (Some(parent_taffy), Some(child_taffy)) = (parent_taffy, child_taffy) {
      let _ = self
        .taffy
        .borrow_mut()
        .remove_child(parent_taffy, child_taffy);
    }

    self.dirty.set(true);
  }

  pub(crate) fn destroy(&self, node: NodeId) {
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
      let record = self.nodes.borrow_mut().remove(&current);

      if let Some(record) = record {
        stack.extend(record.children);

        let _ = self.taffy.borrow_mut().remove(record.taffy);
      }

      self.click.borrow_mut().remove(&current);
      self.focus.borrow_mut().remove(&current);
      self.blur.borrow_mut().remove(&current);
      self.key_down.borrow_mut().remove(&current);
      self.input.borrow_mut().remove(&current);
      self.pointer_enter.borrow_mut().remove(&current);
      self.pointer_leave.borrow_mut().remove(&current);
      self.hovered.borrow_mut().retain(|node| *node != current);

      if self.focused.get() == Some(current) {
        self.focused.set(None);
      }
    }

    self.dirty.set(true);
  }

  /// Runs `apply` on a node's record.
  pub(crate) fn with_node<R>(&self, node: NodeId, apply: impl FnOnce(&mut NodeRecord) -> R) -> R {
    let mut nodes = self.nodes.borrow_mut();
    let record = nodes.get_mut(&node).expect("tui driver: unknown node id");

    apply(record)
  }

  /// The focusable nodes (inputs and boxes with click handlers), in tree order.
  pub(crate) fn focusable(&self) -> Vec<NodeId> {
    let nodes = self.nodes.borrow();
    let click = self.click.borrow();
    let mut order = Vec::new();
    let mut stack = vec![ROOT_ID];

    while let Some(current) = stack.pop() {
      if let Some(record) = nodes.get(&current) {
        if matches!(record.kind, NodeKind::Input { .. }) || click.contains_key(&current) {
          order.push(current);
        }

        // Reverse, so the stack pops children in tree order.
        stack.extend(record.children.iter().rev());
      }
    }

    order
  }

  /// The path from `node` up to the root, for bubbling dispatch.
  pub(crate) fn ancestors(&self, node: NodeId) -> Vec<NodeId> {
    let nodes = self.nodes.borrow();
    let mut path = Vec::new();
    let mut current = Some(node);

    while let Some(id) = current {
      path.push(id);
      current = nodes.get(&id).and_then(|record| record.parent);
    }

    path
  }

  /// Runs `node`'s handlers from `handlers` with `payload`, cloning them out first so a
  /// handler may re-enter the driver and build or destroy nodes.
  pub(crate) fn run_handlers<F: ?Sized, P: Copy>(
    handlers: &Handlers<F>,
    node: NodeId,
    payload: P,
    invoke: impl Fn(&F, P),
  ) {
    let Some(to_run) = handlers.borrow().get(&node).cloned()
    else {
      return;
    };

    for handler in to_run {
      invoke(&handler, payload);
    }
  }

  /// Moves focus by `step` through the focusable nodes, firing blur and focus handlers.
  pub(crate) fn move_focus(&self, step: isize) {
    let order = self.focusable();

    if order.is_empty() {
      return;
    }

    let next = match self
      .focused
      .get()
      .and_then(|focused| order.iter().position(|node| *node == focused))
    {
      Some(index) => {
        let count = isize::try_from(order.len()).expect("a small focus order");
        let index = isize::try_from(index).expect("a small focus order");

        order[usize::try_from((index + step).rem_euclid(count)).expect("in range")]
      }
      None => {
        if step >= 0 {
          order[0]
        }
        else {
          order[order.len() - 1]
        }
      }
    };

    self.set_focused(Some(next));
  }

  /// Sets the focused node, firing blur and focus handlers on change.
  pub(crate) fn set_focused(&self, next: Option<NodeId>) {
    let previous = self.focused.replace(next);

    if previous == next {
      return;
    }

    if let Some(previous) = previous {
      Self::run_handlers(&self.blur, previous, (), |run, ()| run());
    }

    if let Some(next) = next {
      Self::run_handlers(&self.focus, next, (), |run, ()| run());
    }

    self.dirty.set(true);
  }

  /// The path from the root to the deepest node containing the cell (`x`, `y`), against
  /// the last computed layout.
  pub(crate) fn hit_test(&self, x: u16, y: u16) -> Vec<NodeId> {
    let mut path = Vec::new();

    self.hit_walk(ROOT_ID, 0, 0, i32::from(x), i32::from(y), &mut path);

    path
  }

  /// Descends into the deepest child containing the point, preferring later (topmost)
  /// children, accumulating the path.
  fn hit_walk(
    &self,
    node: NodeId,
    offset_x: i32,
    offset_y: i32,
    x: i32,
    y: i32,
    path: &mut Vec<NodeId>,
  ) -> bool {
    let (origin_x, origin_y, width, height, children) = {
      let nodes = self.nodes.borrow();
      let Some(record) = nodes.get(&node)
      else {
        return false;
      };

      let Ok(layout) = self.taffy.borrow().layout(record.taffy).copied()
      else {
        return false;
      };

      #[expect(clippy::cast_possible_truncation)]
      let (origin_x, origin_y) = (
        offset_x + layout.location.x.round() as i32,
        offset_y + layout.location.y.round() as i32,
      );

      #[expect(clippy::cast_possible_truncation)]
      let (width, height) = (
        layout.size.width.round().max(0.0) as i32,
        layout.size.height.round().max(0.0) as i32,
      );

      (origin_x, origin_y, width, height, record.children.clone())
    };

    let contains = x >= origin_x && x < origin_x + width && y >= origin_y && y < origin_y + height;

    if node != ROOT_ID && !contains {
      return false;
    }

    path.push(node);

    // Later children paint on top, so they win the hit.
    for child in children.iter().rev() {
      if self.hit_walk(*child, origin_x, origin_y, x, y, path) {
        break;
      }
    }

    true
  }

  /// Updates the hover path to the nodes under (`x`, `y`), firing pointer-leave and
  /// pointer-enter handlers for the difference.
  pub(crate) fn set_hover(&self, x: u16, y: u16) {
    let next = self.hit_test(x, y);
    let previous = self.hovered.borrow().clone();

    if previous == next {
      return;
    }

    for node in previous.iter().rev() {
      if !next.contains(node) {
        Self::run_handlers(&self.pointer_leave, *node, (), |run, ()| run());
      }
    }

    for node in &next {
      if !previous.contains(node) {
        Self::run_handlers(&self.pointer_enter, *node, (), |run, ()| run());
      }
    }

    *self.hovered.borrow_mut() = next;
    self.dirty.set(true);
  }

  /// Dispatches a mouse click at (`x`, `y`): focuses the deepest focusable node on the
  /// hit path (or blurs when none is hit), then runs click handlers from the deepest
  /// node up.
  pub(crate) fn mouse_click(&self, x: u16, y: u16) {
    let path = self.hit_test(x, y);
    let focusable = self.focusable();
    let target = path
      .iter()
      .rev()
      .find(|node| focusable.contains(node))
      .copied();

    self.set_focused(target);

    for node in path.iter().rev() {
      Self::run_handlers(&self.click, *node, (), |run, ()| run());
    }
  }

  /// The pointer shape for the current hover path: the deepest hovered node with a
  /// cursor set, resolved through its effective style.
  pub(crate) fn hover_cursor(&self) -> Option<MouseCursor> {
    let (columns, rows) = self.last_size.get();
    let environment = Environment { columns, rows };
    let hovered = self.hovered.borrow();
    let nodes = self.nodes.borrow();

    hovered.iter().rev().find_map(|node| {
      let record = nodes.get(node)?;
      let (NodeKind::Box {
        style, variants, ..
      }
      | NodeKind::Input {
        style, variants, ..
      }) = &record.kind
      else {
        return None;
      };

      let interaction = Interaction {
        focused: self.focused.get() == Some(*node),
        hovered: true,
      };

      effective_box_style(style, variants, environment, interaction).cursor
    })
  }
}
