use super::Toggle;

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
}
