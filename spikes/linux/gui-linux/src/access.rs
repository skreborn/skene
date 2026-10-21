//! The reader's view of the window: the realized tree reported to AT-SPI through AccessKit
//! (Skene's own copy of `accesskit_unix` and `accesskit_atspi_common`, carrying EditableText,
//! the missing Text methods, the relations, the expansion states and synchronous writes).
//!
//! Each node's role comes from its spec, its name, description, relations and a control's
//! check, value and expansion from the facts its views and its behavior stated
//! ([`skene_foundation::seam::access::AccessFacts`]), and a text entry's text from its editor,
//! one text run per laid line with each cluster's extent; a text's link runs are links
//! beneath it, which a reader focuses and follows. Inside a control, a part that is decoration
//! is skipped and an unnamed box stands for its children, so a reader meets a control and its
//! caption, not its boxes.
//!
//! A reader's request is reported to the control it is made of
//! ([`skene_foundation::seam::report::Report::Access`]); its behavior answers it.
//!
//! A reader's action is carried to the main loop and applied there; the thread that asked
//! waits until the tree's update with its effect is pushed, so a write reads back at once.

use std::{
  cell::{Cell, RefCell},
  collections::{HashMap, HashSet},
  rc::{Rc, Weak},
  sync::mpsc::{SyncSender, sync_channel},
  time::Duration,
};

use accesskit::{
  Action as AkAction, ActionData, ActionHandler, ActionRequest, ActivationHandler,
  DeactivationHandler, HasPopup, Invalid, Live, Node as AkNode, NodeId as AkId, Orientation,
  Rect as AkRect, Role as AkRole, TextDirection, TextPosition, TextSelection, Toggled, TreeId,
  TreeInfo, TreeUpdate,
};
use skene_foundation::{
  layout::{Axis, Rect},
  seam::{
    access::{Checked, PopupKind, Role},
    content::Unit,
    control::AccessRequest,
    node::{Interaction, LayoutKind, NodeKind},
  },
  text::{Laid as _, editing::Command},
};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
  entry,
  node::{Drawn, Node},
  runtime::{Inbox, Message},
  shell::Shell,
  trace,
};

/// The custom action ids Skene's accesskit_atspi_common sends for EditableText's clipboard
/// operations.
const COPY: i32 = -0x5c01;
const CUT: i32 = -0x5c02;
const PASTE: i32 = -0x5c03;

/// The id of the window's node.
const WINDOW: u64 = 1 << 63;
/// The tag of a text run's id.
const RUN: u64 = 1 << 62;
/// The tag of a text's link run's id.
const LINK: u64 = 1 << 59;

/// What the accessibility layer asks of the main loop.
pub struct Request {
  /// A reader's action, or `None` where the layer asks for the whole tree.
  pub action: Option<ActionRequest>,
  /// Where to say the action's effect is in the tree.
  pub done: Option<SyncSender<()>>,
}

impl std::fmt::Debug for Request {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Request")
      .field("action", &self.action)
      .finish_non_exhaustive()
  }
}

thread_local! {
  static DIRTY: Cell<bool> = const { Cell::new(true) };
  static ADAPTER: RefCell<Option<accesskit_unix::Adapter>> = const { RefCell::new(None) };
  static NODES: RefCell<HashMap<u64, Weak<Drawn>>> = RefCell::new(HashMap::new());
  /// The selection an entry had before a reader's last selection, where a copy restores it.
  static BEFORE: RefCell<Option<(Weak<Drawn>, skene_foundation::seam::content::TextSelection)>> =
    const { RefCell::new(None) };
}

/// The reader must hear the tree again.
pub fn invalidate() {
  DIRTY.with(|dirty| dirty.set(true));
}

/// Whether the reader must hear the tree again: once.
pub fn take_dirty() -> bool {
  DIRTY.with(|dirty| dirty.replace(false))
}

struct Activation(Inbox);

impl ActivationHandler for Activation {
  fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
    self.0.send(Message::Access(Request {
      action: None,
      done: None,
    }));

    None
  }
}

struct Actions(Inbox);

impl ActionHandler for Actions {
  fn do_action(&mut self, request: ActionRequest) {
    let (done, applied) = sync_channel(1);

    self.0.send(Message::Access(Request {
      action: Some(request),
      done: Some(done),
    }));

    // The reader's call returns once the action shows in the tree (a write reads back at
    // once), or after a second where the loop is busy.
    let _ = applied.recv_timeout(Duration::from_secs(1));
  }
}

