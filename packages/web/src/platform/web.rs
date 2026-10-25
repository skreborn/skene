//! The web platform: [`Web`], its escape traits and its seams beyond the
//! [`Realization`](skene_foundation::Realization) trait.
//!
//! For a library author writing a view directly against the browser DOM or the server's
//! markup. The browser leaf (`skene_platform = "web"` on `wasm32-unknown-unknown`)
//! realizes against a live DOM and the server leaf (`--cfg skene_server` beside it, on
//! any target) against a string tree, both emitting the same web semantics, so a node
//! styled the same way gets the same content-addressed class on either leaf and a
//! served page hydrates in place. Each leaf implements
//! [`Realization`](skene_foundation::Realization) and
//! [`WebHydration`](skene_foundation::web::WebHydration) for [`Web`] and exposes its
//! own entry points beside them: the executor (`drive`), the document head and the path
//! (the browser's history, the server's request).
//!
//! A low-level view states an escape trait as its bound: [`WebMarkup`] (an element,
//! its attributes, raw [`Declarations`] and text) is implemented on both leaves, and
//! `WebDom` (the live element and document) on the browser leaf alone, so
//! DOM-reaching code fails to compile for the server. The native door adopts the
//! browser leaf's live `Element` as a node
//! ([`Realization::Native`](skene_foundation::Realization::Native)); the server leaf's
//! native view is `Unadoptable`, so its door cannot be reached. [`RenderHints`] is the
//! environment a server render answers with, parsed from a request's client hints.

#[cfg(skene_server)]
pub use skene_dev::Document;
pub use skene_dev::STATE_SCRIPT_ID;

#[cfg(not(skene_server))]
pub use crate::browser::boot_with_dev_state;
#[cfg(not(skene_server))]
pub use crate::browser::uninstall;
#[cfg(not(skene_server))]
pub use crate::browser::{
  add_head_element,
  adopt_restoration,
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
  view_transitions_enabled,
  watch_media,
};
#[cfg(not(skene_server))]
pub use crate::low_level::WebDom;
#[cfg(skene_server)]
pub use crate::server::{
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
pub use crate::{
  condition::{CONTROL_ATTRIBUTE, SCHEME_ATTRIBUTE},
  declarations::Declarations,
  hints::RenderHints,
  low_level::WebMarkup,
  token::WebTokenValue,
};

/// The server leaf's native view: none, so the native door cannot be reached there.
#[cfg(skene_server)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unadoptable {}

/// The web platform: zero-sized, implementing the portable vocabulary over the DOM in the
/// browser and over a string tree on the server.
///
/// Lengths are logical pixels ([`Scalar`](skene_foundation::Realization::Scalar) `f32`), the
/// style extension is [`WebStyle`](skene_foundation::web::WebStyle), and a token is a
/// [`WebTokenValue`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Web;
