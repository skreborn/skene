//! # Recording driver
//!
//! The host driver for headless view tests: the same typed vocabulary the real
//! drivers realize, mapped to an in-memory tree that records every call. Tests build
//! views exactly as an app does, then inspect the recorded nodes ([`outline`],
//! [`children`], [`text`], [`box_style`]) and simulate events through the
//! `trigger_` seams.
//!
//! Recording is faithful bookkeeping, not emulation: no layout runs, no default
//! event behavior fires—a trigger runs exactly the handlers the view registered.

use std::{cell::RefCell, fmt::Write as _, rc::Rc};

use rustc_hash::FxHashMap;
use skene_core::{
  BoxProp,
  BoxProps,
  BoxStyle,
  ButtonProps,
  CheckboxProps,
  InputProps,
  KeyEvent,
  NodeId,
  OverlayProps,
  PointerEvent,
  Prevention,
  RadioProps,
  Role,
  RunProp,
  RunProps,
  RunStyle,
  ScrollEvent,
  SelectProps,
  SliderProps,
  TextareaProps,
};

thread_local! {
  static RECORDING: RefCell<Recording> = RefCell::new(Recording::default());
}

/// What one recorded node is, with everything its creation call carried.
#[derive(Clone)]
enum Kind {
  Group,
  Box(Box<BoxProps>),
  Run(Box<RunProps>),
  Button(Box<ButtonProps>),
  Input(Box<InputProps>),
  Checkbox(Box<CheckboxProps>),
  Radio(Box<RadioProps>),
  Select(Box<SelectProps>),
  Textarea(Box<TextareaProps>),
  Slider(Box<SliderProps>),
  Overlay(Box<OverlayProps>),
}

impl Kind {
  fn name(&self) -> &'static str {
    match self {
      Kind::Group => "Group",
      Kind::Box(_) => "Box",
      Kind::Run(_) => "Run",
      Kind::Button(_) => "Button",
      Kind::Input(_) => "Input",
      Kind::Checkbox(_) => "Checkbox",
      Kind::Radio(_) => "Radio",
      Kind::Select(_) => "Select",
      Kind::Textarea(_) => "Textarea",
      Kind::Slider(_) => "Slider",
      Kind::Overlay(_) => "Overlay",
    }
  }
}

/// A registered scroll handler.
type ScrollHandler = Rc<dyn Fn(&ScrollEvent)>;
/// A registered change handler, receiving the committed value.
type ChangeHandler = Rc<dyn Fn(&str)>;

/// One recorded node: its creation props, live state, and registered handlers.
#[derive(Default)]
struct Handlers {
  click: Vec<Rc<skene_core::ClickHandler>>,
  focus: Vec<Rc<dyn Fn()>>,
  blur: Vec<Rc<dyn Fn()>>,
  key_down: Vec<Rc<skene_core::KeyHandler>>,
  pointer_enter: Vec<Rc<dyn Fn()>>,
  pointer_leave: Vec<Rc<dyn Fn()>>,
  pointer_down: Vec<Rc<skene_core::PointerHandler>>,
  pointer_up: Vec<Rc<skene_core::PointerHandler>>,
  pointer_move: Vec<Rc<skene_core::PointerHandler>>,
  scroll: Vec<ScrollHandler>,
  change: Vec<ChangeHandler>,
  toggle: Vec<Rc<dyn Fn(bool)>>,
  dismiss: Vec<Rc<dyn Fn()>>,
  size: Vec<Rc<dyn Fn(f32, f32)>>,
}

struct Node {
  kind: Kind,
  children: Vec<NodeId>,
  /// Reactive box-property updates folded in application order.
  box_overrides: Vec<BoxProp>,
  /// Reactive run-property updates folded in application order.
  run_overrides: Vec<RunProp>,
  text: String,
  value: String,
  checked: bool,
  open: bool,
  placed_in_stack: bool,
  handlers: Handlers,
}

impl Node {
  fn new(kind: Kind) -> Self {
    Self {
      kind,
      children: Vec::new(),
      box_overrides: Vec::new(),
      run_overrides: Vec::new(),
      text: String::new(),
      value: String::new(),
      checked: false,
      open: false,
      placed_in_stack: false,
      handlers: Handlers::default(),
    }
  }
}