struct Deactivation;

impl DeactivationHandler for Deactivation {
  fn deactivate_accessibility(&mut self) {}
}

/// Stops reporting the window: a reader no longer finds it.
pub fn stop() {
  ADAPTER.with(|slot| slot.borrow_mut().take());
  NODES.with(|nodes| nodes.borrow_mut().clear());
  invalidate();
}

/// Starts reporting the window to readers, in place of any reported before, unless
/// `SKENE_ACCESS=0`.
pub fn start() {
  if std::env::var_os("SKENE_ACCESS").is_some_and(|value| value == "0") {
    return;
  }

  let inbox = crate::runtime::inbox();
  let adapter =
    accesskit_unix::Adapter::new(Activation(inbox.clone()), Actions(inbox), Deactivation);

  ADAPTER.with(|slot| *slot.borrow_mut() = Some(adapter));
}

/// Tells the reader where the window is: `outer` its frame, `inner` its content, in screen
/// pixels.
pub fn set_window_bounds(outer: Rect, inner: Rect) {
  let rect = |rect: Rect| {
    AkRect::new(
      f64::from(rect.x),
      f64::from(rect.y),
      f64::from(rect.x + rect.width),
      f64::from(rect.y + rect.height),
    )
  };

  ADAPTER.with(|slot| {
    if let Some(adapter) = slot.borrow_mut().as_mut() {
      adapter.set_root_window_bounds(rect(outer), rect(inner));
    }
  });
}

/// Tells the reader whether the window has the keyboard.
pub fn set_window_focus(focused: bool) {
  ADAPTER.with(|slot| {
    if let Some(adapter) = slot.borrow_mut().as_mut() {
      adapter.update_window_focus_state(focused);
    }
  });
}

/// Pushes the tree where it changed.
pub fn sync(shell: &Shell) {
  let entries = entry::take_access_dirty();

  if take_dirty() || entries {
    push(shell);
  }
}

fn push(shell: &Shell) {
  ADAPTER.with(|slot| {
    if let Some(adapter) = slot.borrow_mut().as_mut() {
      adapter.update_if_active(|| build(shell));
    }
  });
}

/// Carries out a request of the accessibility layer, then pushes the tree and says so.
pub fn handle(shell: &mut Shell, request: Request) {
  if let Some(action) = &request.action {
    trace::log(|| {
      let secure = owner_of(action.target_node).is_some_and(|node| node.contents.borrow().secure);
      let data = match &action.data {
        Some(ActionData::Value(_)) if secure => "Some(Value(..))".to_owned(),
        data => format!("{data:?}"),
      };

      format!(
        "[access] {:?} at {:?} {data}",
        action.action, action.target_node
      )
    });
    apply(shell, action);
  }

  // The action's effect is laid out before the tree is built, so geometry reads current.
  shell.lay_out();
  take_dirty();
  entry::take_access_dirty();
  push(shell);

  if let Some(done) = request.done {
    let _ = done.try_send(());
  }
}

fn node_of(id: AkId) -> Option<Node> {
  NODES.with(|nodes| nodes.borrow().get(&id.0).and_then(Weak::upgrade))
}

/// The node a text run's or a link run's id belongs to.
fn owner_of(id: AkId) -> Option<Node> {
  if id.0 & (RUN | LINK) != 0 {
    NODES.with(|nodes| nodes.borrow().get(&id.0).and_then(Weak::upgrade))
  } else {
    node_of(id)
  }
}

