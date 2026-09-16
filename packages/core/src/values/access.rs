use super::{Axis, Toggle};
use crate::NodeId;

/// One accessibility property a widget reports, for a reactive update.
///
/// Semantics, not style: a driver maps these to whatever its accessibility layer
/// speaks—ARIA attributes on the web, a platform accessibility node elsewhere—so the
/// vocabulary names the concept and never the platform's spelling of it. Pairs with a
/// widget [`Role`](super::Role), which says what the control *is* while these say what it currently
/// reports.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub enum AccessProp {
  /// Whether a toggle is checked.
  Checked(Toggle),
  /// The value a range control currently holds.
  Value(f32),
  /// The lowest value that control accepts.
  ValueMin(f32),
  /// The highest value that control accepts.
  ValueMax(f32),
  /// A human-readable reading of the value, when the number alone does not say enough.
  ValueText(String),
  /// Whether the control's associated content is revealed.
  Expanded(bool),
  /// Whether a choice within a set is the chosen one.
  Selected(bool),
  /// Whether the control is present but not operable.
  Disabled(bool),
  /// The control's accessible name, when nothing visible supplies one.
  Label(String),
  /// Whether a value must be supplied before the form the control is in submits.
  Required(bool),
  /// Whether the control holds an invalid value.
  Invalid(bool),
  /// Whether the control's value is shown but not editable.
  ReadOnly(bool),
  /// The kind of popup the control opens.
  HasPopup(PopupKind),
  /// The node the control operates: its popup's list, the region it expands.
  Controls(NodeId),
  /// The node whose text names the control, where the name is not the control's own
  /// content.
  LabelledBy(NodeId),
  /// The node whose text describes the control.
  DescribedBy(NodeId),
  /// A plain-text description: the accessible description where nothing else supplies
  /// one, and the platform's own tooltip where the platform shows one without script.
  Description(String),
  /// The axis a range control runs along.
  Orientation(Axis),
}

/// What a control's popup is; a list of options unless told otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use]
pub enum PopupKind {
  /// A list of options.
  #[default]
  Listbox,
  /// A menu of commands.
  Menu,
  /// A dialog.
  Dialog,
}
