/// A layout length: a fixed value in the medium's unit, a percentage of the parent, or
/// automatic.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum Dimension<T> {
  /// A fixed length.
  Length(T),
  /// A percentage of the parent's corresponding extent (`0.` to `100.`).
  Percent(f32),
  /// Sized by the layout algorithm.
  Auto,
}

impl<T> From<T> for Dimension<T> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::Length(value)
  }
}