fn apply(shell: &mut Shell, request: &ActionRequest) {
  let Some(node) = owner_of(request.target_node) else {
    return;
  };

  if request.target_node.0 & LINK != 0 {
    let link = (request.target_node.0 & 0xffff) as usize;

    match request.action {
      AkAction::Focus => shell.access_focus_link(&node, link),
      AkAction::Click => shell.access_follow(&node, link),
      _ => {}
    }

    return;
  }

  match (request.action, &request.data) {
    (AkAction::Focus, _) => shell.access_focus(&node),
    (AkAction::Click, _) => shell.access_request(&node, AccessRequest::Activate),
    (AkAction::Increment, _) => shell.access_request(&node, AccessRequest::Increment),
    (AkAction::Decrement, _) => shell.access_request(&node, AccessRequest::Decrement),
    (AkAction::Expand, _) => shell.access_request(&node, AccessRequest::Expand),
    (AkAction::Collapse, _) => shell.access_request(&node, AccessRequest::Collapse),
    (AkAction::ScrollIntoView, _) => shell.access_scroll(&node, AccessRequest::ScrollIntoView),
    (AkAction::ScrollToPoint, Some(ActionData::ScrollToPoint(point))) => {
      let scale = shell.scale;

      shell.access_scroll(
        &node,
        AccessRequest::ScrollToPoint((point.x / scale) as f32, (point.y / scale) as f32),
      );
    }
    (
      AkAction::ScrollDown | AkAction::ScrollUp | AkAction::ScrollLeft | AkAction::ScrollRight,
      _,
    ) => {
      let (axis, forward) = match request.action {
        AkAction::ScrollDown => (Axis::Vertical, true),
        AkAction::ScrollUp => (Axis::Vertical, false),
        AkAction::ScrollRight => (Axis::Horizontal, true),
        _ => (Axis::Horizontal, false),
      };

      shell.access_scroll(&node, AccessRequest::ScrollPage { axis, forward });
    }
    (AkAction::SetValue, Some(ActionData::NumericValue(value))) => {
      shell.access_request(&node, AccessRequest::SetValue(*value));
    }
    (AkAction::SetValue, Some(ActionData::Value(value))) => {
      node.with_entry(|entry| {
        entry.editor.replace(
          Some(0..entry.editor.count_in(Unit::Utf8)),
          value,
          Unit::Utf8,
        )
      });
    }
    (AkAction::SetTextSelection, Some(ActionData::SetTextSelection(selection))) => {
      let anchor = byte_of(&node, selection.anchor);
      let focus = byte_of(&node, selection.focus);

      if let (Some(anchor), Some(focus)) = (anchor, focus) {
        node.with_entry(|entry| {
          BEFORE.with(|before| {
            *before.borrow_mut() = Some((Rc::downgrade(&node), entry.editor.selection()));
          });
          entry.editor.select_in(anchor, focus, Unit::Utf8)
        });
      }
    }
    (AkAction::ReplaceSelectedText, Some(ActionData::Value(value))) => {
      node.with_entry(|entry| entry.editor.replace(None, value, Unit::Utf8));
    }
    (AkAction::CustomAction, Some(ActionData::CustomAction(id))) => match *id {
      COPY => {
        let _ = node.perform(Command::Copy);

        let before = BEFORE.with(|before| before.borrow_mut().take());

        if let Some((owner, selection)) = before
          && owner
            .upgrade()
            .is_some_and(|owner| Rc::ptr_eq(&owner, &node))
        {
          node.with_entry(|entry| entry.editor.set_selection(selection));
        }
      }
      CUT => {
        let _ = node.perform(Command::Cut);
      }
      PASTE => node.paste(crate::clipboard::Selection::Clipboard),
      _ => {}
    },
    _ => {}
  }
}

/// The byte of the entry's text a text position names.
fn byte_of(node: &Node, position: TextPosition) -> Option<usize> {
  let line = (position.node.0 & 0xffff) as usize;

  node.with_entry(|entry| {
    let laid = entry.editor.laid();
    let lines = laid.lines();
    let text = entry.editor.text();
    let range = clamp(
      text,
      lines
        .get(line)
        .map_or(0..text.len(), |line| line.range.clone()),
    );
    let slice = &text[range.clone()];

    range.start
      + slice
        .grapheme_indices(true)
        .nth(position.character_index)
        .map_or(slice.len(), |(byte, _)| byte)
  })
}

/// `range` within `text`, on character boundaries.
fn clamp(text: &str, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
  let mut start = range.start.min(text.len());
  let mut end = range.end.min(text.len()).max(start);

  while !text.is_char_boundary(start) {
    start -= 1;
  }

  while !text.is_char_boundary(end) {
    end -= 1;
  }

  start..end
}

/// An AccessKit id for scene node `node`.
fn id_of(node: &Drawn) -> u64 {
  let id = node.id.get();

  (u64::from(id.generation()) << 24 | u64::from(id.index())) & !(WINDOW | RUN | LINK)
}

