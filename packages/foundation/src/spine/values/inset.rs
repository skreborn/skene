use crate::spine::{IntoScalar, Scalar};

/// Spacing on the four edges of a box, its horizontal edges logical: the start is the
/// edge text begins at, the left in left-to-right text, and the end the edge it ends at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct Inset<T> {
  /// Offset from the start edge.
  pub start: T,
  /// Offset from the top edge.
  pub top: T,
  /// Offset from the end edge.
  pub end: T,
  /// Offset from the bottom edge.
  pub bottom: T,
}

/// One edge of a box, its horizontal edges logical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Edge {
  /// The edge text begins at: the left in left-to-right text.
  Start,
  /// The top edge.
  Top,
  /// The edge text ends at: the right in left-to-right text.
  End,
  /// The bottom edge.
  Bottom,
}

impl<T: Scalar> Inset<T> {
  /// Zero inset on all sides.
  pub const ZERO: Self = Self {
    start: T::ZERO,
    top: T::ZERO,
    end: T::ZERO,
    bottom: T::ZERO,
  };
}

impl<T: Copy> Inset<T> {
  /// The value at `edge`.
  #[inline(always)]
  pub const fn edge(&self, edge: Edge) -> T {
    match edge {
      Edge::Start => self.start,
      Edge::Top => self.top,
      Edge::End => self.end,
      Edge::Bottom => self.bottom,
    }
  }

  /// An inset from its four edges, in the order of its fields: the start, the top, the
  /// end and the bottom.
  #[inline(always)]
  pub fn new(
    start: impl IntoScalar<T>,
    top: impl IntoScalar<T>,
    end: impl IntoScalar<T>,
    bottom: impl IntoScalar<T>,
  ) -> Self {
    Self {
      start: start.into_scalar(),
      top: top.into_scalar(),
      end: end.into_scalar(),
      bottom: bottom.into_scalar(),
    }
  }

  /// Creates an inset with the same value on all four sides.
  #[inline(always)]
  pub fn all(value: impl IntoScalar<T>) -> Self {
    let value = value.into_scalar();

    Self {
      start: value,
      top: value,
      end: value,
      bottom: value,
    }
  }

  /// Creates an inset with symmetric values on the horizontal and vertical axes.
  #[inline(always)]
  pub fn symmetric(horizontal: impl IntoScalar<T>, vertical: impl IntoScalar<T>) -> Self {
    let (horizontal, vertical) = (horizontal.into_scalar(), vertical.into_scalar());

    Self {
      start: horizontal,
      top: vertical,
      end: horizontal,
      bottom: vertical,
    }
  }
}

impl<T: Copy> Inset<T>
where
  i32: IntoScalar<T>,
{
  /// Creates an inset with symmetric values on the horizontal axis, zero on the
  /// vertical.
  #[inline(always)]
  pub fn horizontal(value: impl IntoScalar<T>) -> Self {
    Self::symmetric(value, 0)
  }

  /// Creates an inset with symmetric values on the vertical axis, zero on the
  /// horizontal.
  #[inline(always)]
  pub fn vertical(value: impl IntoScalar<T>) -> Self {
    Self::symmetric(0, value)
  }
}

impl<T: Copy> From<i32> for Inset<T>
where
  i32: IntoScalar<T>,
{
  /// A plain number as the same inset on all four sides.
  #[inline(always)]
  fn from(value: i32) -> Self {
    Self::all(value)
  }
}

impl<T: Copy> From<f32> for Inset<T>
where
  f32: IntoScalar<T>,
{
  /// A plain number as the same inset on all four sides, rounded to the unit.
  #[inline(always)]
  fn from(value: f32) -> Self {
    Self::all(value)
  }
}
