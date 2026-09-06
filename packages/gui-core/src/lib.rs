//! # skene-gui-core
//!
//! What every pixel driver shares above the spine: the pixel style extension
//! ([`PixelExt`]: corner radius and type metrics), the pixel [`Typography`] token set,
//! and the portable painting path ([`Painting`], [`Canvas`], [`Painted`]) through
//! vello. The web and headless drivers implement against it; a facade for pixel media
//! aliases from it.

mod painted;
mod painting;
mod pixel;
mod seams;
mod typography;

pub use crate::{
  painted::Painted,
  painting::{Canvas, IntoBrush, Painter, Painting, kurbo, peniko},
  pixel::{PixelBoxFields, PixelExt, PixelProperty, PixelRunFields, pixel_fields},
  typography::Typography,
};
