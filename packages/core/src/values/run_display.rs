/// How a text run participates in the surrounding layout.
#[derive(Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum RunDisplay {
  /// Its own line.
  Block,
  /// Flows with the surrounding text.
  Inline,
  /// An inline-level box laid out among the surrounding text.
  InlineBox,
}
