//! What the pixel medium answers of the seams a component library asks a medium for.

use skene_core::{Length, Shape};

use super::GuiExt;
use crate::{CornerRadius, TypeMetrics};

impl CornerRadius for GuiExt {
  #[inline(always)]
  fn set_corner_radius<S: Shape>(fields: &mut Self::BoxFields<S>, radius: S::Of<Length>) {
    fields.corner_radius = radius;
  }
}

impl TypeMetrics for GuiExt {
  #[inline(always)]
  fn set_font_size<S: Shape>(fields: &mut Self::RunFields<S>, size: S::Of<Length>) {
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
  fn set_letter_spacing<S: Shape>(fields: &mut Self::RunFields<S>, spacing: S::Of<Length>) {
    fields.letter_spacing = spacing;
  }
}
