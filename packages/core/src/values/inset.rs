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

/// One edge of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Edge {
  /// The top edge.
  Top,
  /// The right edge.
  Right,
  /// The bottom edge.
  Bottom,
  /// The left edge.
  Left,
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
  /// The value at `edge`.
  #[inline(always)]
  pub const fn edge(&self, edge: Edge) -> T {
    match edge {
      Edge::Top => self.top,
      Edge::Right => self.right,
      Edge::Bottom => self.bottom,
      Edge::Left => self.left,
    }
  }

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
