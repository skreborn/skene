//! The type metrics a medium may have, asked of a platform's style extension.

use skene_core::{Length, Shape, StyleExtension};

/// A style extension that can size, weight and space a run's type.
///
/// A medium with type metrics implements the setters to write each metric into its own
/// field and the getters to read it back; the defaults write nothing and read nothing,
/// so `()` and any medium without metrics accept them silently. Sizes and spacing are lengths in
/// the medium's unit, the weight is `100` to `900`, and the line height is a multiple of the font
/// size.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not say whether it has type metrics",
  label = "a style extension implements `TypeMetrics`, overriding the setters when its medium sizes type and leaving the defaults when it does not"
)]
pub trait TypeMetrics: StyleExtension {
  /// Writes `size` into the extension's run fields; nothing, on a medium without type
  /// metrics.
  fn set_font_size<S: Shape>(_fields: &mut Self::RunFields<S>, _size: S::Of<Length>) {}

  /// Writes `weight` into the extension's run fields; nothing, on a medium without
  /// type metrics.
  fn set_font_weight<S: Shape>(_fields: &mut Self::RunFields<S>, _weight: S::Of<f32>) {}

  /// Writes `height` into the extension's run fields; nothing, on a medium without
  /// type metrics.
  fn set_line_height<S: Shape>(_fields: &mut Self::RunFields<S>, _height: S::Of<f32>) {}

  /// Writes `spacing` into the extension's run fields; nothing, on a medium without
  /// type metrics.
  fn set_letter_spacing<S: Shape>(_fields: &mut Self::RunFields<S>, _spacing: S::Of<Length>) {}

  /// The font size `fields` hold; unset on a medium without type metrics.
  fn font_size<S: Shape>(_fields: &Self::RunFields<S>) -> S::Of<Length> {
    S::Of::<Length>::default()
  }

  /// The font weight `fields` hold; unset on a medium without type metrics.
  fn font_weight<S: Shape>(_fields: &Self::RunFields<S>) -> S::Of<f32> {
    S::Of::<f32>::default()
  }

  /// The line height `fields` hold; unset on a medium without type metrics.
  fn line_height<S: Shape>(_fields: &Self::RunFields<S>) -> S::Of<f32> {
    S::Of::<f32>::default()
  }

  /// The letter spacing `fields` hold; unset on a medium without type metrics.
  fn letter_spacing<S: Shape>(_fields: &Self::RunFields<S>) -> S::Of<Length> {
    S::Of::<Length>::default()
  }
}

impl TypeMetrics for () {}
