//! Accessibility facts a realization maps to its platform's layer.

use std::rc::Rc;

use crate::{
  NodeId,
  layout::Axis,
  seam::{
    control::AccessRequest,
    node::{LayoutKind, NodeKind},
  },
};

/// What a reader takes a node for.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Role {
  /// Nothing: a box only layout sees.
  Generic,
  /// Decoration a reader never meets: the node is left out of the accessibility tree.
  Presentation,
  /// A group of related nodes.
  Group,
  /// A landmark region, named.
  Region,
  /// Text.
  Text,
  /// A heading of a level from 1.
  Heading(u8),
  /// A picture.
  Image,
  /// A command.
  Button,
  /// A box that acts on activation and may hold controls: an activatable row.
  Row,
  /// A link.
  Link,
  /// An on/off switch.
  Switch,
  /// A checkbox.
  Checkbox,
  /// A radio button.
  Radio,
  /// A group of radio buttons.
  RadioGroup,
  /// A slider.
  Slider,
  /// A stepper.
  SpinButton,
  /// A one-line text entry.
  TextField,
  /// A multi-line text entry.
  TextEditor,
  /// A closed choice that opens a list.
  ComboBox,
  /// A list of options.
  ListBox,
  /// An option of a list.
  Option,
  /// A menu.
  Menu,
  /// A menu item.
  MenuItem,
  /// A progress indicator.
  ProgressBar,
  /// A dialog.
  Dialog,
  /// A polite live region.
  Status,
  /// An assertive live region.
  Alert,
  /// A tooltip's bubble.
  Tooltip,
  /// A scrolling region.
  ScrollArea,
  /// A list of items, a lazy list's among them: its count is its set's size.
  List,
  /// An item of a list.
  ListItem,
}

impl Role {
  /// Whether a reader operates a node of this role: a control, a command, a link, an
  /// option or an item of a menu.
  #[must_use]
  pub const fn is_interactive(self) -> bool {
    matches!(
      self,
      Self::Button
        | Self::Row
        | Self::Link
        | Self::Switch
        | Self::Checkbox
        | Self::Radio
        | Self::RadioGroup
        | Self::Slider
        | Self::SpinButton
        | Self::TextField
        | Self::TextEditor
        | Self::ComboBox
        | Self::ListBox
        | Self::Option
        | Self::Menu
        | Self::MenuItem
    )
  }

  /// The role a node of `kind` has unless it is given another.
  #[must_use]
  pub const fn of(kind: NodeKind) -> Self {
    match kind {
      NodeKind::Layout(LayoutKind::Scroll { .. }) => Self::ScrollArea,
      NodeKind::Layout(_)
      | NodeKind::Overlay
      | NodeKind::Scope
      | NodeKind::Segment
      | NodeKind::Native => Self::Generic,
      NodeKind::Text => Self::Text,
      NodeKind::Image | NodeKind::Painted => Self::Image,
      NodeKind::Editable { multiline: false } => Self::TextField,
      NodeKind::Editable { multiline: true } => Self::TextEditor,
    }
  }

  /// Whether the role takes its name from its content (a button's, an option's, a menu
  /// item's, a link's label): such a control's label is a presentational part whose text is
  /// its name, where every other control's caption names it by reference.
  #[must_use]
  pub const fn is_named_by_content(self) -> bool {
    matches!(
      self,
      Self::Button | Self::Option | Self::MenuItem | Self::Link
    )
  }
}

/// A tri-state check.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Checked {
  /// Off.
  Off,
  /// On.
  On,
  /// Mixed.
  Mixed,
}

/// What kind of popup a node opens, as a reader is told it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PopupKind {
  /// A list to choose from: a picker's.
  Listbox,
  /// A menu of commands: a context menu.
  Menu,
}

/// A value on a range, for a node that draws one itself.
#[derive(Clone, PartialEq, Debug)]
pub struct RangeValue {
  /// The value.
  pub now: f64,
  /// The least value.
  pub minimum: f64,
  /// The greatest value.
  pub maximum: f64,
  /// The step the value moves by, where it moves by one.
  pub step: Option<f64>,
  /// The value as read aloud, where it is not the number.
  pub text: Option<Rc<str>>,
}