#[derive(Default)]
struct Recording {
  next: u64,
  nodes: FxHashMap<NodeId, Node>,
}

impl Recording {
  fn insert(&mut self, kind: Kind) -> NodeId {
    let id = NodeId::new(self.next);

    self.next += 1;
    self.nodes.insert(id, Node::new(kind));

    id
  }
}

fn with<R>(f: impl FnOnce(&mut Recording) -> R) -> R {
  RECORDING.with(|recording| f(&mut recording.borrow_mut()))
}

fn with_node<R>(node: NodeId, f: impl FnOnce(&mut Node) -> R) -> Option<R> {
  with(|recording| recording.nodes.get_mut(&node).map(f))
}

// --- The typed vocabulary ---

/// Records a group node.
#[must_use]
pub fn create_group() -> NodeId {
  with(|recording| recording.insert(Kind::Group))
}

/// Attaches `child` under `parent`, before `before` or at the end.
pub fn attach(parent: NodeId, child: NodeId, before: Option<NodeId>) {
  let _ = with_node(parent, |node| {
    node.children.retain(|existing| *existing != child);

    match before.and_then(|before| node.children.iter().position(|c| *c == before)) {
      Some(index) => node.children.insert(index, child),
      None => node.children.push(child),
    }
  });
}

/// Detaches `child` from `parent`.
pub fn detach(parent: NodeId, child: NodeId) {
  let _ = with_node(parent, |node| {
    node.children.retain(|existing| *existing != child);
  });
}

/// Destroys `node` and every recorded descendant.
pub fn destroy(node: NodeId) {
  let children = with_node(node, |node| std::mem::take(&mut node.children));

  for child in children.unwrap_or_default() {
    destroy(child);
  }

  with(|recording| {
    recording.nodes.remove(&node);
  });
}

/// Records a box with its full static props.
#[must_use]
pub fn create_box(props: &BoxProps) -> NodeId {
  with(|recording| recording.insert(Kind::Box(Box::new(props.clone()))))
}

/// Records one changed box property, applied over the static style.
pub fn update_box(node: NodeId, prop: &BoxProp) {
  let _ = with_node(node, |node| node.box_overrides.push(prop.clone()));
}

/// Records a stack child's placement into its parent's single cell.
pub fn place_in_stack(node: NodeId) {
  let _ = with_node(node, |node| node.placed_in_stack = true);
}

/// Records a text run with its full static props.
#[must_use]
pub fn create_run(props: &RunProps) -> NodeId {
  with(|recording| recording.insert(Kind::Run(Box::new(props.clone()))))
}

/// Records one changed run property, applied over the static style.
pub fn update_run(node: NodeId, prop: &RunProp) {
  let _ = with_node(node, |node| node.run_overrides.push(*prop));
}

/// Records a run's text content.
pub fn set_text(node: NodeId, text: &str) {
  let _ = with_node(node, |node| {
    node.text.clear();
    node.text.push_str(text);
  });
}

/// Records a button control.
#[must_use]
pub fn create_button(props: &ButtonProps) -> NodeId {
  with(|recording| recording.insert(Kind::Button(Box::new(props.clone()))))
}

/// Records a single-line text input.
#[must_use]
pub fn create_input(props: &InputProps) -> NodeId {
  with(|recording| recording.insert(Kind::Input(Box::new(props.clone()))))
}

/// Records a checkbox.
#[must_use]
pub fn create_checkbox(props: &CheckboxProps) -> NodeId {
  with(|recording| recording.insert(Kind::Checkbox(Box::new(props.clone()))))
}

/// Records one radio button.
#[must_use]
pub fn create_radio(props: &RadioProps) -> NodeId {
  with(|recording| recording.insert(Kind::Radio(Box::new(props.clone()))))
}

/// Records a select control.
#[must_use]
pub fn create_select(props: &SelectProps) -> NodeId {
  with(|recording| recording.insert(Kind::Select(Box::new(props.clone()))))
}

/// Records a multi-line text area.
#[must_use]
pub fn create_textarea(props: &TextareaProps) -> NodeId {
  with(|recording| recording.insert(Kind::Textarea(Box::new(props.clone()))))
}

