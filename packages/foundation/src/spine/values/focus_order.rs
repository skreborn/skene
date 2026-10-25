/// A control's place in sequential focus traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum FocusOrder {
  /// Reached in the scene's child order.
  Auto,
  /// Skipped by traversal, staying focusable by pointer and programmatically.
  Skip,
}
