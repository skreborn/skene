//! Overflow behavior.

/// How a box treats content that exceeds its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Overflow {
  /// Content spills out visibly.
  Visible,
  /// Content is clipped.
  Hidden,
  /// Content is clipped and the box scrolls, with scrollbars always shown.
  Scroll,
  /// Content is clipped and the box scrolls when needed.
  Auto,
}
