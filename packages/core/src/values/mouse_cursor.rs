/// Mouse cursor shown over a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum MouseCursor {
  /// Default arrow cursor.
  Default,
  /// Pointing-hand cursor.
  Click,
  /// Text (I-beam) cursor.
  Text,
}
