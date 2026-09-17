//! # skene-gui-core
//!
//! What every pixel platform shares above the spine: the pixel style extension
//! ([`GuiExt`]: corner radius and type metrics) and the pixel [`Typography`] token set;
//! the portable painting path ([`Painting`], [`Canvas`], [`Painted`]) is
//! `skene-views`', re-exported here. The web and headless platforms implement against
//! it; a facade for pixel media aliases from it.

mod ext;
mod seams;
mod typography;

pub use skene_views::{
  Canvas,
  IntoBrush,
  PaintType,
  Painted,
  Painter,
  Painting,
  Renderer,
  kurbo,
  paint,
  peniko,
};

pub use crate::{
  ext::{GuiBoxFields, GuiExt, GuiProperty, GuiRunFields, gui_fields},
  typography::Typography,
};