/// What a node tells a reader beyond what its toolkit control reports itself: a native
/// control reports its own role states (checked, value) from the toolkit; these are the
/// facts any node may carry, and every fact of a control Skene draws itself.
///
/// A node's facts are the merge of every view's contribution ([`crate::seam::tree::add_access`]),
/// kept current by one effect: a later contribution's name or description wins, flags hold
/// where any contribution sets them, and relations of one kind fold into one list.
#[derive(Clone, PartialEq, Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per fact")]
pub struct AccessFacts {
  /// The keyboard shortcut that activates the node wherever focus is in its window.
  pub shortcut: Option<Shortcut>,
  /// The accessible name, where no caption names the node.
  pub name: Option<Rc<str>>,
  /// The nodes whose text names this one.
  pub labelled_by: Vec<NodeId>,
  /// A description.
  pub description: Option<Rc<str>>,
  /// The nodes whose text describes this one.
  pub described_by: Vec<NodeId>,
  /// Whether a reader skips the node and everything inside it.
  pub hidden: bool,
  /// Whether a value is required.
  pub required: bool,
  /// Whether the value is invalid.
  pub invalid: bool,
  /// Whether the value can be read but not changed.
  pub read_only: bool,
  /// Whether the node is present but not operable.
  pub disabled: bool,
  /// A drawn control's check.
  pub checked: Option<Checked>,
  /// A drawn control's expansion.
  pub expanded: Option<bool>,
  /// The popup the node opens, where it opens one: a picker's list, a context menu.
  pub has_popup: Option<PopupKind>,
  /// Whether the node is the chosen item of its collection.
  pub selected: Option<bool>,
  /// A drawn control's range value.
  pub value: Option<RangeValue>,
  /// The value a drawn control shows as text: a picker face's chosen option.
  pub value_text: Option<Rc<str>>,
  /// The axis a drawn control's value moves along.
  pub orientation: Option<Axis>,
  /// Whether a drawn node takes focus.
  pub focusable: bool,
  /// The requests a drawn control answers, as a reader finds them offered.
  pub actions: Vec<AccessRequest>,
  /// What a painted node draws, as a reader finds it: the parts of a chart, the markers of a
  /// map, each with its own role, facts and bounds in the node.
  pub parts: Vec<AccessPart>,
  /// How many items the set the node holds counts, realized or not: a lazy list's count.
  pub set_size: Option<usize>,
  /// The node's place in its set, from 0, where it is an item of one.
  pub position_in_set: Option<usize>,
  /// Whether several of the node's items can be chosen at once.
  pub multiselectable: bool,
}

impl AccessFacts {
  /// Merges `other` in after these.
  pub fn merge(&mut self, other: Self) {
    self.shortcut = other.shortcut.or(self.shortcut);
    self.name = other.name.or(self.name.take());
    self.labelled_by.extend(other.labelled_by);
    self.description = other.description.or(self.description.take());
    self.described_by.extend(other.described_by);
    self.hidden |= other.hidden;
    self.required |= other.required;
    self.invalid |= other.invalid;
    self.read_only |= other.read_only;
    self.disabled |= other.disabled;
    self.checked = other.checked.or(self.checked);
    self.expanded = other.expanded.or(self.expanded);
    self.has_popup = other.has_popup.or(self.has_popup);
    self.selected = other.selected.or(self.selected);
    self.value = other.value.or(self.value.take());
    self.value_text = other.value_text.or(self.value_text.take());
    self.orientation = other.orientation.or(self.orientation);
    self.focusable |= other.focusable;
    self.actions.extend(other.actions);
    self.parts.extend(other.parts);
    self.set_size = other.set_size.or(self.set_size);
    self.position_in_set = other.position_in_set.or(self.position_in_set);
    self.multiselectable |= other.multiselectable;
  }
}

/// A keyboard shortcut on the desktops: the primary modifier (Command on macOS, Control on
/// Windows and Linux), Shift where it says, and a key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Shortcut {
  /// The key, as its character types it unshifted (`'s'`, `'1'`).
  pub key: char,
  /// Whether Shift is held too.
  pub shift: bool,
}

impl Shortcut {
  /// The primary modifier and `key`.
  #[must_use]
  pub const fn primary(key: char) -> Self {
    Self {
      key: key.to_ascii_lowercase(),
      shift: false,
    }
  }

  /// The primary modifier, Shift and `key`.
  #[must_use]
  pub const fn primary_shift(key: char) -> Self {
    Self {
      key: key.to_ascii_lowercase(),
      shift: true,
    }
  }
}

/// A part of a painted node as a reader finds it: an accessible object with no node of its
/// own (AccessKit's model; a virtual child on every platform's layer).
#[derive(Clone, PartialEq, Debug)]
pub struct AccessPart {
  /// What a reader takes it for.
  pub role: Role,
  /// Its facts, its own parts among them.
  pub facts: AccessFacts,
  /// Its bounds in the painted node, in pixels.
  pub bounds: crate::layout::Rect,
}
