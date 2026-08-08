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

impl<T> From<T> for Dimension<T> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::Length(value)
  }
}
