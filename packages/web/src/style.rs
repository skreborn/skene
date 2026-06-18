//! Web-backend style value types.

use skene_core::{Color, Number};

/// A solid border: a width (logical pixels) and a color. The corner radius is not
/// part of this—it is set separately with
/// [`Container::set_border_radius`](crate::Container::set_border_radius).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Border {
  /// The border width, in logical pixels.
  pub width: Number,
  /// The border color.
  pub color: Color,
}

impl Border {
  /// Creates a border with the given width and color.
  #[inline(always)]
  pub fn new(width: impl Into<Number>, color: Color) -> Border {
    Border {
      width: width.into(),
      color,
    }
  }
}
