//! A box's width over its height.

/// A box's width over its height, which the axis the box leaves unsized follows.
///
/// It moves through its logarithm, as CSS's `<ratio>` does, so a change from `1.` to `4.`
/// passes `2.` halfway; a ratio of zero or of infinity is discrete.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[must_use]
pub struct AspectRatio(pub f32);

impl AspectRatio {
  /// The ratio of `width` to `height`: `AspectRatio::new(16., 9.)` is a wide screen's.
  #[inline(always)]
  pub const fn new(width: f32, height: f32) -> Self {
    Self(width / height)
  }

  /// Whether the ratio is zero or infinite, and so has no in-between with another.
  #[inline(always)]
  #[must_use]
  pub const fn is_degenerate(self) -> bool {
    !(self.0 > 0. && self.0.is_finite())
  }
}
