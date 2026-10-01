//! What every platform of the web shares above the spine.
//!
//! The web's style extension ([`WebStyle`]: corner radius and type metrics), its
//! [`Typography`] token set in pixels and the document [`Head`] as registered; the
//! portable painting path ([`Painting`], [`Canvas`], [`Painted`]) is the crate root's,
//! re-exported here. The web's leaves and the headless platform standing in for them
//! implement against it; the web's facade aliases from it.

mod ext;
mod head;
mod seams;
mod typography;

pub use self::{
  ext::{WebBoxFields, WebProperty, WebRunFields, WebStyle, web_fields},
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
