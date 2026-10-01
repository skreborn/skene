use crate::spine::Scalar;

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

  /// Creates an inset with the same value on all four sides.
  #[inline(always)]
  pub const fn all(value: T) -> Self {
    Self {
      start: value,
      top: value,
      end: value,
      bottom: value,
    }
  }

  /// Creates an inset with symmetric values on the horizontal and vertical axes.
  #[inline(always)]
  pub const fn symmetric(horizontal: T, vertical: T) -> Self {
    Self {
      start: horizontal,
      top: vertical,
      end: horizontal,
      bottom: vertical,
    }
  }
}

impl<T: Scalar> Inset<T> {
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