fn bounds(rect: Rect, scale: f64) -> AkRect {
  AkRect::new(
    f64::from(rect.x) * scale,
    f64::from(rect.y) * scale,
    f64::from(rect.x + rect.width) * scale,
    f64::from(rect.y + rect.height) * scale,
  )
}

const fn role(role: Role, secure: bool) -> AkRole {
  match role {
    Role::Generic => AkRole::GenericContainer,
    Role::Presentation => AkRole::GenericContainer,
    Role::Group => AkRole::Group,
    Role::Region => AkRole::Region,
    Role::Text => AkRole::Label,
    Role::Heading(_) => AkRole::Heading,
    Role::Image => AkRole::Image,
    Role::Button => AkRole::Button,
    Role::Row => AkRole::ListItem,
    Role::Link => AkRole::Link,
    Role::Switch => AkRole::Switch,
    Role::Checkbox => AkRole::CheckBox,
    Role::Radio => AkRole::RadioButton,
    Role::RadioGroup => AkRole::RadioGroup,
    Role::Slider => AkRole::Slider,
    Role::SpinButton => AkRole::SpinButton,
    Role::TextField if secure => AkRole::PasswordInput,
    Role::TextField => AkRole::TextInput,
    Role::TextEditor => AkRole::MultilineTextInput,
    Role::ComboBox => AkRole::ComboBox,
    Role::ListBox => AkRole::ListBox,
    Role::Option => AkRole::ListBoxOption,
    Role::Menu => AkRole::Menu,
    Role::MenuItem => AkRole::MenuItem,
    Role::ProgressBar => AkRole::ProgressIndicator,
    Role::Dialog => AkRole::Dialog,
    Role::Status => AkRole::Status,
    Role::Alert => AkRole::Alert,
    Role::Tooltip => AkRole::Tooltip,
    Role::ScrollArea => AkRole::ScrollView,
    Role::List => AkRole::List,
    Role::ListItem => AkRole::ListItem,
  }
}

const fn ak_action(request: AccessRequest) -> AkAction {
  match request {
    AccessRequest::Activate | AccessRequest::Toggle => AkAction::Click,
    AccessRequest::Increment => AkAction::Increment,
    AccessRequest::Decrement => AkAction::Decrement,
    AccessRequest::Expand => AkAction::Expand,
    AccessRequest::Collapse => AkAction::Collapse,
    AccessRequest::Focus => AkAction::Focus,
    AccessRequest::SetValue(_) => AkAction::SetValue,
    AccessRequest::ScrollIntoView => AkAction::ScrollIntoView,
    AccessRequest::ScrollPage {
      axis: Axis::Vertical,
      forward: true,
    } => AkAction::ScrollDown,
    AccessRequest::ScrollPage {
      axis: Axis::Vertical,
      forward: false,
    } => AkAction::ScrollUp,
    AccessRequest::ScrollPage {
      axis: Axis::Horizontal,
      forward: true,
    } => AkAction::ScrollRight,
    AccessRequest::ScrollPage {
      axis: Axis::Horizontal,
      forward: false,
    } => AkAction::ScrollLeft,
    AccessRequest::ScrollToPoint(..) => AkAction::ScrollToPoint,
  }
}

/// How many nodes a reader finds in the window now, the window's own aside.
pub fn count(shell: &Shell) -> usize {
  build(shell).nodes.len() - 1
}

/// The whole tree, as AccessKit takes it.
fn build(shell: &Shell) -> TreeUpdate {
  let scale = shell.scale;
  let mut nodes: Vec<(AkId, AkNode)> = Vec::new();
  let mut map: HashMap<u64, Weak<Drawn>> = HashMap::new();
  let mut window = AkNode::new(AkRole::Window);
  let (width, height) = shell.size;

  window.set_label(shell.title().to_owned());
  window.set_bounds(bounds(
    Rect {
      x: 0.,
      y: 0.,
      width: width as f32,
      height: height as f32,
    },
    scale,
  ));

  let mut focus = AkId(WINDOW);

  let mut relations = Vec::new();

  if let Some(root) = shell.root() {
    let children = add(root, false, &mut nodes, &mut map, &mut relations, scale);

    window.set_children(children);

    // Focus names a node the tree holds, else the window: a focused node the scene removed
    // or hid is not one until the next turn moves focus off it.
    if let Some(node) = shell.focus() {
      let id = match shell.focused_link() {
        Some(link) => AkId(LINK | (id_of(node) << 16) | link as u64),
        None => AkId(id_of(node)),
      };

      if map.contains_key(&id.0) {
        focus = id;
      }
    }
  }

  Relation::resolve(relations, &mut nodes);
  nodes.insert(0, (AkId(WINDOW), window));
  NODES.with(|slot| *slot.borrow_mut() = map);

  let mut tree = TreeInfo::new(AkId(WINDOW));

  tree.toolkit_name = Some("Skene".into());
  tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());

  TreeUpdate {
    nodes,
    tree: Some(tree),
    tree_id: TreeId::ROOT,
    focus,
  }
}

