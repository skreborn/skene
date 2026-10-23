use crate::spine::{IntoScalar, Scalar};

/// A layout length: a fixed value in the medium's unit, a fraction of the parent, or
/// automatic.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum Dimension<T> {
  /// A fixed length.
  Length(T),
  /// A fraction of the parent's corresponding extent: `0.5` is half the parent,
  /// whatever the siblings say, and values above `1.` overflow it.
  ///
  /// Not a weight—to split an axis between siblings in proportion, give them
  /// [`Flexibility`](super::Flexibility) weights (or, for grid tracks,
  /// [`GridTrack::Flex`](super::GridTrack) shares) instead.
  Fraction(f32),
  /// Sized by the layout algorithm.
  Auto,
}

impl<N: Scalar> Dimension<N> {
  /// This dimension with a length or a fraction below zero raised to zero.
  pub(in crate::spine) fn at_least_zero(self) -> Self {
    match self {
      Self::Length(units) if units.to_f32() < 0. => Self::Length(N::ZERO),
      Self::Fraction(value) => Self::Fraction(value.max(0.)),
      dimension => dimension,
    }
  }
}

impl<N: Scalar> From<i32> for Dimension<N> {
  /// A plain number as a fixed length.
  #[inline(always)]
  fn from(length: i32) -> Self {
    Self::Length(N::from_i32(length))
  }
}

impl<N: Scalar> From<f32> for Dimension<N> {
  /// A plain number as a fixed length, rounded to the unit.
  #[inline(always)]
  fn from(length: f32) -> Self {
    Self::Length(N::from_f32(length))
  }
}

/// A plain number as a fixed length, wherever a dimension stands in for a length: an
/// inset of dimensions takes `Inset::all(-4)` as it takes `Inset::all(Dimension::Auto)`.
impl<N: Scalar> IntoScalar<Dimension<N>> for i32 {
  #[inline(always)]
  fn into_scalar(self) -> Dimension<N> {
    Dimension::from(self)
  }
}

/// A plain number as a fixed length, rounded to the unit.
impl<N: Scalar> IntoScalar<Dimension<N>> for f32 {
  #[inline(always)]
  fn into_scalar(self) -> Dimension<N> {
    Dimension::from(self)
  }
}

/// A dimension as itself.
impl<N: Scalar> IntoScalar<Self> for Dimension<N> {
  #[inline(always)]
  fn into_scalar(self) -> Self {
    self
  }
}
