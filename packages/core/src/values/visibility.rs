//! Whether a box is drawn.

/// Whether a box is drawn.
///
/// A hidden box keeps its place in the layout but draws nothing, itself and its content
/// alike, and takes neither the pointer nor focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub enum Visibility {
  /// Drawn, where its ancestors are.
  #[default]
  Visible,
  /// Not drawn, nor anything inside it.
  Hidden,
}