/// A node's relations to the nodes that label and describe it, resolved once the whole tree
/// is built: a relation names only a node the tree holds.
struct Relation {
  /// The node they are of.
  id: AkId,
  /// The scene nodes that label it.
  labelled_by: Vec<skene_foundation::NodeId>,
  /// The scene nodes that describe it.
  described_by: Vec<skene_foundation::NodeId>,
}

impl Relation {
  /// Gives each node in `nodes` its relations to the nodes among them; where one names a node
  /// the tree leaves out (hidden, decoration, inside a text entry), the node is labelled or
  /// described by the text of every node named instead, as a browser names by a hidden
  /// label.
  fn resolve(relations: Vec<Self>, nodes: &mut [(AkId, AkNode)]) {
    if relations.is_empty() {
      return;
    }

    let exported: HashSet<u64> = nodes.iter().map(|(id, _)| id.0).collect();
    let index: HashMap<u64, usize> = nodes
      .iter()
      .enumerate()
      .map(|(index, (id, _))| (id.0, index))
      .collect();

    for relation in relations {
      let Some(&at) = index.get(&relation.id.0) else {
        continue;
      };
      let node = &mut nodes[at].1;
      let (labelled_by, label) = Self::split(&relation.labelled_by, &exported);
      let (described_by, description) = Self::split(&relation.described_by, &exported);

      if !labelled_by.is_empty() {
        node.set_labelled_by(labelled_by);
      }

      if let Some(label) = label
        && node.label().is_none()
      {
        node.set_label(label);
      }

      if !described_by.is_empty() {
        node.set_described_by(described_by);
      }

      if let Some(description) = description
        && node.description().is_none()
      {
        node.set_description(description);
      }
    }
  }

  /// The ids of `targets` the tree holds, and, where it leaves one out, the text of them all.
  fn split(
    targets: &[skene_foundation::NodeId],
    exported: &HashSet<u64>,
  ) -> (Vec<AkId>, Option<String>) {
    let found: Vec<Node> = targets
      .iter()
      .filter_map(|id| crate::node_of(*id))
      .collect();
    let held: Vec<AkId> = found
      .iter()
      .map(|node| AkId(id_of(node)))
      .filter(|id| exported.contains(&id.0))
      .collect();

    if held.len() == found.len() {
      return (held, None);
    }

    let text = found
      .iter()
      .map(|node| node.text_alternative())
      .filter(|text| !text.is_empty())
      .collect::<Vec<_>>()
      .join(" ");

    (held, (!text.is_empty()).then_some(text))
  }
}

impl Drawn {
  /// The text a relation naming this node reads: its name, else its text (a secure entry's
  /// marks), else its children's joined by a space.
  fn text_alternative(&self) -> String {
    if let Some(name) = &self.access.borrow().name {
      return name.to_string();
    }

    match self.spec.kind {
      NodeKind::Text => self
        .text
        .borrow()
        .as_ref()
        .and_then(|cell| cell.value())
        .map(|text| text.to_string())
        .unwrap_or_default(),
      NodeKind::Editable { .. } => {
        let contents = self.contents.borrow();
        let text = contents
          .edit
          .as_ref()
          .map(|edit| edit.text.to_string())
          .unwrap_or_default();

        if contents.secure {
          "•".repeat(text.graphemes(true).count())
        } else {
          text
        }
      }
      _ => self
        .children_ref()
        .iter()
        .filter(|child| !child.is_hidden() && child.is_alive())
        .map(|child| child.text_alternative())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" "),
    }
  }
}

