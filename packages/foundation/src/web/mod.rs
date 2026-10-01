//! What every platform of the web shares above the spine.
//!
//! The web's style extension ([`WebStyle`]: corner radius and type metrics), its
//! [`Typography`] token set in pixels, the document [`Head`] as registered, and what
//! crosses from a server render to the client hydrating it ([`WebHydration`]: the
//! hydrating [`Resource`], the streaming [`Deferred`] and the lazily loaded
//! [`AsyncView`]); the
//! portable painting path ([`Painting`], [`Canvas`], [`Painted`]) is the crate root's,
//! re-exported here. The web's leaves and the headless platform standing in for them
//! implement against it; the web's facade aliases from it.

mod async_view;
mod deferred;
mod ext;
mod head;
mod hydration;
mod resource;
mod seams;
mod typography;

pub use self::{
  async_view::{AsyncView, AsyncViewBase, ChildLoader},
  deferred::{Deferred, DeferredBase},
  ext::{WebBoxFields, WebProperty, WebRunFields, WebStyle, web_fields},
  head::{Head, HeadElement, HeadId},
  hydration::{HydrationSource, WebHydration},
  resource::Resource,
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
