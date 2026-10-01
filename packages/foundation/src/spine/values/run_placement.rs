/// Where a text run sits in the surrounding layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum RunPlacement {
  /// On a line of its own.
  Line,
  /// Flowing with the surrounding text.
  Inline,
  /// A box embedded among the surrounding text.
  Embedded,
}