/// Adds `node`'s accessible (or its children's, where it stands for them) and returns the
/// ids its parent lists; `in_control` where it is built inside a control. Its relations are
/// left in `relations`, for [`Relation::resolve`].
fn add(
  node: &Node,
  in_control: bool,
  nodes: &mut Vec<(AkId, AkNode)>,
  map: &mut HashMap<u64, Weak<Drawn>>,
  relations: &mut Vec<Relation>,
  scale: f64,
) -> Vec<AkId> {
  if node.is_hidden() || !node.is_alive() {
    return Vec::new();
  }

  let facts = node.access.borrow().clone();

  if facts.hidden || node.spec.role == Role::Presentation {
    return Vec::new();
  }

  let is_control = node.spec.is_control();
  let inner = in_control || is_control;
  let stands_for_children = matches!(node.spec.kind, NodeKind::Scope | NodeKind::Segment)
    || (in_control
      && !is_control
      && node.spec.role == Role::Generic
      && node.spec.interaction == Interaction::None
      && matches!(node.spec.kind, NodeKind::Layout(_) | NodeKind::Overlay)
      && facts.name.is_none()
      && facts.labelled_by.is_empty());

  if stands_for_children {
    return node
      .children()
      .iter()
      .flat_map(|child| add(child, inner, nodes, map, relations, scale))
      .collect();
  }

  let id = id_of(node);
  let contents = node.contents.borrow().clone();
  let mut ak = AkNode::new(role(node.spec.role, contents.secure));
  let rect = node.window_rect();

  map.insert(id, Rc::downgrade(node));
  ak.set_bounds(bounds(rect, scale));

  if let Role::Heading(level) = node.spec.role {
    ak.set_level(usize::from(level));
  }

  match node.spec.role {
    Role::Status => ak.set_live(Live::Polite),
    Role::Alert => ak.set_live(Live::Assertive),
    _ => {}
  }

  if let Some(name) = &facts.name {
    ak.set_label(name.to_string());
  }

  // GTK's accelerator syntax, which AT-SPI's key binding carries to Orca.
  if let Some(shortcut) = facts.shortcut {
    ak.set_keyboard_shortcut(format!(
      "<Control>{}{}",
      if shortcut.shift { "<Shift>" } else { "" },
      shortcut.key
    ));
  }

  if let Some(description) = &facts.description {
    ak.set_description(description.to_string());
  }

  if !facts.labelled_by.is_empty() || !facts.described_by.is_empty() {
    relations.push(Relation {
      id: AkId(id),
      labelled_by: facts.labelled_by.clone(),
      described_by: facts.described_by.clone(),
    });
  }

  if facts.required {
    ak.set_required();
  }

  if facts.invalid {
    ak.set_invalid(Invalid::True);
  }

  if node.spec.interaction == Interaction::Acting {
    ak.add_action(AkAction::Focus);
    ak.add_action(AkAction::Click);
  }

  if node.is_scroll_stop() {
    ak.add_action(AkAction::Focus);
  }

  if is_control {
    if facts.disabled {
      ak.set_disabled();
    } else if facts.focusable {
      ak.add_action(AkAction::Focus);
    }
  }

  if facts.read_only {
    ak.set_read_only();
  }

  if let Some(checked) = facts.checked {
    ak.set_toggled(match checked {
      Checked::Off => Toggled::False,
      Checked::On => Toggled::True,
      Checked::Mixed => Toggled::Mixed,
    });
  }

  if let Some(selected) = facts.selected {
    ak.set_selected(selected);
  }

  if let Some(expanded) = facts.expanded {
    ak.set_expanded(expanded);
  }

  match facts.has_popup {
    Some(PopupKind::Listbox) => ak.set_has_popup(HasPopup::Listbox),
    Some(PopupKind::Menu) => ak.set_has_popup(HasPopup::Menu),
    None => {}
  }

  if let Some(value) = &facts.value {
    ak.set_numeric_value(value.now);
    ak.set_min_numeric_value(value.minimum);
    ak.set_max_numeric_value(value.maximum);

    if let Some(step) = value.step {
      ak.set_numeric_value_step(step);
    }

    if let Some(text) = &value.text {
      ak.set_value(text.to_string());
    }
  }

  if let Some(text) = &facts.value_text {
    ak.set_value(text.to_string());
  }

  if let Some(size) = facts.set_size {
    ak.set_size_of_set(size);
  }

  if let Some(position) = facts.position_in_set {
    ak.set_position_in_set(position);
  }

  if facts.multiselectable {
    ak.set_multiselectable();
  }

  if let NodeKind::Layout(LayoutKind::Scroll {
    horizontal,
    vertical,
  }) = node.spec.kind
  {
    ak.add_child_action(AkAction::ScrollIntoView);

    if vertical {
      ak.add_action(AkAction::ScrollDown);
      ak.add_action(AkAction::ScrollUp);
    }

    if horizontal {
      ak.add_action(AkAction::ScrollRight);
      ak.add_action(AkAction::ScrollLeft);
    }
  }

  if let Some(axis) = facts.orientation {
    ak.set_orientation(match axis {
      Axis::Horizontal => Orientation::Horizontal,
      Axis::Vertical => Orientation::Vertical,
    });
  }

  if !facts.disabled {
    for request in &facts.actions {
      ak.add_action(ak_action(*request));
    }
  }

  let mut children = Vec::new();

  match node.spec.kind {
    NodeKind::Text => {
      let text = node
        .text
        .borrow()
        .as_ref()
        .and_then(|cell| cell.value())
        .unwrap_or_else(|| std::rc::Rc::from(""));

      ak.set_value(text.to_string());
      add_links(node, &text, &mut children, nodes, map, rect, scale);
    }
    NodeKind::Editable { .. } => {
      add_entry(node, &mut ak, &mut children, nodes, map, rect, scale);
    }
    _ => {
      for child in node.children() {
        children.extend(add(&child, inner, nodes, map, relations, scale));
      }
    }
  }

  ak.set_children(children);
  nodes.push((AkId(id), ak));

  vec![AkId(id)]
}

