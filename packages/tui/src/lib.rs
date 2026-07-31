//! The skene terminal driver.
//!
//! Implements the typed driver vocabulary for a terminal: taffy computes the layout with
//! `1.0 = 1 cell`, and the tree paints into a cell buffer—to the terminal through
//! [`launch`], or to a plain string through [`render_to_text`] for headless tests.
//! Custom-painted and embedded content have no realization here.
//!
//! Interaction is keyboard-first: `Tab`/`Shift-Tab` move focus across inputs and
//! clickable boxes, `Enter` clicks the focused node, printable keys edit the focused
//! input, and `Ctrl-C` quits. Every dispatch runs inside a reactive batch.
//!
//! Host-only: on a wasm target the crate is empty.

#![cfg(not(target_arch = "wasm32"))]

mod paint;
mod style;
mod terminal;

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use rustc_hash::FxHashMap;
use skene_core::{
  BoxProp,
  BoxProps,
  BoxStyle,
  Condition,
  InputProps,
  KeyEvent,
  Layout,
  NodeId,
  RunProp,
  RunProps,
  RunStyle,
};
pub use terminal::launch;
#[cfg(feature = "test")]
pub use terminal::{focused_node, trigger_key};

/// The virtual root every mounted view attaches under.
const ROOT_ID: NodeId = NodeId::new(0);

/// What a node realizes as.
enum NodeKind {
  /// A container: a layout mode plus box styling.
  Box {
    layout: Layout,
    style: BoxStyle,
    variants: Vec<(Vec<Condition>, BoxStyle)>,
  },
  /// A text run. The run's display mode needs no realization here: a run is sized by
  /// its text, and a group run by its children.
  Run {
    style: RunStyle,
    variants: Vec<(Vec<Condition>, RunStyle)>,
    text: String,
  },
  /// A single-line text input.
  Input {
    style: BoxStyle,
    variants: Vec<(Vec<Condition>, BoxStyle)>,
    value: String,
    placeholder: Option<String>,
  },
  /// A structural anchor with no visuals of its own.
  Group,
}

/// One node's record: its realization, its taffy node, and its place in the tree.
struct NodeRecord {
  kind: NodeKind,
  taffy: taffy::NodeId,
  children: Vec<NodeId>,
  parent: Option<NodeId>,
  in_stack: bool,
}

/// A node's handlers of one event kind.
type Handlers<F> = RefCell<FxHashMap<NodeId, Vec<Rc<F>>>>;

/// The terminal driver's state: the node tree, the taffy tree, handlers, and focus.
struct State {
  nodes: RefCell<FxHashMap<NodeId, NodeRecord>>,
  taffy: RefCell<taffy::TaffyTree<()>>,
  next_id: Cell<u64>,
  click: Handlers<dyn Fn()>,
  focus: Handlers<dyn Fn()>,
  blur: Handlers<dyn Fn()>,
  key_down: Handlers<dyn Fn(&KeyEvent)>,
  input: Handlers<dyn Fn(&str)>,
  focused: Cell<Option<NodeId>>,
  dirty: Cell<bool>,
  quit: Cell<bool>,
}

