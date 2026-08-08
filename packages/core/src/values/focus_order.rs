/// A control's place in sequential focus traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum FocusOrder {
  /// Reached in document order.
  Auto,
  /// Reached at an explicit position, counted from 0; positioned controls come before
  /// everything reached in document order.
  Position(u16),
  /// Skipped by traversal, staying focusable by pointer and programmatically.
  Skip,
}