/// Adds a text's link runs as its children: each a link named by its text, which a reader
/// focuses and follows.
fn add_links(
  node: &Node,
  text: &str,
  children: &mut Vec<AkId>,
  nodes: &mut Vec<(AkId, AkNode)>,
  map: &mut HashMap<u64, Weak<Drawn>>,
  rect: Rect,
  scale: f64,
) {
  let links = node.links();

  if links.is_empty() {
    return;
  }

  let laid = node
    .text
    .borrow()
    .as_ref()
    .and_then(|cell| cell.laid(Some(node.laid_width.get())));
  let base = LINK | (id_of(node) << 16);

  for (index, (range, destination)) in links.into_iter().enumerate() {
    let id = AkId(base | index as u64);
    let mut link = AkNode::new(AkRole::Link);
    let area = laid
      .as_ref()
      .map(|laid| laid.range_rects(range.clone()))
      .unwrap_or_default()
      .into_iter()
      .map(|run| Rect {
        x: rect.x + run.x,
        y: rect.y + run.y,
        ..run
      })
      .reduce(crate::paint::union)
      .unwrap_or(rect);

    link.set_label(text.get(range).unwrap_or_default().to_owned());
    link.set_url(destination.to_string());
    link.set_bounds(bounds(area, scale));
    link.add_action(AkAction::Focus);
    link.add_action(AkAction::Click);
    map.insert(id.0, Rc::downgrade(node));
    children.push(id);
    nodes.push((id, link));
  }
}

