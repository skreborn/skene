//! The skene web platform: the [`Web`] platform type, the shared compiler of the property
//! model to CSS, and two leaves selected by target.
//!
//! The browser leaf (`csr`, `target_os = "unknown"`) realizes against a live DOM and
//! the server leaf (`ssr`, `target_os = "wasi"`) against a string tree, both emitting
//! the same web semantics, so a node styled the same way gets the same
//! content-addressed class on either leaf and a served page hydrates in place. The
//! compiler and the element choices are shared; each leaf implements
//! [`Platform`](skene_core::Platform) for [`Web`] and exposes its own entry points beside
//! the seams a facade's satellites need beyond the platform: the executor (`drive`),
//! the hydration state channel (`adopt_next`, `register_source` and their tagged
//! twins, `is_server`), the document head, the path (the browser's history, the
//! server's request) and, in the browser, `claim_lazy` and `discard_lazy`.
//!
//! A low-level view states an escape trait as its bound: [`WebMarkup`] (an element,
//! its attributes, raw [`Declarations`] and text) is implemented on both leaves, and
//! `WebDom` (the live element and document) on the browser leaf alone, so
//! DOM-reaching code fails to compile for the server. [`RenderHints`] is the
//! environment a server render answers with, parsed from a request's client hints.

// The compiler is consumed by the leaves, which only compile on wasm targets.
#[cfg_attr(not(target_arch = "wasm32"), expect(dead_code))]
mod compile;
mod condition;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod csr;
mod declarations;
mod escape;
mod expr;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod gpu;
mod hints;
mod hydration;
#[cfg_attr(not(target_arch = "wasm32"), expect(dead_code))]
mod interner;
#[cfg(target_arch = "wasm32")]
mod markup;
mod motion;
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
mod ssr;
#[cfg_attr(not(target_arch = "wasm32"), expect(dead_code))]
mod token;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use self::csr::boot_with_dev_state;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use self::csr::uninstall;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use self::csr::{
  add_head_element,
  adopt_restoration,
  claim_lazy,
  commit_navigation,
  current_path,
  discard_lazy,
  document,
  drive,
  element,
  enable_view_transitions,
  hydrate,
  hydrate_page,
  hydration_mismatches,
  install,
  install_executor,
  launch,
  media_query,
  mount,
  on_pop,
  push_path,
  set_document_title,
  set_scheme,
  stored_scheme,
  view_transitions_enabled,
  watch_media,
};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use self::escape::WebDom;
#[cfg(target_arch = "wasm32")]
pub use self::hydration::{
  LazyRegion,
  adopt_next,
  adopt_tagged,
  enter_lazy,
  is_server,
  register_source,
  register_tagged_source,
  within_lazy,
};
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub use self::ssr::{
  Rendered,
  StreamPiece,
  add_head_element,
  current_path,
  drive,
  enable_view_transitions,
  install_executor,
  record_static_routes,
  recorded_static_routes,
  register,
  register_boundary,
  render_to_stream,
  render_to_stream_with,
  render_to_string,
  set_document_title,
  set_render_deadline,
  set_render_hints,
  set_request_path,
  set_scheme,
  sleep,
  stored_scheme,
};
pub use self::{
  condition::{CONTROL_ATTRIBUTE, SCHEME_ATTRIBUTE},
  declarations::Declarations,
  escape::WebMarkup,
  hints::RenderHints,
  hydration::{HydrationSource, STATE_SCRIPT_ID},
  token::WebTokenValue,
};

/// The reset stylesheet every web leaf applies: injected into a `<style>` by the
/// browser, serialized into the document by a server.
#[cfg(target_arch = "wasm32")]
const RESET_CSS: &str = include_str!("reset.css");

/// The web platform: zero-sized, implementing the portable vocabulary over the DOM in the
/// browser and over a string tree on the server.
///
/// Lengths are logical pixels ([`Scalar`](skene_core::Platform::Scalar) `f32`), the
/// style extension is [`GuiExt`](skene_common::gui::GuiExt), and a token is a
/// [`WebTokenValue`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Web;