impl State {
  fn new() -> Self {
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
      focused: Cell::new(None),
      dirty: Cell::new(true),
      quit: Cell::new(false),
    }
  }

  fn insert_node(&self, kind: NodeKind, taffy_style: taffy::style::Style) -> NodeId {
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

  fn attach_child(&self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
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

  fn detach_child(&self, parent: NodeId, child: NodeId) {
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

  fn destroy(&self, node: NodeId) {
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

      if self.focused.get() == Some(current) {
        self.focused.set(None);
      }
    }

    self.dirty.set(true);
  }

  /// Runs `apply` on a node's record.
  fn with_node<R>(&self, node: NodeId, apply: impl FnOnce(&mut NodeRecord) -> R) -> R {
    let mut nodes = self.nodes.borrow_mut();
    let record = nodes.get_mut(&node).expect("tui driver: unknown node id");

    apply(record)
  }

  /// The focusable nodes (inputs and boxes with click handlers), in tree order.
  fn focusable(&self) -> Vec<NodeId> {
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
  fn ancestors(&self, node: NodeId) -> Vec<NodeId> {
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
  fn run_handlers<F: ?Sized, P: Copy>(
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
  fn move_focus(&self, step: isize) {
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

    let previous = self.focused.replace(Some(next));

    if let Some(previous) = previous {
      Self::run_handlers(&self.blur, previous, (), |run, ()| run());
    }

    Self::run_handlers(&self.focus, next, (), |run, ()| run());
    self.dirty.set(true);
  }
}

thread_local! {
  static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Runs `apply` against the installed driver state.
///
/// # Panics
///
/// Panics if the terminal driver is not installed for this thread.
fn with_state<R>(apply: impl FnOnce(&State) -> R) -> R {
  STATE.with(|state| {
    apply(
      state
        .borrow()
        .as_ref()
        .expect("the terminal driver to be installed for this thread"),
    )
  })
}

/// Installs the terminal driver for this thread.
///
/// # Panics
///
/// Panics if it is already installed.
pub fn install() {
  skene_reactive::Runtime::init_on_ui_thread();

  STATE.with(|state| {
    let mut state = state.borrow_mut();

    assert!(
      state.is_none(),
      "the terminal driver is already installed for this thread",
    );

    *state = Some(State::new());
  });
}

/// Uninstalls the terminal driver and resets the reactive runtime and root scope.
///
/// Lets one thread run many independent mounts, as a test suite does.
#[cfg(feature = "test")]
pub fn uninstall() {
  skene_reactive::Runtime::reset();
  skene_core::reset_root_scope();

  STATE.with(|state| *state.borrow_mut() = None);
}

/// Builds `view` under the installed root, returning its node.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  skene_core::root_scope().enter(|| {
    let node = view();

    with_state(|state| state.attach_child(ROOT_ID, node, None));

    node
  })
}

/// Lays the mounted tree out at `width` x `height` cells and paints it to plain text—
/// the headless render a test asserts against.
#[must_use]
pub fn render_to_text(width: u16, height: u16) -> String {
  with_state(|state| paint::render(state, width, height).to_text())
}

/// Realizes a box for its layout mode and static style.
#[must_use]
pub fn create_box(props: &BoxProps) -> NodeId {
  with_state(|state| {
    state.insert_node(
      NodeKind::Box {
        layout: props.layout,
        style: props.style,
        variants: props.variants.clone(),
      },
      style::box_taffy_style(props.layout, &props.style, false),
    )
  })
}

/// Applies one changed box property.
pub fn update_box(node: NodeId, prop: &BoxProp) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Box { style, .. } = &mut record.kind {
        apply_box_prop(style, prop);
      }
    });

    state.dirty.set(true);
  });
}

/// Overlaps a stack child in the box's single cell.
pub fn place_in_stack(node: NodeId) {
  with_state(|state| {
    state.with_node(node, |record| record.in_stack = true);
    state.dirty.set(true);
  });
}

/// Realizes a text run.
#[must_use]
pub fn create_run(props: &RunProps) -> NodeId {
  with_state(|state| {
    state.insert_node(
      NodeKind::Run {
        style: props.style,
        variants: props.variants.clone(),
        text: String::new(),
      },
      taffy::style::Style::default(),
    )
  })
}

/// Applies one changed text-run property.
pub fn update_run(node: NodeId, prop: &RunProp) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Run { style, .. } = &mut record.kind {
        match prop {
          RunProp::Color(value) => style.color = Some(*value),
          RunProp::FontSize(value) => style.font_size = Some(*value),
          RunProp::FontWeight(value) => style.font_weight = Some(*value),
        }
      }
    });

    state.dirty.set(true);
  });
}

/// Realizes a single-line text input.
#[must_use]
pub fn create_input(props: &InputProps) -> NodeId {
  with_state(|state| {
    state.insert_node(
      NodeKind::Input {
        style: props.style,
        variants: props.variants.clone(),
        value: String::new(),
        placeholder: props.placeholder.clone(),
      },
      taffy::style::Style::default(),
    )
  })
}

/// Applies one changed input style property.
pub fn update_input(node: NodeId, prop: &BoxProp) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Input { style, .. } = &mut record.kind {
        apply_box_prop(style, prop);
      }
    });

    state.dirty.set(true);
  });
}

