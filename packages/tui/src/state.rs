//! The terminal driver's node tree, handlers, and focus.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHashSet};
use skene_core::{
  Axis,
  Color,
  Layout,
  Modifiers,
  MouseCursor,
  NodeId,
  Overflow,
  Placement,
  PointerButton,
  Prevent,
  Prevention,
};

use crate::{
  events::{PointerEvent, ScrollEvent},
  paint::field_window,
  props::{BoxStyle, Condition, ControlStyle, RunStyle},
  selection::Selection,
  style::{Environment, Interaction, effective_box_style, effective_control_style},
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
  /// A text run, wrapping at the available width.
  Run {
    style: RunStyle,
    variants: Vec<(Condition, RunStyle)>,
    text: String,
    selectable: bool,
  },
  /// A single-line text input; the caret is a character position in the value,
  /// `0..=len`.
  TextField {
    style: BoxStyle,
    variants: Vec<(Condition, BoxStyle)>,
    value: String,
    placeholder: Option<String>,
    caret: usize,
  },
  /// A toggleable checkbox: its checked or unchecked mark, then the label.
  Checkbox {
    style: ControlStyle,
    variants: Vec<(Condition, ControlStyle)>,
    marks: (String, String),
    label: Option<String>,
    checked: bool,
  },
  /// One choice in a radio group: its mark—`(x)` or `( )`—then the label.
  Radio {
    style: ControlStyle,
    variants: Vec<(Condition, ControlStyle)>,
    group: String,
    marks: (String, String),
    label: Option<String>,
    checked: bool,
  },
  /// A fixed-option selector, cycling in place.
  Picker {
    style: ControlStyle,
    variants: Vec<(Condition, ControlStyle)>,
    options: Vec<(String, String)>,
    indicator: char,
    value: String,
  },
  /// A numeric slider on a horizontal track.
  Slider {
    style: ControlStyle,
    variants: Vec<(Condition, ControlStyle)>,
    glyphs: (char, char),
    min: f32,
    max: f32,
    step: f32,
    value: f32,
  },
  /// A structural anchor with no visuals of its own.
  Group,
  /// An overlay: content above the app, opened and closed by the facade.
  Overlay {
    modal: bool,
    open: bool,
    placement: Placement,
    backdrop: Option<Color>,
    anchor: Option<NodeId>,
    style: BoxStyle,
    variants: Vec<(Condition, BoxStyle)>,
  },
}

impl NodeKind {
  /// The node's box styling, when it carries one.
  pub(crate) fn box_style(&self) -> Option<(&BoxStyle, &[(Condition, BoxStyle)])> {
    match self {
      NodeKind::Box {
        style, variants, ..
      }
      | NodeKind::TextField {
        style, variants, ..
      }
      | NodeKind::Overlay {
        style, variants, ..
      } => Some((style, variants)),
      NodeKind::Run { .. }
      | NodeKind::Group
      | NodeKind::Checkbox { .. }
      | NodeKind::Radio { .. }
      | NodeKind::Picker { .. }
      | NodeKind::Slider { .. } => None,
    }
  }

  /// The node's control styling, when it is a small control.
  pub(crate) fn control_style(&self) -> Option<(&ControlStyle, &[(Condition, ControlStyle)])> {
    match self {
      NodeKind::Checkbox {
        style, variants, ..
      }
      | NodeKind::Radio {
        style, variants, ..
      }
      | NodeKind::Picker {
        style, variants, ..
      }
      | NodeKind::Slider {
        style, variants, ..
      } => Some((style, variants)),
      _ => None,
    }
  }
}

/// A press-initiated drag in progress, followed by pointer motion until release.
#[derive(Clone, Copy)]
pub(crate) enum Drag {
  /// A slider's thumb: motion slides the value to the pointer's column.
  Slider(NodeId),
  /// A scrollbar's thumb: motion scrolls so the grabbed spot follows the pointer.
  Scrollbar {
    node: NodeId,
    axis: Axis,
    /// The grabbed position within the thumb, in track cells.
    grab: i64,
  },
  /// A text selection: motion extends it to the position under the pointer.
  Selection,
}

/// One node's record: its realization, its taffy node, and its place in the tree.
pub(crate) struct NodeRecord {
  pub(crate) kind: NodeKind,
  pub(crate) taffy: taffy::NodeId,
  pub(crate) children: Vec<NodeId>,
  pub(crate) parent: Option<NodeId>,
  pub(crate) in_stack: bool,
  /// Wrapper-view layout annotations (pinned edges, flexibility, grid placement) for
  /// a node whose kind carries no box style of its own.
  pub(crate) annotations: Option<Box<BoxStyle>>,
}

/// A node's handlers of one event kind.
pub(crate) type Handlers<F> = RefCell<FxHashMap<NodeId, Vec<Rc<F>>>>;

