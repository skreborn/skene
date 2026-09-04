//! The web driver over the new model: the [`Web`] driver type, the shared compiler of
//! the property model to CSS, and the server leaf's entry points.
//!
//! Beside the crate's flat vocabulary until every facade builds on
//! [`skene_core::next`]. The compiler ([`compile`](self) internals) and the element
//! choices ([`markup`](self) internals) are shared by both leaves;
//! [`Driver`](skene_core::next::Driver) is implemented for [`Web`] on the server leaf
//! (`target_os = "wasi"`) and on the browser leaf (`target_os = "unknown"`), each
//! exposing its own entry points.

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
  add_head_element,
  render_to_string,
  set_document_title,
  set_render_hints,
};
pub use self::{
  condition::{CONTROL_ATTRIBUTE, SCHEME_ATTRIBUTE},
  token::WebTokenValue,
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
