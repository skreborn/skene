use super::{Point, Size};
use crate::Scalar;

/// A positioned, sized rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct Rect<T> {
  /// The rectangle's top-left corner.
  pub origin: Point<T>,
  /// The rectangle's extent.
  pub size: Size<T>,
}

impl<T: Scalar> Rect<T> {
  /// An empty rectangle at the origin.
  pub const ZERO: Self = Self {
    origin: Point::ZERO,
    size: Size::ZERO,
  };
}

impl<T> Rect<T> {
  /// Creates a rectangle from its origin and size.
  #[inline(always)]
  pub const fn new(origin: Point<T>, size: Size<T>) -> Self {
    Self { origin, size }
  }
}
