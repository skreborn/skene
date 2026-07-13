use num_traits::ConstZero;

/// Spacing on the four edges of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct Inset<T> {
  /// Offset from the top edge.
  pub top: T,
  /// Offset from the right edge.
  pub right: T,
  /// Offset from the bottom edge.
  pub bottom: T,
  /// Offset from the left edge.
  pub left: T,
}

impl<T: ConstZero> Inset<T> {
  /// Zero inset on all sides.
  pub const ZERO: Self = Self {
    top: T::ZERO,
    right: T::ZERO,
    bottom: T::ZERO,
    left: T::ZERO,
  };
}

impl<T: Copy> Inset<T> {
  /// Creates an inset with the same value on all four sides.
  #[inline(always)]
  pub const fn all(value: T) -> Self {
    Self {
      top: value,
      right: value,
      bottom: value,
      left: value,
    }
  }

  /// Creates an inset with symmetric values on the horizontal and vertical axes.
  #[inline(always)]
  pub const fn symmetric(horizontal: T, vertical: T) -> Self {
    Self {
      top: vertical,
      right: horizontal,
      bottom: vertical,
      left: horizontal,
    }
  }
}

impl<T: Copy + ConstZero> Inset<T> {
  /// Creates an inset with symmetric values on the horizontal axis.
  #[inline(always)]
  pub const fn horizontal(value: T) -> Self {
    Self::symmetric(value, T::ZERO)
  }

  /// Creates an inset with symmetric values on the vertical axis.
  #[inline(always)]
  pub const fn vertical(value: T) -> Self {
    Self::symmetric(T::ZERO, value)
  }
}
