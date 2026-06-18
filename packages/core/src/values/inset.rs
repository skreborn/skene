use super::Number;

/// Spacing on the four edges of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct Inset {
  /// Offset from the top edge.
  pub top: Number,
  /// Offset from the right edge.
  pub right: Number,
  /// Offset from the bottom edge.
  pub bottom: Number,
  /// Offset from the left edge.
  pub left: Number,
}

impl Inset {
  /// Zero inset on all sides.
  pub const ZERO: Self = Self {
    top: Number::ZERO,
    right: Number::ZERO,
    bottom: Number::ZERO,
    left: Number::ZERO,
  };

  /// Creates an inset with the same value on all four sides.
  #[inline(always)]
  pub fn all(value: impl Into<Number>) -> Self {
    let value = value.into();

    Self {
      top: value,
      right: value,
      bottom: value,
      left: value,
    }
  }

  /// Creates an inset with symmetric values on the horizontal and vertical axes.
  #[inline(always)]
  pub fn symmetric(horizontal: impl Into<Number>, vertical: impl Into<Number>) -> Self {
    let horizontal = horizontal.into();
    let vertical = vertical.into();

    Self {
      top: vertical,
      right: horizontal,
      bottom: vertical,
      left: horizontal,
    }
  }

  /// Creates an inset with symmetric values on the horizontal axis.
  #[inline(always)]
  pub fn horizontal(value: impl Into<Number>) -> Self {
    Self::symmetric(value, Number::ZERO)
  }

  /// Creates an inset with symmetric values on the vertical axis.
  #[inline(always)]
  pub fn vertical(value: impl Into<Number>) -> Self {
    Self::symmetric(Number::ZERO, value)
  }
}