/// Records a slider.
#[must_use]
pub fn create_slider(props: &SliderProps) -> NodeId {
  with(|recording| recording.insert(Kind::Slider(Box::new(props.clone()))))
}

/// Records an overlay, closed until [`set_overlay_open`] opens it.
#[must_use]
pub fn create_overlay(props: &OverlayProps) -> NodeId {
  with(|recording| recording.insert(Kind::Overlay(Box::new(props.clone()))))
}

/// Records an overlay's open state; the anchor is bookkeeping the recorder ignores.
pub fn set_overlay_open(node: NodeId, open: bool, _anchor: Option<NodeId>) {
  let _ = with_node(node, |node| node.open = open);
}

/// Records one changed box property on a control, applied over its static style.
pub fn update_input(node: NodeId, prop: &BoxProp) {
  update_box(node, prop);
}

/// Records a control's current value.
pub fn set_input_value(node: NodeId, value: &str) {
  let _ = with_node(node, |node| {
    node.value.clear();
    node.value.push_str(value);
  });
}

/// Records a checkbox's (or radio's) checked state.
pub fn set_checked(node: NodeId, checked: bool) {
  let _ = with_node(node, |node| node.checked = checked);
}

/// Records a programmatic focus; the recorder keeps no focus model.
pub fn focus_node(_node: NodeId) {}

/// Records a programmatic blur; the recorder keeps no focus model.
pub fn blur_node(_node: NodeId) {}

/// Registers a size observer; the recorder runs no layout, so it never fires.
pub fn observe_size(node: NodeId, handler: Rc<dyn Fn(f32, f32)>) {
  let _ = with_node(node, |node| node.handlers.size.push(handler));
}

