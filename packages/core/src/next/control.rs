//! What a control is to the scene and to a driver: its kind, the state a box cannot
//! hold, and the parts it exposes.

use std::fmt::Debug;

use super::{BoxProps, Driver, State};
use crate::Toggle;

/// The state a control holds that a box cannot: what the control-scoped [`State`]
/// conditions of the control and everything inside it resolve against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct ControlState {
  /// Whether the control is checked.
  pub checked: Toggle,
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
    checked: Toggle::Off,
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
      State::Checked => matches!(self.checked, Toggle::On),
      State::Indeterminate => matches!(self.checked, Toggle::Mixed),
      State::Disabled => self.disabled,
      State::Invalid => self.invalid,
      State::Open => self.open,
      State::Hovered | State::Focused | State::FocusVisible | State::Pressed => false,
    }
  }
}

impl Default for ControlState {
  #[inline(always)]
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// What a control is, guiding a driver that realizes some kinds natively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum ControlKind {
  /// A control that activates on press.
  Button,
  /// A control toggling between checked and unchecked.
  Checkbox,
  /// The group a set of mutually-exclusive choices belongs to.
  RadioGroup,
  /// One mutually-exclusive choice within a group.
  Radio,
  /// A control picking a number within a range.
  Range,
  /// A single-line text entry.
  TextField,
  /// A multi-line text entry.
  TextEditor,
  /// A list of choices to select from.
  Listbox,
  /// One choice within a listbox.
  ListboxOption,
  /// A control pairing a text entry with a list of choices.
  Combobox,
}

/// Everything a driver needs to realize a control: a box with a kind and, where
/// nothing visible supplies one, an accessible name.
#[must_use]
pub struct ControlProps<D: Driver> {
  /// What the control is.
  pub kind: ControlKind,
  /// The box the control is realized as.
  pub box_props: BoxProps<D>,
  /// The accessible name, when nothing visible supplies one.
  pub label: Option<String>,
  /// The hint a text entry shows while its value is empty.
  pub placeholder: Option<String>,
  /// The lines a text entry shows at once; `1` for every other kind.
  pub lines: u16,
}

impl<D: Driver> Clone for ControlProps<D> {
  fn clone(&self) -> Self {
    Self {
      kind: self.kind,
      box_props: self.box_props.clone(),
      label: self.label.clone(),
      placeholder: self.placeholder.clone(),
      lines: self.lines,
    }
  }
}

impl<D: Driver> Debug for ControlProps<D> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ControlProps")
      .field("kind", &self.kind)
      .field("label", &self.label)
      .field("placeholder", &self.placeholder)
      .field("lines", &self.lines)
      .finish_non_exhaustive()
  }
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
}
