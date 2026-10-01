/// The line a run's text is decorated with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub enum TextDecoration {
  /// No line.
  #[default]
  None,
  /// A line beneath the text.
  Underline,
  /// A line above the text.
  Overline,
  /// A line through the text.
  LineThrough,
}

/// How a run's decoration line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub enum TextDecorationStyle {
  /// One solid line.
  #[default]
  Solid,
  /// Two solid lines.
  Double,
  /// A dotted line.
  Dotted,
  /// A dashed line.
  Dashed,
  /// A wavy line.
  Wavy,
}