macro_rules! register {
  ($(#[$doc:meta] $fn_name:ident => $slot:ident: $handler:ty;)+) => {
    $(
      #[$doc]
      pub fn $fn_name(node: NodeId, handler: Rc<$handler>) {
        let _ = with_node(node, |node| node.handlers.$slot.push(handler));
      }
    )+
  };
}

register! {
  /// Registers a click handler.
  handle_click => click: skene_core::ClickHandler;
  /// Registers a focus handler.
  handle_focus => focus: dyn Fn();
  /// Registers a blur handler.
  handle_blur => blur: dyn Fn();
  /// Registers a key-down handler.
  handle_key_down => key_down: skene_core::KeyHandler;
  /// Registers a pointer-enter handler.
  handle_pointer_enter => pointer_enter: dyn Fn();
  /// Registers a pointer-leave handler.
  handle_pointer_leave => pointer_leave: dyn Fn();
  /// Registers a pointer-down handler.
  handle_pointer_down => pointer_down: skene_core::PointerHandler;
  /// Registers a pointer-up handler.
  handle_pointer_up => pointer_up: skene_core::PointerHandler;
  /// Registers a pointer-move handler.
  handle_pointer_move => pointer_move: skene_core::PointerHandler;
  /// Registers a scroll handler.
  handle_scroll => scroll: dyn Fn(&ScrollEvent);
  /// Registers a change handler.
  handle_change => change: dyn Fn(&str);
  /// Registers a toggle handler.
  handle_toggle => toggle: dyn Fn(bool);
  /// Registers an overlay-dismiss handler.
  handle_overlay_dismiss => dismiss: dyn Fn();
}

// --- Test seams: triggers ---

/// Runs `node`'s click handlers, returning the accumulated prevention.
pub fn trigger_click(node: NodeId) -> Prevention {
  let handlers = with_node(node, |node| node.handlers.click.clone()).unwrap_or_default();
  let mut prevention = Prevention::default();

  for handler in handlers {
    prevention.absorb(handler());
  }

  prevention
}

/// Runs `node`'s key-down handlers with `event`, returning the accumulated
/// prevention.
pub fn trigger_key_down(node: NodeId, event: &KeyEvent) -> Prevention {
  let handlers = with_node(node, |node| node.handlers.key_down.clone()).unwrap_or_default();
  let mut prevention = Prevention::default();

  for handler in handlers {
    prevention.absorb(handler(event));
  }

  prevention
}

/// Runs `node`'s pointer-down handlers with `event`, returning the accumulated
/// prevention.
pub fn trigger_pointer_down(node: NodeId, event: &PointerEvent) -> Prevention {
  let handlers = with_node(node, |node| node.handlers.pointer_down.clone()).unwrap_or_default();
  let mut prevention = Prevention::default();

  for handler in handlers {
    prevention.absorb(handler(event));
  }

  prevention
}

/// Records a committed value and runs `node`'s change handlers with it.
pub fn trigger_change(node: NodeId, value: &str) {
  set_input_value(node, value);

  let handlers = with_node(node, |node| node.handlers.change.clone()).unwrap_or_default();

  for handler in handlers {
    handler(value);
  }
}

/// Records a checked state and runs `node`'s toggle handlers with it.
pub fn trigger_toggle(node: NodeId, checked: bool) {
  set_checked(node, checked);

  let handlers = with_node(node, |node| node.handlers.toggle.clone()).unwrap_or_default();

  for handler in handlers {
    handler(checked);
  }
}

/// Runs `node`'s focus handlers.
pub fn trigger_focus(node: NodeId) {
  let handlers = with_node(node, |node| node.handlers.focus.clone()).unwrap_or_default();

  for handler in handlers {
    handler();
  }
}

/// Runs `node`'s blur handlers.
pub fn trigger_blur(node: NodeId) {
  let handlers = with_node(node, |node| node.handlers.blur.clone()).unwrap_or_default();

  for handler in handlers {
    handler();
  }
}

/// Runs `node`'s dismiss handlers, as Escape or a backdrop press would.
pub fn trigger_dismiss(node: NodeId) {
  let handlers = with_node(node, |node| node.handlers.dismiss.clone()).unwrap_or_default();

  for handler in handlers {
    handler();
  }
}

/// Reports a measured size to `node`'s size observers.
pub fn trigger_size(node: NodeId, width: f32, height: f32) {
  let handlers = with_node(node, |node| node.handlers.size.clone()).unwrap_or_default();

  for handler in handlers {
    handler(width, height);
  }
}

// --- Test seams: inspection ---

/// Clears the recorded tree; call between tests sharing a thread.
pub fn reset() {
  with(|recording| *recording = Recording::default());
}

/// The recorded children of `node`, in order.
#[must_use]
pub fn children(node: NodeId) -> Vec<NodeId> {
  with_node(node, |node| node.children.clone()).unwrap_or_default()
}

/// The recorded text of a run, if the node is one.
#[must_use]
pub fn text(node: NodeId) -> Option<String> {
  with_node(node, |node| node.text.clone())
}

/// The recorded value of a control.
#[must_use]
pub fn value(node: NodeId) -> Option<String> {
  with_node(node, |node| node.value.clone())
}

/// The recorded checked state of a checkbox or radio.
#[must_use]
pub fn checked(node: NodeId) -> Option<bool> {
  with_node(node, |node| node.checked)
}

/// Whether the recorded overlay is open.
#[must_use]
pub fn overlay_open(node: NodeId) -> Option<bool> {
  with_node(node, |node| node.open)
}

/// A box's effective style: its static style with every reactive update folded in,
/// in application order.
#[must_use]
pub fn box_style(node: NodeId) -> Option<BoxStyle> {
  with_node(node, |node| {
    let mut style = match &node.kind {
      Kind::Box(props) => props.style.clone(),
      Kind::Button(props) => props.control.style.clone(),
      Kind::Input(props) => props.control.style.clone(),
      Kind::Checkbox(props) => props.control.style.clone(),
      Kind::Radio(props) => props.control.style.clone(),
      Kind::Select(props) => props.control.style.clone(),
      Kind::Textarea(props) => props.control.style.clone(),
      Kind::Slider(props) => props.control.style.clone(),
      Kind::Overlay(props) => props.style.clone(),
      Kind::Group | Kind::Run(_) => return None,
    };

    for prop in &node.box_overrides {
      apply_box_prop(&mut style, prop);
    }

    Some(style)
  })
  .flatten()
}

/// A run's effective style: its static style with every reactive update folded in.
#[must_use]
pub fn run_style(node: NodeId) -> Option<RunStyle> {
  with_node(node, |node| {
    let Kind::Run(props) = &node.kind
    else {
      return None;
    };

    let mut style = props.style;

    for prop in &node.run_overrides {
      match prop {
        RunProp::Color(value) => style.color = Some(*value),
        RunProp::FontSize(value) => style.font_size = Some(*value),
        RunProp::FontWeight(value) => style.font_weight = Some(*value),
      }
    }

    Some(style)
  })
  .flatten()
}

/// A box's semantic role, if the node is a box with one.
#[must_use]
pub fn role(node: NodeId) -> Option<Role> {
  with_node(node, |node| match &node.kind {
    Kind::Box(props) => props.role,
    _ => None,
  })
  .flatten()
}

/// An indented outline of the recorded tree under `node`: one line per node, its kind
/// plus the text or value it carries.
#[must_use]
pub fn outline(node: NodeId) -> String {
  let mut out = String::new();

  outline_into(node, 0, &mut out);

  out
}

fn outline_into(node: NodeId, depth: usize, out: &mut String) {
  let line = with_node(node, |entry| {
    let mut line = String::new();

    line.push_str(entry.kind.name());

    if !entry.text.is_empty() {
      let _ = write!(line, " {:?}", entry.text);
    }

    if !entry.value.is_empty() {
      let _ = write!(line, " value={:?}", entry.value);
    }

    (line, entry.children.clone())
  });

  let Some((line, children)) = line
  else {
    return;
  };

  for _ in 0..depth {
    out.push_str("  ");
  }

  out.push_str(&line);
  out.push('\n');

  for child in children {
    outline_into(child, depth + 1, out);
  }
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
    BoxProp::AspectRatio(value) => style.aspect_ratio = Some(*value),
    BoxProp::Margin(value) => style.margin = Some(*value),
    BoxProp::Padding(value) => style.padding = Some(*value),
    BoxProp::Position(value) => style.position = Some(*value),
    BoxProp::Inset(value) => style.inset = Some(*value),
    BoxProp::Color(value) => style.color = Some(*value),
    BoxProp::Border(value) => style.border = Some(*value),
    BoxProp::BorderRadius(value) => style.border_radius = Some(*value),
    BoxProp::Cursor(value) => style.cursor = Some(*value),
    BoxProp::RowSpacing(value) => style.row_spacing = Some(*value),
    BoxProp::ColumnSpacing(value) => style.column_spacing = Some(*value),
    BoxProp::FlexWrap(value) => style.flex_wrap = Some(*value),
    BoxProp::FlexGrow(value) => style.flex_grow = Some(*value),
    BoxProp::FlexShrink(value) => style.flex_shrink = Some(*value),
    BoxProp::FlexBasis(value) => style.flex_basis = Some(*value),
    BoxProp::MainAxisAlignment(value) => style.main_axis_alignment = Some(*value),
    BoxProp::CrossAxisAlignment(value) => style.cross_axis_alignment = Some(*value),
    BoxProp::LineAlignment(value) => style.line_alignment = Some(*value),
    BoxProp::MainAxisItemAlignment(value) => style.main_axis_item_alignment = Some(*value),
    BoxProp::SelfCrossAxisAlignment(value) => style.self_cross_axis_alignment = Some(*value),
    BoxProp::SelfMainAxisAlignment(value) => style.self_main_axis_alignment = Some(*value),
    BoxProp::GridTemplateColumns(value) => style.grid_template_columns = Some(value.clone()),
    BoxProp::GridTemplateRows(value) => style.grid_template_rows = Some(value.clone()),
    BoxProp::GridAutoColumns(value) => style.grid_auto_columns = Some(value.clone()),
    BoxProp::GridAutoRows(value) => style.grid_auto_rows = Some(value.clone()),
    BoxProp::GridAutoFlow(value) => style.grid_auto_flow = Some(*value),
    BoxProp::GridRow(value) => style.grid_row = Some(*value),
    BoxProp::GridColumn(value) => style.grid_column = Some(*value),
    BoxProp::Overflow(value) => style.overflow = Some(*value),
  }
}
