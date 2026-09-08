//! # skene-gui-core
//!
//! What every pixel driver shares above the spine: the pixel style extension
//! ([`GuiExt`]: corner radius and type metrics), the pixel [`Typography`] token set,
//! and the portable painting path ([`Painting`], [`Canvas`], [`Painted`]) through
//! vello. The web and headless drivers implement against it; a facade for pixel media
//! aliases from it.

mod ext;
mod painted;
mod painting;
mod seams;
mod typography;

pub use crate::{
  ext::{GuiBoxFields, GuiExt, GuiProperty, GuiRunFields, gui_fields},
  painted::Painted,
  painting::{Canvas, IntoBrush, PaintType, Painter, Painting, Renderer, kurbo, paint, peniko},
  typography::Typography,
};
