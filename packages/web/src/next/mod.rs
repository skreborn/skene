//! The web driver over the new model: the [`Web`] driver type, the shared compiler of
//! the property model to CSS, and the server leaf's entry points.
//!
//! Beside the crate's flat vocabulary until every facade builds on
//! [`skene_core::next`]. The compiler ([`compile`](self) internals) and the element
//! choices ([`markup`](self) internals) are shared by both leaves;
//! [`Driver`](skene_core::next::Driver) is implemented for [`Web`] on the server leaf
//! (`target_os = "wasi"`) and on the browser leaf (`target_os = "unknown"`), each
//! exposing its own entry points. The seams a facade's satellites need beyond the
//! driver ([`drive`], the hydration state channel, [`claim_lazy`] and
//! [`discard_lazy`], [`create_link`], the head and the request path) sit beside them.

// The compiler is consumed by the leaves, which only compile on wasm targets.
#[cfg_attr(not(target_arch = "wasm32"), expect(dead_code))]
mod compile;
mod condition;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod csr;
mod expr;
#[cfg(target_arch = "wasm32")]
mod markup;
mod motion;
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
mod ssr;
#[cfg_attr(not(target_arch = "wasm32"), expect(dead_code))]
mod token;

#[cfg(all(target_arch = "wasm32", target_os = "unknown", feature = "test"))]
pub use self::csr::uninstall;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use self::csr::{
  claim_lazy,
  create_link,
  discard_lazy,
  document,
  element,
  hydrate,
  hydrate_page,
  hydration_mismatches,
  install,
  launch,
  media_query,
  mount,
  set_scheme,
  stored_scheme,
  watch_media,
};
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub use self::ssr::{
  RenderHints,
  Rendered,
  StreamPiece,
  add_head_element,
  create_link,
  register_boundary,
  render_to_stream,
  render_to_stream_with,
  render_to_string,
  set_document_title,
  set_render_hints,
  set_scheme,
  stored_scheme,
};
pub use self::{
  condition::{CONTROL_ATTRIBUTE, SCHEME_ATTRIBUTE},
  token::WebTokenValue,
};
/// The seams a facade's satellites share with the crate's flat surface: the executor,
/// the hydration state channel and the document head in the browser.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use crate::{
  add_head_element,
  adopt_next,
  adopt_tagged,
  current_path,
  drive,
  is_server,
  on_pop,
  push_path,
  register_source,
  register_tagged_source,
  set_document_title,
};
/// The seams a facade's satellites share with the crate's flat surface: the executor,
/// the hydration state channel and the request the render is for.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub use crate::{
  adopt_next,
  adopt_tagged,
  current_path,
  drive,
  is_server,
  record_static_routes,
  recorded_static_routes,
  register_source,
  register_tagged_source,
  set_request_path,
};

/// The web driver: zero-sized, implementing the portable vocabulary over the DOM in the
/// browser and over a string tree on the server.
///
/// Lengths are logical pixels ([`Scalar`](skene_core::next::Driver::Scalar) `f32`),
/// the style extension is [`PixelExt`](skene_core::next::PixelExt), and a token is a
/// [`WebTokenValue`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Web;
