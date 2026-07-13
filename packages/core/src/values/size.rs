use num_traits::ConstZero;

use super::impl_ops;

/// A 2D size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct Size<T> {
  /// Extent along the horizontal axis.
  pub width: T,
  /// Extent along the vertical axis.
  pub height: T,
}

impl<T: ConstZero> Size<T> {
  /// Zero on both axes.
  pub const ZERO: Self = Self {
    width: T::ZERO,
    height: T::ZERO,
  };
}

impl<T> Size<T> {
  /// Creates a size from its extents.
  #[inline(always)]
  pub const fn new(width: T, height: T) -> Self {
    Self { width, height }
  }
}

impl_ops!(Size { width, height });
