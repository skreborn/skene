//! The type metrics a medium may have, asked of a driver's style extension.

use skene_core::{PixelExt, Shape, StyleExtension};

/// A style extension that can size, weight and space a run's type.
///
/// A medium with type metrics implements the setters to write each metric into its own
/// field; the defaults write nothing, so `()` and any medium without metrics accept
/// them silently. Sizes and spacing are in the medium's unit, the weight is `100` to
/// `900`, and the line height is a multiple of the font size.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not say whether it has type metrics",
  label = "a style extension implements `TypeMetrics`, overriding the setters when its medium sizes type and leaving the defaults when it does not"
)]
pub trait TypeMetrics: StyleExtension {
  /// Writes `size` into the extension's run fields; nothing, on a medium without type
  /// metrics.
  #[inline(always)]
  fn set_font_size<S: Shape>(_fields: &mut Self::RunFields<S>, _size: S::Of<f32>) {}

  /// Writes `weight` into the extension's run fields; nothing, on a medium without
  /// type metrics.
  #[inline(always)]
  fn set_font_weight<S: Shape>(_fields: &mut Self::RunFields<S>, _weight: S::Of<f32>) {}

  /// Writes `height` into the extension's run fields; nothing, on a medium without
  /// type metrics.
  #[inline(always)]
  fn set_line_height<S: Shape>(_fields: &mut Self::RunFields<S>, _height: S::Of<f32>) {}

  /// Writes `spacing` into the extension's run fields; nothing, on a medium without
  /// type metrics.
  #[inline(always)]
  fn set_letter_spacing<S: Shape>(_fields: &mut Self::RunFields<S>, _spacing: S::Of<f32>) {}
}

impl TypeMetrics for () {}

impl TypeMetrics for PixelExt {
  #[inline(always)]
  fn set_font_size<S: Shape>(fields: &mut Self::RunFields<S>, size: S::Of<f32>) {
    fields.font_size = size;
  }

  #[inline(always)]
  fn set_font_weight<S: Shape>(fields: &mut Self::RunFields<S>, weight: S::Of<f32>) {
    fields.font_weight = weight;
  }

  #[inline(always)]
  fn set_line_height<S: Shape>(fields: &mut Self::RunFields<S>, height: S::Of<f32>) {
    fields.line_height = height;
  }

  #[inline(always)]
  fn set_letter_spacing<S: Shape>(fields: &mut Self::RunFields<S>, spacing: S::Of<f32>) {
    fields.letter_spacing = spacing;
  }
}
