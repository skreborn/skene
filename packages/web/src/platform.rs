//! The web platform: [`Web`], its escape traits and its seams beyond the
//! [`Platform`](skene_core::Platform) trait.
//!
//! For a library author writing a view directly against the browser DOM or the server's
//! markup. The browser leaf (`skene_web_platform = "browser"`) realizes against a live
//! DOM and the server leaf (`skene_web_platform = "server"`) against a string tree,
//! both emitting the same web semantics, so a node styled the same way gets the same
//! content-addressed class on either leaf and a served page hydrates in place. The
//! compiler and the element choices are shared; each leaf implements
//! [`Platform`](skene_core::Platform) for [`Web`] and exposes its own entry points
//! beside the seams a satellite such as [`Link`](crate::Link) needs beyond the
//! platform: the executor (`drive`), the hydration state channel (`adopt_next`,
//! `register_source` and their tagged twins, `is_server`), the document head, the path
//! (the browser's history, the server's request) and, in the browser, `claim_lazy` and
//! `discard_lazy`.
//!
//! A low-level view states an escape trait as its bound: [`WebMarkup`] (an element,
//! its attributes, raw [`Declarations`] and text) is implemented on both leaves, and
//! `WebDom` (the live element and document) on the browser leaf alone, so
//! DOM-reaching code fails to compile for the server. [`RenderHints`] is the
//! environment a server render answers with, parsed from a request's client hints.

#[cfg(skene_web_platform = "browser")]
pub use crate::browser::boot_with_dev_state;
#[cfg(skene_web_platform = "browser")]
pub use crate::browser::uninstall;
#[cfg(skene_web_platform = "browser")]
pub use crate::browser::{
  add_head_element,
  add_head_title,
  adopt_restoration,
  claim_lazy,
  commit_navigation,
  current_path,
  discard_lazy,
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
  set_head_title,
  set_scheme,
  view_transitions_enabled,
  watch_media,
};
#[cfg(not(skene_web_platform = "browser"))]
pub use crate::document::Document;
#[cfg(skene_web_platform = "browser")]
pub use crate::low_level::WebDom;
#[cfg(not(skene_web_platform = "headless"))]
pub use crate::resource_state::{
  LazyRegion,
  adopt_next,
  adopt_tagged,
  enter_lazy,
  is_server,
  register_source,
  register_tagged_source,
  within_lazy,
};
#[cfg(skene_web_platform = "server")]
pub use crate::server::{
  Rendered,
  StreamPiece,
  add_head_element,
  add_head_title,
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
  register_boundary,
  remove_head_entry,
  render_to_stream_with,
  render_to_string,
  set_client_bundle,
  set_dev_loop,
  set_document_language,
  set_head_title,
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
  resource_state::{HydrationSource, STATE_SCRIPT_ID},
  token::WebTokenValue,
};

/// The web platform: zero-sized, implementing the portable vocabulary over the DOM in the
/// browser and over a string tree on the server.
///
/// Lengths are logical pixels ([`Scalar`](skene_core::Platform::Scalar) `f32`), the
/// style extension is [`GuiStyle`](skene_common::gui::GuiStyle), and a token is a
/// [`WebTokenValue`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Web;