/// The terminal driver's state: the node tree, the taffy tree, handlers, and focus.
pub(crate) struct State {
  pub(crate) nodes: RefCell<FxHashMap<NodeId, NodeRecord>>,
  /// The taffy tree; a text run's leaf carries its text as context, wrapped by the
  /// layout's measure function.
  pub(crate) taffy: RefCell<taffy::TaffyTree<String>>,
  next_id: Cell<u64>,
  pub(crate) click: Handlers<skene_core::ClickHandler>,
  pub(crate) focus: Handlers<dyn Fn()>,
  pub(crate) blur: Handlers<dyn Fn()>,
  pub(crate) key_down: Handlers<skene_core::KeyHandler>,
  pub(crate) input: Handlers<dyn Fn(&str)>,
  pub(crate) pointer_enter: Handlers<dyn Fn()>,
  pub(crate) pointer_leave: Handlers<dyn Fn()>,
  pub(crate) pointer_down: Handlers<crate::events::PointerHandler>,
  pub(crate) pointer_up: Handlers<crate::events::PointerHandler>,
  pub(crate) pointer_move: Handlers<crate::events::PointerHandler>,
  pub(crate) scroll: Handlers<dyn Fn(&ScrollEvent)>,
  pub(crate) toggle: Handlers<dyn Fn(bool)>,
  pub(crate) change_value: Handlers<dyn Fn(f32)>,
  pub(crate) dismiss: Handlers<dyn Fn()>,
  /// Every overlay node, in build order—later entries stack on top.
  pub(crate) overlays: RefCell<Vec<NodeId>>,
  pub(crate) focused: Cell<Option<NodeId>>,
  /// The nodes registered as focusable controls (buttons, and controls to come).
  pub(crate) controls: RefCell<FxHashSet<NodeId>>,
  /// The path from the root to the deepest node under the mouse, when known.
  pub(crate) hovered: RefCell<Vec<NodeId>>,
  /// The drag in progress, from a press until its release.
  pub(crate) drag: Cell<Option<Drag>>,
  /// The text selection, when one exists.
  pub(crate) selection: Cell<Option<Selection>>,
  /// The copy menu's pressed cell, while the menu is open.
  pub(crate) menu: Cell<Option<(u16, u16)>>,
  /// The last press, for counting multi-click sequences: its time, cell, and count.
  pub(crate) last_press: Cell<Option<(std::time::Instant, u16, u16, u8)>>,
  /// The pointer's last known cell, for painting the menu's hover state.
  pub(crate) last_pointer: Cell<(u16, u16)>,
  /// Text staged for the terminal loop's clipboard write.
  pub(crate) pending_copy: RefCell<Option<String>>,
  /// The size of the last computed layout, which mouse hit-testing resolves against.
  pub(crate) last_size: Cell<(u16, u16)>,
  /// Whether the terminal background is dark, detected at launch.
  pub(crate) dark: Cell<bool>,
  /// Each scrolled node's offsets, in cells.
  pub(crate) scroll_offsets: RefCell<FxHashMap<NodeId, (u32, u32)>>,
  /// The node to scroll into view once the next layout is computed.
  pub(crate) reveal: Cell<Option<NodeId>>,
  /// Each watched node's viewport callback and last reported content size, run after
  /// layout when that size changes.
  #[expect(clippy::type_complexity)]
  pub(crate) viewport_watchers: RefCell<FxHashMap<NodeId, (Rc<dyn Fn(u32, u32)>, (u32, u32))>>,
  /// The terminal title, emitted via OSC 2 when it changes.
  pub(crate) title: RefCell<Option<String>>,
  /// Whether the title changed since it was last emitted.
  pub(crate) title_dirty: Cell<bool>,
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
        annotations: None,
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
      pointer_down: RefCell::new(FxHashMap::default()),
      pointer_up: RefCell::new(FxHashMap::default()),
      pointer_move: RefCell::new(FxHashMap::default()),
      scroll: RefCell::new(FxHashMap::default()),
      toggle: RefCell::new(FxHashMap::default()),
      change_value: RefCell::new(FxHashMap::default()),
      dismiss: RefCell::new(FxHashMap::default()),
      overlays: RefCell::new(Vec::new()),
      focused: Cell::new(None),
      controls: RefCell::new(FxHashSet::default()),
      hovered: RefCell::new(Vec::new()),
      drag: Cell::new(None),
      selection: Cell::new(None),
      menu: Cell::new(None),
      last_press: Cell::new(None),
      last_pointer: Cell::new((0, 0)),
      pending_copy: RefCell::new(None),
      last_size: Cell::new((80, 24)),
      dark: Cell::new(true),
      scroll_offsets: RefCell::new(FxHashMap::default()),
      reveal: Cell::new(None),
      viewport_watchers: RefCell::new(FxHashMap::default()),
      title: RefCell::new(None),
      title_dirty: Cell::new(false),
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
        annotations: None,
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

    let overlay = matches!(
      nodes.get(&child).map(|record| &record.kind),
      Some(NodeKind::Overlay { .. })
    );
    let root_taffy = nodes.get(&ROOT_ID).expect("the root").taffy;
    let child_taffy = {
      let record = nodes.get_mut(&child).expect("tui driver: unknown node id");
      let old = record.parent.take();

      record.parent = Some(parent);

      let _ = old;

      record.taffy
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

    if let Some(current) = taffy.parent(child_taffy) {
      let _ = taffy.remove_child(current, child_taffy);
    }

    // An overlay leaves normal flow entirely: its layout hangs off the root, its tree
    // place stays with its parent.
    if overlay {
      taffy
        .add_child(root_taffy, child_taffy)
        .expect("attach an overlay's taffy node");
    }
    else {
      taffy
        .insert_child_at_index(parent_taffy, index, child_taffy)
        .expect("attach a taffy child");
    }

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

    if let Some(child_taffy) = child_taffy {
      let _ = parent_taffy;
      let mut taffy = self.taffy.borrow_mut();

      if let Some(current) = taffy.parent(child_taffy) {
        let _ = taffy.remove_child(current, child_taffy);
      }
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
      self.pointer_down.borrow_mut().remove(&current);
      self.pointer_up.borrow_mut().remove(&current);
      self.pointer_move.borrow_mut().remove(&current);
      self.scroll.borrow_mut().remove(&current);
      self.toggle.borrow_mut().remove(&current);
      self.change_value.borrow_mut().remove(&current);
      self.dismiss.borrow_mut().remove(&current);
      self.overlays.borrow_mut().retain(|node| *node != current);
      self.hovered.borrow_mut().retain(|node| *node != current);
      self.controls.borrow_mut().remove(&current);
      self.scroll_offsets.borrow_mut().remove(&current);
      self.viewport_watchers.borrow_mut().remove(&current);

      if self.reveal.get() == Some(current) {
        self.reveal.set(None);
      }

      if self.focused.get() == Some(current) {
        self.focused.set(None);
      }

      self.invalidate_selection(current);
    }

    self.dirty.set(true);
  }

  /// Runs `apply` on a node's record.
  pub(crate) fn with_node<R>(&self, node: NodeId, apply: impl FnOnce(&mut NodeRecord) -> R) -> R {
    let mut nodes = self.nodes.borrow_mut();
    let record = nodes.get_mut(&node).expect("tui driver: unknown node id");

    apply(record)
  }