/// Sets the live value of an input node.
pub fn set_input_value(node: NodeId, value: &str) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Input { value: current, .. } = &mut record.kind {
        value.clone_into(current);
      }
    });

    state.dirty.set(true);
  });
}

/// Sets a node's text content.
pub fn set_text(node: NodeId, text: &str) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Run { text: current, .. } = &mut record.kind {
        text.clone_into(current);
      }
    });

    state.dirty.set(true);
  });
}

/// Allocates a detached grouping node—a block box, as every driver's group is.
#[must_use]
pub fn create_group() -> NodeId {
  with_state(|state| {
    state.insert_node(
      NodeKind::Group,
      taffy::style::Style {
        display: taffy::style::Display::Block,
        ..taffy::style::Style::default()
      },
    )
  })
}

/// Attaches `child` to `parent`, immediately before `before` when given, otherwise at
/// the end.
pub fn attach(parent: NodeId, child: NodeId, before: Option<NodeId>) {
  with_state(|state| state.attach_child(parent, child, before));
}

/// Detaches `child` from `parent`.
pub fn detach(parent: NodeId, child: NodeId) {
  with_state(|state| state.detach_child(parent, child));
}

/// Destroys an already-detached node and its entire subtree.
pub fn destroy(node: NodeId) {
  with_state(|state| state.destroy(node));
}

/// Registers `handler` to run when `node` is activated (`Enter` while focused, or while
/// a descendant is), until the node is destroyed. A box with a click handler becomes
/// focusable.
pub fn handle_click(node: NodeId, handler: Rc<dyn Fn()>) {
  with_state(|state| {
    state
      .click
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);
  });
}

/// Registers a pointer-enter handler—the terminal has no pointer, so it never fires.
#[inline(always)]
pub fn handle_pointer_enter(_node: NodeId, _handler: Rc<dyn Fn()>) {}

/// Registers a pointer-leave handler—the terminal has no pointer, so it never fires.
#[inline(always)]
pub fn handle_pointer_leave(_node: NodeId, _handler: Rc<dyn Fn()>) {}

/// Registers `handler` to run when `node` gains focus, until the node is destroyed.
pub fn handle_focus(node: NodeId, handler: Rc<dyn Fn()>) {
  with_state(|state| {
    state
      .focus
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);
  });
}

/// Registers `handler` to run when `node` loses focus, until the node is destroyed.
pub fn handle_blur(node: NodeId, handler: Rc<dyn Fn()>) {
  with_state(|state| {
    state
      .blur
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);
  });
}

/// Registers `handler` to run when a key is pressed on `node` or within it, until the
/// node is destroyed.
pub fn handle_key_down(node: NodeId, handler: Rc<dyn Fn(&KeyEvent)>) {
  with_state(|state| {
    state
      .key_down
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);
  });
}

/// Registers `handler` to run with the new value when the focused input within `node`
/// changes, until the node is destroyed.
pub fn handle_input(node: NodeId, handler: Rc<dyn Fn(&str)>) {
  with_state(|state| {
    state
      .input
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);
  });
}

/// Folds one changed property into a stored box style.
fn apply_box_prop(style: &mut BoxStyle, prop: &BoxProp) {
  match prop {
    BoxProp::Width(value) => style.width = Some(*value),
    BoxProp::Height(value) => style.height = Some(*value),
    BoxProp::MinWidth(value) => style.min_width = Some(*value),
    BoxProp::MinHeight(value) => style.min_height = Some(*value),
    BoxProp::MaxWidth(value) => style.max_width = Some(*value),
    BoxProp::MaxHeight(value) => style.max_height = Some(*value),
    BoxProp::Padding(value) => style.padding = Some(*value),
    BoxProp::Color(value) => style.color = Some(*value),
    BoxProp::Border(value) => style.border = Some(*value),
    BoxProp::BorderRadius(value) => style.border_radius = Some(*value),
    BoxProp::Cursor(value) => style.cursor = Some(*value),
    BoxProp::Spacing(value) => style.spacing = Some(*value),
    BoxProp::MainAxisAlignment(value) => style.main_axis_alignment = Some(*value),
    BoxProp::CrossAxisAlignment(value) => style.cross_axis_alignment = Some(*value),
  }
}
