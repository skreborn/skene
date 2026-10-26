//! # skene-gui-web
//!
//! The web's realization of `skene-gui`: [`Web`], its native door and its seams beyond
//! the [`Realization`](skene_foundation::Realization) trait, in the browser and on a
//! server render.
//!
//! An app names no item here: `skene-gui` is its dependency, naming [`Web`] as its
//! `Active` and re-exporting this crate's surface as `skene_gui::platform::web`, where a
//! library author writing a view against the browser DOM or the server's markup reaches
//! it. The browser leaf (`skene_platform = "web"` on `wasm32-unknown-unknown`) realizes
//! against a live DOM and the server leaf (`--cfg skene_server` beside it, on any
//! target) against a string tree, both emitting the same web semantics through one
//! style compiler and one interner, so a node styled the same way gets the same
//! content-addressed class on either leaf and a served page hydrates in place. Each leaf
//! implements [`Realization`](skene_foundation::Realization) and
//! [`WebHydration`](skene_foundation::web::WebHydration) for [`Web`] and exposes its own
//! entry points beside them: the executor (`drive`), the document head and the path (the
//! browser's history, the server's request).
//!
//! [`NativeView`] is the native door: an element of the DOM built from its
//! [`NativeMarkup`], adopted as a node. The browser leaf's native view
//! ([`Realization::Native`](skene_foundation::Realization::Native)) is the live
//! `Element`, and the server leaf's is the markup it renders as the placeholder a
//! hydrating browser adopts in place. [`RenderHints`] is the environment a server render
//! answers with, parsed from a request's client hints. Every text entry is the web's form
//! element, edited by the browser, so the web draws no entry and its editing state is
//! [`Undrawn`].
//!
//! The crate compiles in a build naming `skene_platform = "web"` alone and is empty in
//! every other; the facade refuses a build it cannot realize.

#![cfg(skene_platform = "web")]

#[cfg(not(skene_server))]
mod browser;
mod compile;
mod condition;
mod declarations;
mod expr;
#[cfg(all(not(skene_server), feature = "gpu"))]
mod gpu;
mod hints;
mod interner;
mod link_policy;
mod logging;
mod markup;
mod motion;
mod native_view;
mod realize;
mod resource_state;
#[cfg(skene_server)]
mod server;
mod style_book;
mod token;

#[cfg(skene_server)]
pub use skene_dev::Document;
pub use skene_dev::STATE_SCRIPT_ID;

#[cfg(not(skene_server))]
pub use self::browser::{
  add_head_element,
  adopt_restoration,
  boot_with_dev_state,
  commit_navigation,
  current_path,
  document,
  drive,
  element,
  enable_view_transitions,
  end_turn,
  hydrate,
  hydration_mismatches,
  install,
  install_executor,
  launch,
  media_query,
  mount,
  on_pop,
  persisted_scheme,
  push_path,
  remove_head_entry,
  set_document_language,
  set_scheme,
  uninstall,
  view_transitions_enabled,
  watch_media,
};
#[cfg(skene_server)]
pub use self::server::{
  Rendered,
  StreamPiece,
  add_head_element,
  current_path,
  document_closing,
  document_opening,
  drive,
  enable_view_transitions,
  install_executor,
  install_logging,
  persisted_scheme,
  record_static_routes,
  recorded_static_routes,
  remove_head_entry,
  render_to_stream_with,
  render_to_string,
  set_client_bundle,
  set_dev_loop,
  set_document_language,
  set_render_deadline,
  set_render_hints,
  set_request_path,
  set_scheme,
  set_script_nonce,
  sleep,
};
pub use self::{
  condition::{CONTROL_ATTRIBUTE, SCHEME_ATTRIBUTE},
  hints::RenderHints,
  link_policy::{LinkAnnotation, LinkPolicy},
  native_view::{NativeMarkup, NativeView},
  token::WebTokenValue,
};

/// The web platform: zero-sized, implementing the portable vocabulary over the DOM in the
/// browser and over a string tree on the server.
///
/// Lengths are logical pixels ([`Scalar`](skene_foundation::Realization::Scalar) `f32`), the
/// style extension is [`WebStyle`](skene_foundation::web::WebStyle), and a token is a
/// [`WebTokenValue`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Web;

/// The web's drawn entry's editing state: none, since every text entry is its `<input>` or
/// `<textarea>`, edited by the browser, so the shared editing state cannot be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Undrawn {}