/// Gives a text entry its value, its text runs (one a laid line), its selection and its text
/// actions.
fn add_entry(
  node: &Node,
  ak: &mut AkNode,
  children: &mut Vec<AkId>,
  nodes: &mut Vec<(AkId, AkNode)>,
  map: &mut HashMap<u64, Weak<Drawn>>,
  rect: Rect,
  scale: f64,
) {
  let base = RUN | (id_of(node) << 16);

  for action in [
    AkAction::SetValue,
    AkAction::SetTextSelection,
    AkAction::ReplaceSelectedText,
  ] {
    ak.add_action(action);
  }

  if let Some(placeholder) = node.contents.borrow().placeholder.as_deref() {
    ak.set_placeholder(placeholder.to_owned());
  }

  let built = node.with_entry(|entry| {
    let secure = entry.mask.is_masked();
    let text = entry.editor.text().to_owned();
    let laid = entry.editor.laid();
    let origin = entry.origin(node.entry_content(rect));
    let lines = laid.lines();
    let shown = |slice: &str| -> String {
      if secure {
        "•".repeat(slice.graphemes(true).count())
      } else {
        slice.to_owned()
      }
    };
    let mut runs = Vec::new();
    let (anchor, focus) = (entry.editor.anchor().index, entry.editor.focus().index);
    let mut positions = (None, None);
    // The lines told last time, taken again where unchanged.
    let mut told = std::mem::take(&mut entry.access_lines);
    let mut telling = HashMap::with_capacity(lines.len());

    for (index, line) in lines.iter().enumerate() {
      let line = skene_foundation::text::LineInfo {
        range: clamp(&text, line.range.clone()),
        ..line.clone()
      };
      let slice = &text[line.range.clone()];
      let shown_line = shown(slice);
      let key = crate::entry::AccessLine::key(&shown_line, line.width, scale);
      let measure = (line.width.to_bits(), scale.to_bits());
      let kept = told
        .remove(&key)
        .filter(|kept| kept.shown == shown_line && kept.measure == measure);
      let told_line = kept.unwrap_or_else(|| {
        let mut lengths = Vec::new();
        let mut xs = Vec::new();
        let mut widths = Vec::new();
        let mut words = Vec::new();
        let mut previous_blank = true;

        for (count, (start, cluster)) in slice.grapheme_indices(true).enumerate() {
          let byte = line.range.start + start;
          let x0 = laid
            .caret(skene_foundation::text::Position::new(byte))
            .primary
            .x;
          let x1 = laid
            .caret(skene_foundation::text::Position::with(
              byte + cluster.len(),
              skene_foundation::text::Affinity::Upstream,
            ))
            .primary
            .x;
          let blank = cluster.chars().all(char::is_whitespace);

          if previous_blank && !blank && count < 256 {
            words.push(count as u8);
          }

          previous_blank = blank;
          lengths.push(u8::try_from(shown(cluster).len()).unwrap_or(u8::MAX));
          // In the run's coordinates, which are its bounds' device pixels.
          xs.push((x0 - line.left) * scale as f32);
          widths.push((x1 - x0).max(0.) * scale as f32);
        }

        crate::entry::AccessLine {
          shown: shown_line,
          measure,
          lengths,
          xs,
          widths,
          words,
        }
      });
      let (lengths, xs, widths, words) = (
        told_line.lengths.clone(),
        told_line.xs.clone(),
        told_line.widths.clone(),
        told_line.words.clone(),
      );
      let value = told_line.shown.clone();

      telling.insert(key, told_line);

      let run_id = AkId(base | index as u64);
      let mut run = AkNode::new(AkRole::TextRun);
      let advance = xs
        .last()
        .zip(widths.last())
        .map_or(0., |(x, width)| (x + width) / scale as f32);

      run.set_value(value);
      run.set_character_lengths(lengths);
      run.set_character_positions(xs);
      run.set_character_widths(widths);
      run.set_word_starts(words);
      run.set_text_direction(TextDirection::LeftToRight);
      run.set_bounds(bounds(
        Rect {
          x: origin.0 + line.left,
          y: origin.1 + line.top,
          width: advance.max(1.),
          height: line.height,
        },
        scale,
      ));

      let last = index + 1 == lines.len();
      let locate = |byte: usize| -> Option<TextPosition> {
        let inside = line.range.contains(&byte) || (last && byte == line.range.end);

        inside.then(|| TextPosition {
          node: run_id,
          character_index: text[line.range.start..byte].graphemes(true).count(),
        })
      };

      if positions.0.is_none() {
        positions.0 = locate(anchor);
      }

      if positions.1.is_none() {
        positions.1 = locate(focus);
      }

      runs.push((run_id, run));
    }

    entry.access_lines = telling;

    (shown(&text), runs, positions)
  });

  let Some((value, runs, (anchor, focus))) = built else {
    return;
  };

  ak.set_value(value);

  if let (Some(anchor), Some(focus)) = (anchor, focus) {
    ak.set_text_selection(TextSelection { anchor, focus });
  }

  for (id, run) in runs {
    map.insert(id.0, Rc::downgrade(node));
    children.push(id);
    nodes.push((id, run));
  }
}
