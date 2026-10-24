//! What a control is to the scene and to a platform: its native element, the state a
//! box cannot hold, and the parts it exposes.

use std::fmt::Debug;

use super::{BoxFacts, State};
use crate::spine::{CheckState, NodeId, Role};

/// The state a control holds that a box cannot: what the control-scoped [`State`]
/// conditions of the control and everything inside it resolve against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct ControlState {
  /// Whether the control is checked.
  pub checked: CheckState,
  /// Whether the control is present but not operable.
  pub disabled: bool,
  /// Whether the control holds an invalid value.
  pub invalid: bool,
  /// Whether the control's popup is open.
  pub open: bool,
}

impl ControlState {
  /// Unchecked, enabled, valid and closed.
  pub const DEFAULT: Self = Self {
    checked: CheckState::Off,
    disabled: false,
    invalid: false,
    open: false,
  };

  /// Whether `state` holds; `false` for a state that is a node's own rather than the
  /// control's.
  #[inline(always)]
  #[must_use]
  pub const fn holds(self, state: State) -> bool {
    match state {
      State::Checked => matches!(self.checked, CheckState::On),
      State::Indeterminate => matches!(self.checked, CheckState::Mixed),
      State::Disabled => self.disabled,
      State::Invalid => self.invalid,
      State::Open => self.open,
      State::Hovered
      | State::Focused
      | State::FocusVisible
      | State::Pressed
      | State::Dragged
      | State::Targeted => false,
    }
  }
}

impl Default for ControlState {
  #[inline(always)]
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// The native element a control is realized as where a platform has one: the web's
/// form element, a native platform's own control.
///
/// A control with none is drawn by its view and named by its [`Role`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum NativeElement {
  /// A button.
  Button,
  /// A checkbox.
  Checkbox,
  /// One mutually-exclusive choice within a group.
  Radio,
  /// A number picked within a range.
  Slider,
  /// A single-line text entry.
  TextField,
  /// A multi-line text entry.
  TextEditor,
  /// A choice among options the platform draws and lists itself.
  Picker,
}

impl NativeElement {
  /// The role the element carries where nothing states another: a text entry's, a
  /// button's, a toggle's, a range's or a picker's.
  #[inline(always)]
  #[must_use]
  pub const fn role(self) -> Role {
    match self {
      Self::Button => Role::Button,
      Self::Checkbox => Role::CheckBox,
      Self::Radio => Role::RadioButton,
      Self::Slider => Role::Slider,
      Self::TextField => Role::TextInput,
      Self::TextEditor => Role::MultilineTextInput,
      Self::Picker => Role::ComboBox,
    }
  }

  /// Whether the element is a text entry, single-line or not.
  #[inline(always)]
  #[must_use]
  pub const fn is_text_entry(self) -> bool {
    matches!(self, Self::TextField | Self::TextEditor)
  }
}

/// What a platform creates a control from: its box, the native element it stands in
/// where it has one and, where nothing visible supplies one, an accessible name.
#[derive(Clone, Debug)]
#[must_use]
pub struct ControlFacts {
  /// The box the control is realized as.
  pub boxed: BoxFacts,
  /// The native element the control stands in; `None` for one its view draws.
  pub native: Option<NativeElement>,
  /// The accessible name, when nothing visible supplies one.
  pub label: Option<String>,
  /// The hint a text entry shows while its value is empty.
  pub placeholder: Option<String>,
  /// The lines a text entry shows at once; `1` for every other kind.
  pub lines: u16,
  /// Whether the control's [`Part::Label`] sits before its box among its siblings,
  /// answering to the control, rather than inside it.
  pub label_beside: bool,
  /// The collection whose members the platform groups as one, as a radio group's
  /// choices; `None` for a control no group holds.
  pub group: Option<NodeId>,
}

/// A styling target a control exposes: the node a part is recorded on is that part of
/// the nearest enclosing control.
///
/// A part's focus is its control's: `Focused`, `FocusVisible` and `Pressed` on a node
/// inside a control resolve against the control, while `Hovered` stays the node's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Part {
  /// The control's own node.
  Root,
  /// The extent a range's value moves along.
  Track,
  /// The portion of the track up to the value.
  Fill,
  /// The handle at the value.
  Thumb,
  /// The mark a toggle shows while checked.
  Mark,
  /// The face a toggle or button draws.
  Face,
  /// The list of choices a popup shows.
  List,
  /// One choice within the list.
  Item,
  /// The caption naming the control.
  Label,
  /// The bubble a tooltip shows beside the node it describes.
  Bubble,
}