  /// The focusable nodes—inputs and registered controls; never plain boxes—in tree
  /// order.
  pub(crate) fn focusable(&self) -> Vec<NodeId> {
    let nodes = self.nodes.borrow();
    let controls = self.controls.borrow();
    let mut order = Vec::new();
    let mut stack = vec![ROOT_ID];

    while let Some(current) = stack.pop() {
      if let Some(record) = nodes.get(&current) {
        if matches!(record.kind, NodeKind::TextField { .. }) || controls.contains(&current) {
          order.push(current);
        }

        // Reverse, so the stack pops children in tree order.
        stack.extend(record.children.iter().rev());
      }
    }

    drop(nodes);

    // An open modal contains the tab order.
    if let Some((top, true)) = self.topmost_open_overlay() {
      order.retain(|node| self.ancestors(*node).contains(&top));
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

  /// Runs `node`'s handlers, folding their verdicts; every handler on the node runs.
  pub(crate) fn run_preventable<F: ?Sized, P: Copy>(
    handlers: &Handlers<F>,
    node: NodeId,
    payload: P,
    invoke: impl Fn(&F, P) -> Prevent,
  ) -> Prevention {
    let mut prevention = Prevention::default();

    let Some(to_run) = handlers.borrow().get(&node).cloned()
    else {
      return prevention;
    };

    for handler in to_run {
      prevention.absorb(invoke(&handler, payload));
    }

    prevention
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

  /// Sets the focused node, firing blur and focus handlers on change; the newly focused
  /// node is scrolled into view at the next render, and a newly focused input's caret
  /// moves to the value's end (a click re-places it afterwards).
  pub(crate) fn set_focused(&self, next: Option<NodeId>) {
    let previous = self.focused.replace(next);

    if previous == next {
      return;
    }

    if let Some(previous) = previous {
      Self::run_handlers(&self.blur, previous, (), |run, ()| run());
    }

    if let Some(next) = next {
      self.with_node(next, |record| {
        if let NodeKind::TextField { value, caret, .. } = &mut record.kind {
          *caret = value.chars().count();
        }
      });

      Self::run_handlers(&self.focus, next, (), |run, ()| run());

      self.reveal.set(Some(next));
    }

    self.dirty.set(true);
  }

  /// The path from the root to the deepest node containing the cell (`x`, `y`)—each
  /// with its absolute origin—against the last computed layout.
  pub(crate) fn hit_test(&self, x: u16, y: u16) -> Vec<(NodeId, (i64, i64))> {
    // Overlays hit first, topmost first; below an open modal nothing is hittable.
    for overlay in self.open_overlays().iter().rev() {
      let mut path = Vec::new();

      if self.hit_overlay(*overlay, x, y, &mut path) {
        return path;
      }

      if matches!(self.topmost_open_overlay(), Some((top, true)) if top == *overlay) {
        return Vec::new();
      }
    }

    let mut path = Vec::new();

    self.hit_walk(ROOT_ID, 0, 0, i64::from(x), i64::from(y), &mut path);

    path
  }

  /// Descends into the deepest child containing the point, preferring later (topmost)
  /// children, accumulating the path.
  fn hit_walk(
    &self,
    node: NodeId,
    offset_x: i64,
    offset_y: i64,
    x: i64,
    y: i64,
    path: &mut Vec<(NodeId, (i64, i64))>,
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
        offset_x + layout.location.x.round() as i64,
        offset_y + layout.location.y.round() as i64,
      );

      #[expect(clippy::cast_possible_truncation)]
      let (width, height) = (
        layout.size.width.round().max(0.0) as i64,
        layout.size.height.round().max(0.0) as i64,
      );

      (origin_x, origin_y, width, height, record.children.clone())
    };

    let contains = x >= origin_x && x < origin_x + width && y >= origin_y && y < origin_y + height;

    if node != ROOT_ID && !contains {
      return false;
    }

    path.push((node, (origin_x, origin_y)));

    // A scrolled box shifts its children by its offsets.
    let (scroll_x, scroll_y) = self.scroll_offset(node);
    let (child_x, child_y) = (
      origin_x - i64::from(scroll_x),
      origin_y - i64::from(scroll_y),
    );

    // Later children paint on top, so they win the hit; overlays hit separately.
    for child in children.iter().rev() {
      if self.is_overlay(*child) {
        continue;
      }

      if self.hit_walk(*child, child_x, child_y, x, y, path) {
        break;
      }
    }

    true
  }

  /// Updates the hover path to the nodes under (`x`, `y`), firing pointer-leave and
  /// pointer-enter handlers for the difference.
  pub(crate) fn set_hover(&self, x: u16, y: u16) {
    let next: Vec<NodeId> = self.hit_test(x, y).iter().map(|(node, _)| *node).collect();
    let previous = self.hovered.borrow().clone();

    if self.last_pointer.replace((x, y)) != (x, y) && self.menu.get().is_some() {
      // The menu's hover state follows the pointer.
      self.dirty.set(true);
    }

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
  /// hit path (or blurs when none is hit), runs click handlers from the deepest node
  /// up, then—unless a handler prevented the default—activates the focused control.
  pub(crate) fn mouse_click(&self, x: u16, y: u16) {
    // An open copy menu consumes the press: `Copy` copies, anywhere else just closes.
    if self.menu.get().is_some() {
      if self.copy_hit(x, y) {
        self.copy_selection();
      }

      self.close_menu();

      return;
    }

    // A press on a scrollbar grabs it—jumping the thumb when pressed off it—and
    // consumes the press: no focus change, no click dispatch.
    if let Some((node, axis, position)) = self.scrollbar_hit(x, y) {
      self.grab_scrollbar(node, axis, position);

      return;
    }

    self.clear_selection();

    if let Some((top, modal)) = self.topmost_open_overlay() {
      let mut path = Vec::new();

      if !self.hit_overlay(top, x, y, &mut path) {
        Self::run_handlers(&self.dismiss, top, (), |run, ()| run());

        if modal {
          return;
        }
      }
    }

    let path = self.hit_test(x, y);
    let focusable = self.focusable();
    let target = path
      .iter()
      .rev()
      .find(|(node, _)| focusable.contains(node))
      .map(|(node, _)| *node);

    self.set_focused(target);

    let mut prevention = Prevention::default();

    for (node, _) in path.iter().rev() {
      prevention.merge(Self::run_preventable(&self.click, *node, (), |run, ()| {
        run()
      }));

      if prevention.bubbling {
        break;
      }
    }

    if prevention.default {
      return;
    }

    if let Some(target) = target {
      let origin = path
        .iter()
        .find(|(node, _)| *node == target)
        .map_or((0, 0), |(_, origin)| *origin);

      self.activate_control(target, Some(i64::from(x) - origin.0));

      // A press on a slider starts a drag: motion keeps sliding until release.
      if matches!(
        self.nodes.borrow().get(&target).map(|record| &record.kind),
        Some(NodeKind::Slider { .. })
      ) {
        self.drag.set(Some(Drag::Slider(target)));
      }
    }

    // A press on a selectable run starts a selection: a drag extends it, a double
    // click takes the word, a triple click the whole run.
    if let Some(point) = self.text_position_at(x, y) {
      match self.register_press(x, y) {
        2 => self.select_word(point),
        3 => self.select_run(point),
        _ => self.anchor_selection(point),
      }
    }
    else {
      self.last_press.set(None);
    }
  }

  /// Follows the drag in progress to the pointer at (`x`, `y`), if one is under way.
  pub(crate) fn drag_to(&self, x: u16, y: u16) {
    match self.drag.get() {
      Some(Drag::Slider(node)) => {
        let (origin_x, _) = self.absolute_origin(node);

        self.slide_to(node, i64::from(x) - origin_x);
      }
      Some(Drag::Scrollbar { node, axis, grab }) => {
        let Some((track_start, track, limit)) = self.scroll_geometry(node, axis)
        else {
          return;
        };

        let along = match axis {
          Axis::Horizontal => i64::from(x),
          Axis::Vertical => i64::from(y),
        };

        self.scroll_thumb_to(node, axis, track, limit, along - track_start - grab);
      }
      Some(Drag::Selection) => self.extend_selection(x, y),
      None => {}
    }
  }

  /// Ends the drag in progress, on release.
  pub(crate) fn end_drag(&self) {
    self.drag.set(None);
  }

  /// The scrollbar under (`x`, `y`), if any: the owning scrollable, its axis, and the
  /// pointer's position within the track.
  fn scrollbar_hit(&self, x: u16, y: u16) -> Option<(NodeId, Axis, i64)> {
    let ids: Vec<NodeId> = self.nodes.borrow().keys().copied().collect();
    let (px, py) = (i64::from(x), i64::from(y));

    for node in ids {
      if !self.is_scrollable(node) {
        continue;
      }

      let (limit_x, limit_y) = self.scroll_limits(node);
      let (origin_x, origin_y) = self.absolute_origin(node);
      let (width, height) = self.node_size(node);

      if limit_y > 0
        && px == origin_x + width - 1
        && let Some((track_start, track, _)) = self.scroll_geometry(node, Axis::Vertical)
        && py >= track_start
        && py < track_start + track
      {
        return Some((node, Axis::Vertical, py - track_start));
      }

      if limit_x > 0
        && py == origin_y + height - 1
        && let Some((track_start, track, _)) = self.scroll_geometry(node, Axis::Horizontal)
        && px >= track_start
        && px < track_start + track
      {
        return Some((node, Axis::Horizontal, px - track_start));
      }
    }

    None
  }

  /// A scrollable's track along `axis`: its absolute start, its length in cells, and
  /// the scroll limit.
  fn scroll_geometry(&self, node: NodeId, axis: Axis) -> Option<(i64, i64, u32)> {
    let (limit_x, limit_y) = self.scroll_limits(node);
    let (origin_x, origin_y) = self.absolute_origin(node);
    let (width, height) = self.node_size(node);
    let border = {
      let nodes = self.nodes.borrow();
      let record = nodes.get(&node)?;

      match self.taffy.borrow().layout(record.taffy) {
        #[expect(clippy::cast_possible_truncation)]
        Ok(layout) => (
          layout.border.left.round() as i64,
          layout.border.top.round() as i64,
          layout.border.right.round() as i64,
          layout.border.bottom.round() as i64,
        ),
        Err(_) => (0, 0, 0, 0),
      }
    };

    match axis {
      Axis::Vertical => Some((origin_y + border.1, height - border.1 - border.3, limit_y)),
      Axis::Horizontal => Some((origin_x + border.0, width - border.0 - border.2, limit_x)),
    }
  }

  /// A press on a scrollbar: grabs the thumb where it was pressed—or jumps it to the
  /// press and grabs its middle—then drags until release.
  fn grab_scrollbar(&self, node: NodeId, axis: Axis, position: i64) {
    let Some((_, track, limit)) = self.scroll_geometry(node, axis)
    else {
      return;
    };

    let offset = match axis {
      Axis::Horizontal => self.scroll_offset(node).0,
      Axis::Vertical => self.scroll_offset(node).1,
    };
    let (start, thumb) =
      crate::paint::thumb_span(isize::try_from(track).unwrap_or(0), offset, limit);
    let (start, thumb) = (start as i64, thumb as i64);
    let grab = if position >= start && position < start + thumb {
      position - start
    }
    else {
      thumb / 2
    };

    self.scroll_thumb_to(node, axis, track, limit, position - grab);
    self.drag.set(Some(Drag::Scrollbar { node, axis, grab }));
  }

  /// Scrolls so the thumb's start lands on `target_start` within its track, clamped.
  fn scroll_thumb_to(&self, node: NodeId, axis: Axis, track: i64, limit: u32, target_start: i64) {
    let (_, thumb) = crate::paint::thumb_span(isize::try_from(track).unwrap_or(0), 0, limit);
    let thumb = thumb as i64;
    let room = (track - thumb).max(1);
    let target_start = target_start.clamp(0, track - thumb);
    let offset = target_start * i64::from(limit) / room;
    let current = match axis {
      Axis::Horizontal => i64::from(self.scroll_offset(node).0),
      Axis::Vertical => i64::from(self.scroll_offset(node).1),
    };
    let delta = offset - current;

    let _ = match axis {
      Axis::Horizontal => self.scroll_by(node, delta, 0),
      Axis::Vertical => self.scroll_by(node, 0, delta),
    };
  }

  /// Dispatches an activation on `target` as `Enter`/`Space` do: click handlers bubble
  /// from it, then—unless prevented—the control's own activation.
  pub(crate) fn activate_dispatch(&self, target: NodeId) {
    let mut prevention = Prevention::default();

    for node in self.ancestors(target) {
      prevention.merge(Self::run_preventable(&self.click, node, (), |run, ()| {
        run()
      }));

      if prevention.bubbling {
        break;
      }
    }

    if !prevention.default {
      self.activate_control(target, None);
    }
  }

  /// Activates a control the way `Enter`, `Space`, or a click does: toggles a
  /// checkbox, checks a radio, cycles a select, and—given a click's local column—jumps
  /// a slider's thumb or places an input's caret there.
  pub(crate) fn activate_control(&self, node: NodeId, local_column: Option<i64>) {
    enum Activation {
      Toggle,
      Check,
      Cycle,
      Slide,
      Place,
    }

    let activation = {
      let nodes = self.nodes.borrow();

      match nodes.get(&node).map(|record| &record.kind) {
        Some(NodeKind::Checkbox { .. }) => Activation::Toggle,
        Some(NodeKind::Radio { .. }) => Activation::Check,
        Some(NodeKind::Picker { .. }) => Activation::Cycle,
        Some(NodeKind::Slider { .. }) => Activation::Slide,
        Some(NodeKind::TextField { .. }) => Activation::Place,
        _ => return,
      }
    };

    match activation {
      Activation::Toggle => self.toggle_checkbox(node),
      Activation::Check => self.check_radio(node),
      Activation::Cycle => self.cycle_picker(node, 1),
      Activation::Slide => {
        if let Some(local_column) = local_column {
          self.slide_to(node, local_column);
        }
      }
      Activation::Place => {
        if let Some(local_column) = local_column {
          self.place_caret(node, local_column);
        }
      }
    }
  }

  /// Places an input's caret at a click's local column, mapped through the border
  /// inset and the value's visible window.
  fn place_caret(&self, node: NodeId, local_column: i64) {
    let (columns, rows) = self.last_size.get();
    let environment = Environment {
      columns,
      rows,
      dark: self.dark.get(),
    };
    let interaction = Interaction {
      focused: self.focused.get() == Some(node),
      hovered: self.hovered.borrow().contains(&node),
    };

    let placed = self.with_node(node, |record| {
      let NodeKind::TextField {
        style,
        variants,
        value,
        caret,
        ..
      } = &mut record.kind
      else {
        return false;
      };

      #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      let width = self
        .taffy
        .borrow()
        .layout(record.taffy)
        .map_or(0, |layout| layout.size.width.round().max(0.0) as usize);
      let effective = effective_box_style(style, variants, environment, interaction);
      let inset = usize::from(effective.border.is_some());
      let inner = width.saturating_sub(2 * inset);
      let window = field_window(*caret, inner);
      let column = usize::try_from(local_column)
        .unwrap_or(0)
        .saturating_sub(inset);

      *caret = (window + column).min(value.chars().count());

      true
    });

    if placed {
      self.dirty.set(true);
    }
  }

  /// Flips a checkbox, firing its toggle handlers with the new state.
  fn toggle_checkbox(&self, node: NodeId) {
    let next = self.with_node(node, |record| {
      if let NodeKind::Checkbox { checked, .. } = &mut record.kind {
        *checked = !*checked;

        Some(*checked)
      }
      else {
        None
      }
    });

    if let Some(next) = next {
      self.dirty.set(true);

      Self::run_handlers(&self.toggle, node, next, |run, checked| run(checked));
    }
  }

  /// Checks a radio, unchecking its group, firing toggle handlers for every change.
  pub(crate) fn check_radio(&self, node: NodeId) {
    let group = {
      let nodes = self.nodes.borrow();

      match nodes.get(&node).map(|record| &record.kind) {
        Some(NodeKind::Radio { group, .. }) => group.clone(),
        _ => return,
      }
    };

    let mut flipped = Vec::new();

    {
      let mut nodes = self.nodes.borrow_mut();

      for (id, record) in nodes.iter_mut() {
        if let NodeKind::Radio {
          group: this_group,
          checked,
          ..
        } = &mut record.kind
          && *this_group == group
        {
          let next = *id == node;

          if *checked != next {
            *checked = next;

            flipped.push((*id, next));
          }
        }
      }
    }

    if flipped.is_empty() {
      return;
    }

    self.dirty.set(true);

    for (id, checked) in flipped {
      Self::run_handlers(&self.toggle, id, checked, |run, checked| run(checked));
    }
  }

  /// The radios of `node`'s group, in tree order, with `node`'s position among them.
  pub(crate) fn radio_group(&self, node: NodeId) -> Option<(Vec<NodeId>, usize)> {
    let group = {
      let nodes = self.nodes.borrow();

      match nodes.get(&node).map(|record| &record.kind) {
        Some(NodeKind::Radio { group, .. }) => group.clone(),
        _ => return None,
      }
    };

    let members: Vec<NodeId> = {
      let nodes = self.nodes.borrow();

      self
        .focusable()
        .into_iter()
        .filter(|id| {
          matches!(
            nodes.get(id).map(|record| &record.kind),
            Some(NodeKind::Radio { group: this_group, .. }) if *this_group == group
          )
        })
        .collect()
    };

    let position = members.iter().position(|id| *id == node)?;

    Some((members, position))
  }

  /// Cycles a select by `step` options, wrapping, firing its change handlers.
  pub(crate) fn cycle_picker(&self, node: NodeId, step: isize) {
    let next = self.with_node(node, |record| {
      let NodeKind::Picker { options, value, .. } = &mut record.kind
      else {
        return None;
      };

      if options.is_empty() {
        return None;
      }

      let count = isize::try_from(options.len()).ok()?;
      let index = options
        .iter()
        .position(|(option, _)| option == value)
        .and_then(|index| isize::try_from(index).ok())
        .unwrap_or(0);
      let next = usize::try_from((index + step).rem_euclid(count)).ok()?;
      let next = options[next].0.clone();

      if next == *value {
        return None;
      }

      value.clone_from(&next);

      Some(next)
    });

    if let Some(next) = next {
      self.dirty.set(true);

      Self::run_handlers(&self.input, node, next.as_str(), |run, value| run(value));
    }
  }

  /// Selects a picker's option by value, firing its change handlers on a change.
  pub(crate) fn select_picker(&self, node: NodeId, value: &str) {
    let next = self.with_node(node, |record| {
      let NodeKind::Picker {
        options,
        value: current,
        ..
      } = &mut record.kind
      else {
        return None;
      };

      if current == value || !options.iter().any(|(option, _)| option == value) {
        return None;
      }

      value.clone_into(current);

      Some(current.clone())
    });

    if let Some(next) = next {
      self.dirty.set(true);

      Self::run_handlers(&self.input, node, next.as_str(), |run, value| run(value));
    }
  }

  /// Moves a slider by `steps` of its step size, clamped, firing its change handlers.
  pub(crate) fn nudge_slider(&self, node: NodeId, steps: f32) {
    self.write_slider(node, |min, max, step, value| {
      (value + step * steps).clamp(min, max)
    });
  }

  /// Jumps a slider's thumb to a clicked cell of its track, snapped to its step.
  fn slide_to(&self, node: NodeId, local_column: i64) {
    let (width, _) = self.node_size(node);

    self.write_slider(node, |min, max, step, _| {
      #[expect(clippy::cast_precision_loss)]
      let fraction = if width > 1 {
        (local_column.clamp(0, width - 1) as f32) / ((width - 1) as f32)
      }
      else {
        0.0
      };
      let value = min + fraction * (max - min);

      if step > 0.0 {
        (min + ((value - min) / step).round() * step).clamp(min, max)
      }
      else {
        value.clamp(min, max)
      }
    });
  }

  /// Rewrites a slider's value through `next`, firing change handlers on change.
  fn write_slider(&self, node: NodeId, next: impl FnOnce(f32, f32, f32, f32) -> f32) {
    let changed = self.with_node(node, |record| {
      let NodeKind::Slider {
        min,
        max,
        step,
        value,
        ..
      } = &mut record.kind
      else {
        return None;
      };

      let target = next(*min, *max, *step, *value);

      if (target - *value).abs() < f32::EPSILON {
        return None;
      }

      *value = target;

      Some(target)
    });

    if let Some(value) = changed {
      self.dirty.set(true);

      Self::run_handlers(&self.change_value, node, value, |run, value| run(value));
    }
  }

  /// Runs one pointer registry's handlers along the hit path at (`x`, `y`), deepest
  /// first, each receiving its own view-relative coordinates; the folded verdicts come
  /// back so the caller can honor them.
  pub(crate) fn run_pointer(
    &self,
    handlers: &Handlers<crate::events::PointerHandler>,
    x: u16,
    y: u16,
    button: Option<PointerButton>,
    modifiers: Modifiers,
  ) -> Prevention {
    let mut prevention = Prevention::default();

    for (node, (origin_x, origin_y)) in self.hit_test(x, y).iter().rev() {
      let event = PointerEvent {
        column: u32::from(x),
        row: u32::from(y),
        local_column: u32::try_from(i64::from(x) - origin_x).unwrap_or(0),
        local_row: u32::try_from(i64::from(y) - origin_y).unwrap_or(0),
        button,
        modifiers,
      };

      prevention.merge(Self::run_preventable(
        handlers,
        *node,
        &event,
        |run, event| run(event),
      ));

      if prevention.bubbling {
        break;
      }
    }

    prevention
  }

  /// Opens or closes an overlay, resolving its anchor at that moment.
  pub(crate) fn set_overlay_open(&self, node: NodeId, open: bool, anchor: Option<NodeId>) {
    self.with_node(node, |record| {
      if let NodeKind::Overlay {
        open: current,
        anchor: bound,
        ..
      } = &mut record.kind
      {
        *current = open;
        *bound = anchor;
      }
    });

    self.dirty.set(true);
  }

  /// Whether `node` is an overlay.
  pub(crate) fn is_overlay(&self, node: NodeId) -> bool {
    matches!(
      self.nodes.borrow().get(&node).map(|record| &record.kind),
      Some(NodeKind::Overlay { .. })
    )
  }

  /// Every open overlay, in build order—later entries stack on top.
  pub(crate) fn open_overlays(&self) -> Vec<NodeId> {
    let nodes = self.nodes.borrow();

    self
      .overlays
      .borrow()
      .iter()
      .copied()
      .filter(|node| {
        matches!(
          nodes.get(node).map(|record| &record.kind),
          Some(NodeKind::Overlay { open: true, .. })
        )
      })
      .collect()
  }

  /// The topmost open overlay and whether it is modal.
  pub(crate) fn topmost_open_overlay(&self) -> Option<(NodeId, bool)> {
    let nodes = self.nodes.borrow();

    self.overlays.borrow().iter().rev().find_map(|node| {
      match nodes.get(node).map(|record| &record.kind) {
        Some(NodeKind::Overlay {
          open: true, modal, ..
        }) => Some((*node, *modal)),
        _ => None,
      }
    })
  }

  /// A node's absolute origin: its taffy locations summed up the parent chain, each
  /// ancestor's scroll offsets subtracted.
  fn absolute_origin(&self, node: NodeId) -> (i64, i64) {
    let nodes = self.nodes.borrow();
    let taffy = self.taffy.borrow();
    let mut current = Some(node);
    let (mut x, mut y) = (0, 0);

    while let Some(id) = current {
      let Some(record) = nodes.get(&id)
      else {
        break;
      };

      if let Ok(layout) = taffy.layout(record.taffy) {
        #[expect(clippy::cast_possible_truncation)]
        {
          x += layout.location.x.round() as i64;
          y += layout.location.y.round() as i64;
        }
      }

      if let Some(parent) = record.parent {
        let (scroll_x, scroll_y) = self.scroll_offset(parent);

        x -= i64::from(scroll_x);
        y -= i64::from(scroll_y);
      }

      current = record.parent;
    }

    (x, y)
  }

  /// A node's on-screen origin: like [`absolute_origin`](Self::absolute_origin), but
  /// content inside an overlay positions against the overlay's computed origin.
  pub(crate) fn screen_origin(&self, node: NodeId) -> (i64, i64) {
    let (mut x, mut y) = (0, 0);
    let mut current = Some(node);

    while let Some(id) = current {
      if self.is_overlay(id) {
        let (overlay_x, overlay_y) = self.overlay_origin(id);

        return (x + overlay_x, y + overlay_y);
      }

      let (location, parent) = {
        let nodes = self.nodes.borrow();
        let Some(record) = nodes.get(&id)
        else {
          break;
        };

        #[expect(clippy::cast_possible_truncation)]
        let location = match self.taffy.borrow().layout(record.taffy) {
          Ok(layout) => (
            layout.location.x.round() as i64,
            layout.location.y.round() as i64,
          ),
          Err(_) => (0, 0),
        };

        (location, record.parent)
      };

      x += location.0;
      y += location.1;

      if let Some(parent) = parent {
        let (scroll_x, scroll_y) = self.scroll_offset(parent);

        x -= i64::from(scroll_x);
        y -= i64::from(scroll_y);
      }

      current = parent;
    }

    (x, y)
  }

  /// An open overlay's absolute origin: anchored `Top`/`Bottom` position against the
  /// anchor, everything else against the terminal.
  pub(crate) fn overlay_origin(&self, node: NodeId) -> (i64, i64) {
    let (columns, rows) = self.last_size.get();
    let (columns, rows) = (i64::from(columns), i64::from(rows));
    let (placement, anchor) = {
      let nodes = self.nodes.borrow();

      match nodes.get(&node).map(|record| &record.kind) {
        Some(NodeKind::Overlay {
          placement, anchor, ..
        }) => (*placement, *anchor),
        _ => return (0, 0),
      }
    };
    let (width, height) = self.node_size(node);

    if let Some(anchor) = anchor.filter(|_| matches!(placement, Placement::Top | Placement::Bottom))
    {
      let (anchor_x, anchor_y) = self.absolute_origin(anchor);
      let (_, anchor_height) = self.node_size(anchor);

      return match placement {
        Placement::Top => (anchor_x, anchor_y - height),
        _ => (anchor_x, anchor_y + anchor_height),
      };
    }

    match placement {
      Placement::Center => ((columns - width) / 2, (rows - height) / 2),
      Placement::Top => ((columns - width) / 2, 0),
      Placement::TopStart => (0, 0),
      Placement::TopEnd => (columns - width, 0),
      Placement::Bottom => ((columns - width) / 2, rows - height),
      Placement::BottomStart => (0, rows - height),
      Placement::BottomEnd => (columns - width, rows - height),
    }
  }

  /// A node's laid-out size, in cells.
  pub(crate) fn node_size(&self, node: NodeId) -> (i64, i64) {
    let nodes = self.nodes.borrow();
    let Some(record) = nodes.get(&node)
    else {
      return (0, 0);
    };

    match self.taffy.borrow().layout(record.taffy) {
      #[expect(clippy::cast_possible_truncation)]
      Ok(layout) => (
        layout.size.width.round().max(0.0) as i64,
        layout.size.height.round().max(0.0) as i64,
      ),
      Err(_) => (0, 0),
    }
  }

  /// Hits `overlay`'s subtree at its computed origin, accumulating the path.
  pub(crate) fn hit_overlay(
    &self,
    overlay: NodeId,
    x: u16,
    y: u16,
    path: &mut Vec<(NodeId, (i64, i64))>,
  ) -> bool {
    let (origin_x, origin_y) = self.overlay_origin(overlay);
    let (location_x, location_y) = {
      let nodes = self.nodes.borrow();
      let Some(record) = nodes.get(&overlay)
      else {
        return false;
      };

      match self.taffy.borrow().layout(record.taffy) {
        #[expect(clippy::cast_possible_truncation)]
        Ok(layout) => (
          layout.location.x.round() as i64,
          layout.location.y.round() as i64,
        ),
        Err(_) => (0, 0),
      }
    };

    self.hit_walk(
      overlay,
      origin_x - location_x,
      origin_y - location_y,
      i64::from(x),
      i64::from(y),
      path,
    )
  }

  /// The current environment conditions resolve against.
  pub(crate) fn environment(&self) -> Environment {
    let (columns, rows) = self.last_size.get();

    Environment {
      columns,
      rows,
      dark: self.dark.get(),
    }
  }

  /// The pointer shape for the current hover path: the deepest hovered node with a
  /// cursor set, resolved through its effective style.
  pub(crate) fn hover_cursor(&self) -> Option<MouseCursor> {
    let environment = self.environment();
    let hovered = self.hovered.borrow();
    let nodes = self.nodes.borrow();

    hovered.iter().rev().find_map(|node| {
      let record = nodes.get(node)?;
      let interaction = Interaction {
        focused: self.focused.get() == Some(*node),
        hovered: true,
      };

      if let Some((style, variants)) = record.kind.box_style() {
        return effective_box_style(style, variants, environment, interaction).cursor;
      }

      if let NodeKind::Run {
        text,
        selectable: true,
        ..
      } = &record.kind
        && !text.is_empty()
      {
        return Some(MouseCursor::Text);
      }

      let (style, variants) = record.kind.control_style()?;

      effective_control_style(style, variants, environment, interaction).cursor
    })
  }

  /// A node's effective overflow, `Visible` when unset or the node has no box style.
  fn effective_overflow(&self, node: NodeId) -> Overflow {
    let nodes = self.nodes.borrow();
    let Some(record) = nodes.get(&node)
    else {
      return Overflow::Visible;
    };

    let Some((style, variants)) = record.kind.box_style()
    else {
      return Overflow::Visible;
    };

    let interaction = Interaction {
      focused: self.focused.get() == Some(node),
      hovered: self.hovered.borrow().contains(&node),
    };

    effective_box_style(style, variants, self.environment(), interaction)
      .overflow
      .unwrap_or(Overflow::Visible)
  }

  /// Whether the node's content scrolls.
  pub(crate) fn is_scrollable(&self, node: NodeId) -> bool {
    matches!(
      self.effective_overflow(node),
      Overflow::Scroll | Overflow::Auto
    )
  }

  /// Whether the node clips its children when painting.
  pub(crate) fn clips(&self, node: NodeId) -> bool {
    !matches!(self.effective_overflow(node), Overflow::Visible)
  }

  /// A node's scroll offsets, clamped to what the last layout allows.
  pub(crate) fn scroll_offset(&self, node: NodeId) -> (u32, u32) {
    let Some((x, y)) = self.scroll_offsets.borrow().get(&node).copied()
    else {
      return (0, 0);
    };

    let (limit_x, limit_y) = self.scroll_limits(node);

    (x.min(limit_x), y.min(limit_y))
  }

  /// A node's maximum scroll offsets, from the last computed layout.
  pub(crate) fn scroll_limits(&self, node: NodeId) -> (u32, u32) {
    let nodes = self.nodes.borrow();
    let Some(record) = nodes.get(&node)
    else {
      return (0, 0);
    };

    match self.taffy.borrow().layout(record.taffy) {
      #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      Ok(layout) => (
        layout.scroll_width().round().max(0.0) as u32,
        layout.scroll_height().round().max(0.0) as u32,
      ),
      Err(_) => (0, 0),
    }
  }

  /// Scrolls the node by a delta, clamped to its limits, firing its scroll handlers;
  /// `false` when the node does not scroll or cannot move further.
  pub(crate) fn scroll_by(&self, node: NodeId, delta_x: i64, delta_y: i64) -> bool {
    if !self.is_scrollable(node) {
      return false;
    }

    let (limit_x, limit_y) = self.scroll_limits(node);
    let (x, y) = self.scroll_offset(node);
    let next = (
      u32::try_from((i64::from(x) + delta_x).clamp(0, i64::from(limit_x))).expect("clamped"),
      u32::try_from((i64::from(y) + delta_y).clamp(0, i64::from(limit_y))).expect("clamped"),
    );

    if next == (x, y) {
      return false;
    }

    self.set_scroll(node, next);

    true
  }

  /// Every scrolled node's offsets, keyed by the node's build-order id and ordered by
  /// it—the dev loop's carry across a restart.
  #[cfg(feature = "snapshot")]
  pub(crate) fn scroll_snapshot(&self) -> Vec<(u64, u32, u32)> {
    let mut entries: Vec<_> = self
      .scroll_offsets
      .borrow()
      .iter()
      .filter(|(_, (x, y))| *x > 0 || *y > 0)
      .map(|(node, (x, y))| (node.value(), *x, *y))
      .collect();

    entries.sort_unstable();

    entries
  }

  /// Adopts offsets a prior run captured through [`scroll_snapshot`], by node id;
  /// entries no live node carries are dropped, and reads clamp to the next layout.
  #[cfg(feature = "snapshot")]
  pub(crate) fn restore_scroll(&self, entries: &[(u64, u32, u32)]) {
    let nodes = self.nodes.borrow();
    let mut offsets = self.scroll_offsets.borrow_mut();

    for (id, x, y) in entries {
      let node = NodeId::new(*id);

      if nodes.contains_key(&node) {
        offsets.insert(node, (*x, *y));
      }
    }

    self.dirty.set(true);
  }

  /// Stores a node's offsets, fires its scroll handlers, and repaints.
  fn set_scroll(&self, node: NodeId, (x, y): (u32, u32)) {
    self.scroll_offsets.borrow_mut().insert(node, (x, y));
    self.dirty.set(true);

    Self::run_handlers(
      &self.scroll,
      node,
      &ScrollEvent { column: x, row: y },
      |run, event| run(event),
    );
  }

  /// Scrolls the innermost box under the cell (`x`, `y`) that can still move in the
  /// given direction.
  pub(crate) fn mouse_scroll(&self, x: u16, y: u16, delta_x: i64, delta_y: i64) {
    self.close_menu();

    for (node, _) in self.hit_test(x, y).iter().rev() {
      if self.scroll_by(*node, delta_x, delta_y) {
        return;
      }
    }
  }

  /// Scrolls the nearest scrollable box around the focus (or the first in tree order)
  /// by `pages` viewport heights.
  pub(crate) fn page_scroll(&self, pages: i64) {
    let target = self
      .focused
      .get()
      .and_then(|focused| {
        self
          .ancestors(focused)
          .into_iter()
          .find(|node| self.is_scrollable(*node))
      })
      .or_else(|| self.first_scrollable());

    if let Some(node) = target {
      let (_, height) = self.node_size(node);
      let _ = self.scroll_by(node, 0, pages * height.max(1));
    }
  }

  /// The first scrollable node in tree order.
  fn first_scrollable(&self) -> Option<NodeId> {
    let order: Vec<NodeId> = {
      let nodes = self.nodes.borrow();
      let mut order = Vec::new();
      let mut stack = vec![ROOT_ID];

      while let Some(current) = stack.pop() {
        if let Some(record) = nodes.get(&current) {
          order.push(current);
          stack.extend(record.children.iter().rev());
        }
      }

      order
    };

    order.into_iter().find(|node| self.is_scrollable(*node))
  }

  /// Registers `watch` to run with the node's content size (inside its border) after
  /// each layout in which that size changed.
  pub(crate) fn watch_viewport(&self, node: NodeId, watch: Rc<dyn Fn(u32, u32)>) {
    self
      .viewport_watchers
      .borrow_mut()
      .insert(node, (watch, (0, 0)));
  }

  /// Runs the viewport watchers whose content size changed since they last ran; `true`
  /// when any did. Call once the layout is computed.
  pub(crate) fn notify_viewports(&self) -> bool {
    let mut pending = Vec::new();

    {
      let nodes = self.nodes.borrow();
      let taffy = self.taffy.borrow();
      let mut watchers = self.viewport_watchers.borrow_mut();

      for (node, (watch, last)) in watchers.iter_mut() {
        let Some(record) = nodes.get(node)
        else {
          continue;
        };

        let Ok(layout) = taffy.layout(record.taffy)
        else {
          continue;
        };

        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let size = (
          (layout.size.width - layout.border.left - layout.border.right)
            .round()
            .max(0.0) as u32,
          (layout.size.height - layout.border.top - layout.border.bottom)
            .round()
            .max(0.0) as u32,
        );

        if size != *last {
          *last = size;

          pending.push((Rc::clone(watch), size));
        }
      }
    }

    let changed = !pending.is_empty();

    for (watch, (width, height)) in pending {
      watch(width, height);
    }

    changed
  }

  /// Scrolls ancestors so the pending reveal target is visible, firing scroll handlers;
  /// `true` when any offset changed. Call once the layout is computed.
  pub(crate) fn resolve_reveal(&self) -> bool {
    let Some(node) = self.reveal.take()
    else {
      return false;
    };

    let mut adjusted = Vec::new();

    {
      let nodes = self.nodes.borrow();
      let taffy = self.taffy.borrow();
      let layout_of = |id: NodeId| {
        nodes
          .get(&id)
          .and_then(|record| taffy.layout(record.taffy).ok().copied())
      };
      let Some(layout) = layout_of(node)
      else {
        return false;
      };

      #[expect(clippy::cast_possible_truncation)]
      let (mut rel_x, mut rel_y) = (
        layout.location.x.round() as i64,
        layout.location.y.round() as i64,
      );

      #[expect(clippy::cast_possible_truncation)]
      let (width, height) = (
        layout.size.width.round().max(0.0) as i64,
        layout.size.height.round().max(0.0) as i64,
      );

      let mut current = nodes.get(&node).and_then(|record| record.parent);

      while let Some(ancestor) = current {
        let Some(ancestor_layout) = layout_of(ancestor)
        else {
          break;
        };

        let (mut offset_x, mut offset_y) = self.scroll_offset(ancestor);

        if self.is_scrollable(ancestor) {
          let (limit_x, limit_y) = self.scroll_limits(ancestor);

          #[expect(clippy::cast_possible_truncation)]
          let next = (
            fit_axis(
              i64::from(offset_x),
              rel_x,
              width,
              ancestor_layout.size.width.round() as i64,
              ancestor_layout.border.left.round() as i64,
              i64::from(limit_x),
            ),
            fit_axis(
              i64::from(offset_y),
              rel_y,
              height,
              ancestor_layout.size.height.round() as i64,
              ancestor_layout.border.top.round() as i64,
              i64::from(limit_y),
            ),
          );
          let next = (
            u32::try_from(next.0).unwrap_or(0),
            u32::try_from(next.1).unwrap_or(0),
          );

          if next != (offset_x, offset_y) {
            adjusted.push((ancestor, next));
          }

          (offset_x, offset_y) = next;
        }

        #[expect(clippy::cast_possible_truncation)]
        {
          rel_x += ancestor_layout.location.x.round() as i64 - i64::from(offset_x);
          rel_y += ancestor_layout.location.y.round() as i64 - i64::from(offset_y);
        }

        current = nodes.get(&ancestor).and_then(|record| record.parent);
      }
    }

    let changed = !adjusted.is_empty();

    for (node, next) in adjusted {
      self.set_scroll(node, next);
    }

    changed
  }
}

/// The nearest offset showing `[rel, rel + extent)` within a viewport of `size` behind
/// a border, clamped to `[0, limit]`; an oversized target aligns to its start.
const fn fit_axis(offset: i64, rel: i64, extent: i64, size: i64, border: i64, limit: i64) -> i64 {
  let min_offset = rel + extent - size + border;
  let max_offset = rel - border;
  let next = if min_offset > max_offset || offset > max_offset {
    max_offset
  }
  else if offset < min_offset {
    min_offset
  }
  else {
    offset
  };

  if next < 0 {
    0
  }
  else if next > limit {
    limit
  }
  else {
    next
  }
}
