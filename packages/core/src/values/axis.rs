/// Layout axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Axis {
  /// Horizontal axis.
  Horizontal,
  /// Vertical axis.
  Vertical,
}

/// One value per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Axes<T> {
  /// The horizontal axis's value.
  pub horizontal: T,
  /// The vertical axis's value.
  pub vertical: T,
}

impl<T: Copy> Axes<T> {
  /// The same value on both axes.
  #[inline(always)]
  pub const fn all(value: T) -> Self {
    Self {
      horizontal: value,
      vertical: value,
    }
  }

  /// A value per axis.
  #[inline(always)]
  pub const fn new(horizontal: T, vertical: T) -> Self {
    Self {
      horizontal,
      vertical,
    }
  }
}

impl<T: Copy> From<T> for Axes<T> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::all(value)
  }
}
