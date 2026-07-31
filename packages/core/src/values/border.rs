use super::Color;

/// A solid border: a width in logical pixels and a color.
///
/// The corner radius is a separate property (`set_border_radius`).
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Border {
  /// The border width, in logical pixels.
  pub width: f32,
  /// The border color.
  pub color: Color,
}

impl Border {
  /// Creates a border with the given width and color.
  #[inline(always)]
  pub fn new(width: f32, color: Color) -> Self {
    Self { width, color }
  }
}
