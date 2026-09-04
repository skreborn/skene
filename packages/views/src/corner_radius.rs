//! The corner radius a medium may have, asked of a driver's style extension.

use skene_core::next::{PixelExt, Shape, StyleExtension};

/// A style extension that can round a box's corners.
///
/// A medium with a corner radius implements
/// [`set_corner_radius`](Self::set_corner_radius) to write the radius into its own
/// field; the default writes nothing, so `()` and any medium without radii accept a
/// radius silently. A library's optional decoration degrades this way; a capability a
/// library requires is a hard bound on the extension instead, so the library does not
/// exist on a medium lacking it. The radius is in the medium's unit.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not say whether it rounds corners",
  label = "a style extension implements `CornerRadius`, overriding `set_corner_radius` when its medium has a radius and leaving the default when it has none"
)]
pub trait CornerRadius: StyleExtension {
  /// Writes `radius` into the extension's box fields; nothing, on a medium without
  /// radii.
  #[inline(always)]
  fn set_corner_radius<S: Shape>(_fields: &mut Self::BoxFields<S>, _radius: S::Of<f32>) {}
}

impl CornerRadius for () {}

impl CornerRadius for PixelExt {
  #[inline(always)]
  fn set_corner_radius<S: Shape>(fields: &mut Self::BoxFields<S>, radius: S::Of<f32>) {
    fields.corner_radius = radius;
  }
}
