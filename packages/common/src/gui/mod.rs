//! What every pixel platform shares above the spine: the pixel style extension
//! ([`GuiExt`]: corner radius and type metrics), the pixel [`Typography`] token set
//! and the document [`Head`] as registered; the portable painting path ([`Painting`],
//! [`Canvas`], [`Painted`]) is the crate root's, re-exported here. The web and headless
//! platforms implement against it; a facade for pixel media aliases from it.

mod ext;
mod head;
mod seams;
mod typography;

pub use self::{
  ext::{GuiBoxFields, GuiExt, GuiProperty, GuiRunFields, gui_fields},
  head::{Head, HeadElement, HeadId},
  typography::Typography,
};
pub use crate::{
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
