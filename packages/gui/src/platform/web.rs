//! The web family: the realization shared by every web platform.
//!
//! `Container` → `<div>` with `display:flex` is the same decision whether it lands on a live
//! DOM or an HTML string, so the built-in views, the styling helper (`Styler`) and the CSS
//! vocabulary (`Declarations`) live here, written once against the leaf vocabulary re-exported
//! below. Each web leaf (`csr`, later `ssr`) implements that vocabulary for its own sink.
//!
//! This module is also the escape hatch (`skene_gui::web`): a low-level web view calls its
//! functions directly, cfg-gated to a web target. It is one module regardless of which leaf
//! the target selects.

mod declarations;
mod interner;
mod styler;
mod views;

/// The reset stylesheet every web leaf applies — injected into a `<style>` by the browser,
/// serialized into the document by a server.
const RESET_CSS: &str = include_str!("web/reset.css");

#[cfg(target_os = "unknown")]
mod csr;
#[cfg(target_os = "wasi")]
mod ssr;

// The active leaf's surface, re-exported up to `platform::`. Both leaves expose the same
// shared vocabulary and the structural seam (`drive`/`tree`); they differ at the edges. The
// browser leaf adds the raw-handle escape hatch (`element`/`document`) and its `launch` entry
// point — `element` is CSR-only, so a view reaching for it will not compile for the server —
// while the server leaf omits those and exposes `render_to_string` instead. `uninstall` is the
// browser leaf's test teardown.
#[cfg(all(target_os = "unknown", feature = "test"))]
pub use self::csr::uninstall;
#[cfg(target_os = "unknown")]
pub(crate) use self::csr::{adopt_next, drive, is_server, register_source, tree};
#[cfg(target_os = "unknown")]
pub use self::csr::{
  create_element,
  document,
  element,
  handle_click,
  hydrate,
  hydrate_page,
  install,
  launch,
  mount,
  override_style,
  set_attribute,
  set_style,
  set_text,
};
#[cfg(target_os = "wasi")]
pub(crate) use self::ssr::{adopt_next, drive, is_server, register_source, tree};
#[cfg(target_os = "wasi")]
pub use self::ssr::{
  create_element,
  handle_click,
  override_style,
  register_boundary,
  render_to_stream,
  render_to_string,
  set_attribute,
  set_style,
  set_text,
  sleep,
};
pub use self::{declarations::Declarations, styler::Styler};
