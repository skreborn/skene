//! The web platform: [`Web`], its native door and its seams beyond the
//! [`Realization`](skene_foundation::Realization) trait, realized by `skene-gui-web`.
//!
//! For a library author writing a view against the browser DOM or the server's markup.
//! The browser leaf (`skene_platform = "web"` on `wasm32-unknown-unknown`) realizes
//! against a live DOM and the server leaf (`--cfg skene_server` beside it, on any
//! target) against a string tree, both emitting the same web semantics, so a node styled
//! the same way gets the same content-addressed class on either leaf and a served page
//! hydrates in place. Each leaf implements
//! [`Realization`](skene_foundation::Realization) and
//! [`WebHydration`](skene_foundation::web::WebHydration) for [`Web`] and exposes its
//! own entry points beside them: the executor (`drive`), the document head and the path
//! (the browser's history, the server's request).
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
//! ```
//! use skene_gui::{platform::web::NativeView, prelude::*};
//!
//! let well = Container::new().set_width(48).set_height(24).set_child(
//!   NativeView::new("input")
//!     .add_attribute("type", "color")
//!     .add_attribute("aria-label", "Ink"),
//! );
//! ```

#[cfg(skene_server)]
pub use skene_gui_web::Document;
pub use skene_gui_web::{
  CONTROL_ATTRIBUTE,
  NativeMarkup,
  NativeView,
  RenderHints,
  SCHEME_ATTRIBUTE,
  STATE_SCRIPT_ID,
  Undrawn,
  Web,
  WebTokenValue,
};
#[cfg(skene_server)]
pub use skene_gui_web::{
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
#[cfg(not(skene_server))]
pub use skene_gui_web::{
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
