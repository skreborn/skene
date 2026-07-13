use std::ops::Neg;

use num_traits::ConstZero;

use super::impl_ops;

/// A point in a 2D coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct Point<T> {
  /// Position along the horizontal axis.
  pub x: T,
  /// Position along the vertical axis.
  pub y: T,
}

impl<T: ConstZero> Point<T> {
  /// The origin.
  pub const ZERO: Self = Self {
    x: T::ZERO,
    y: T::ZERO,
  };
}

impl<T> Point<T> {
  /// Creates a point from its coordinates.
  #[inline(always)]
  pub const fn new(x: T, y: T) -> Self {
    Self { x, y }
  }
}

impl_ops!(Point { x, y });

impl<T: Neg<Output = T>> Neg for Point<T> {
  type Output = Self;

  #[inline(always)]
  fn neg(self) -> Self {
    Self {
      x: -self.x,
      y: -self.y,
    }
  }
}
