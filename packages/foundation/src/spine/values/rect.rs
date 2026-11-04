use super::{Inset, Point, Size};
use crate::spine::Scalar;

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

impl Rect<f32> {
  /// Whether `point` lies inside the rectangle: on or past its start edges, short of its
  /// end edges.
  #[must_use]
  pub const fn contains(self, point: Point<f32>) -> bool {
    point.x >= self.origin.x
      && point.x < self.origin.x + self.size.width
      && point.y >= self.origin.y
      && point.y < self.origin.y + self.size.height
  }

  /// The rectangle both `self` and `other` cover, of no extent where they do not meet.
  pub const fn intersect(self, other: Self) -> Self {
    let start = Point::new(
      self.origin.x.max(other.origin.x),
      self.origin.y.max(other.origin.y),
    );
    let end = Point::new(
      (self.origin.x + self.size.width).min(other.origin.x + other.size.width),
      (self.origin.y + self.size.height).min(other.origin.y + other.size.height),
    );

    Self::new(
      start,
      Size::new((end.x - start.x).max(0.), (end.y - start.y).max(0.)),
    )
  }

  /// The smallest rectangle covering both `self` and `other`.
  pub const fn union(self, other: Self) -> Self {
    let start = Point::new(
      self.origin.x.min(other.origin.x),
      self.origin.y.min(other.origin.y),
    );
    let end = Point::new(
      (self.origin.x + self.size.width).max(other.origin.x + other.size.width),
      (self.origin.y + self.size.height).max(other.origin.y + other.size.height),
    );

    Self::new(start, Size::new(end.x - start.x, end.y - start.y))
  }

  /// The rectangle grown by `by` on each edge, its start edge the left.
  pub const fn inflate(self, by: Inset<f32>) -> Self {
    Self::new(
      Point::new(self.origin.x - by.start, self.origin.y - by.top),
      Size::new(
        self.size.width + by.start + by.end,
        self.size.height + by.top + by.bottom,
      ),
    )
  }

  /// The rectangle moved by `by`.
  pub const fn translate(self, by: Point<f32>) -> Self {
    Self::new(
      Point::new(self.origin.x + by.x, self.origin.y + by.y),
      self.size,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::Rect;
  use crate::spine::{Inset, Point, Size};

  /// A rectangle at `(x, y)`, `width` by `height`.
  const fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect<f32> {
    Rect::new(Point::new(x, y), Size::new(width, height))
  }

  /// Two rectangles intersect in what both cover, of no extent where they do not meet,
  /// unite in the least rectangle covering both, grow by an inset edge by edge and move
  /// by a point.
  #[test]
  fn rectangles_intersect_unite_grow_and_move() {
    let a = rect(0., 0., 4., 3.);

    assert_eq!(a.intersect(rect(2., 1., 4., 4.)), rect(2., 1., 2., 2.));
    assert_eq!(a.intersect(rect(6., 5., 1., 1.)).size, Size::ZERO);
    assert_eq!(a.union(rect(2., -1., 4., 2.)), rect(0., -1., 6., 4.));
    assert_eq!(a.inflate(Inset::new(1, 2, 3, 4)), rect(-1., -2., 8., 9.));
    assert_eq!(a.translate(Point::new(-2., 5.)), rect(-2., 5., 4., 3.));
  }

  /// A rectangle holds a point on its start edges and short of its end edges, and none
  /// where it has no extent.
  #[test]
  fn a_rectangle_holds_the_points_short_of_its_end_edges() {
    let a = rect(1., 2., 3., 2.);

    assert!(a.contains(Point::new(1., 2.)));
    assert!(a.contains(Point::new(3.9, 3.9)));
    assert!(!a.contains(Point::new(4., 3.)));
    assert!(!a.contains(Point::new(2., 4.)));
    assert!(!a.contains(Point::new(0.9, 2.)));
    assert!(!rect(1., 1., 0., 2.).contains(Point::new(1., 1.5)));
  }
}
