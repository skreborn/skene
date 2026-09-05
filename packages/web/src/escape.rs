//! The escape traits a low-level view written against the web states as its bound.
//!
//! [`WebMarkup`] is what both leaves offer: an element by tag, its attributes, raw CSS
//! declarations and text content, so a view bound on it renders in the browser and
//! serializes on the server alike. [`WebDom`] exists on the browser leaf only and hands
//! out the live [`Element`](web_sys::Element) and the [`Document`](web_sys::Document),
//! so a view bound on it does not compile for the server leaf and is wrapped in a
//! facade's `client_only` (or given a [`WebMarkup`] fallback) by the app that uses it.
//! A sink either trait creates is registered in the scene through
//! `skene_views::LowLevel`, and the node composes as a built-in does.

use skene_core::Driver;

use crate::Declarations;

/// A web driver: an element for a low-level view, on either leaf.
///
/// Every element counts in creation order on both leaves, so a view creating the same
/// elements on the server and in the browser hydrates in place.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a web driver",
  label = "a view bound on `WebMarkup` exists on the web driver's leaves alone"
)]
pub trait WebMarkup: Driver {
  /// Creates a detached element of `tag` and returns its sink.
  fn create_element(&self, tag: &'static str) -> Self::Sink;

  /// Sets the attribute `name` of the element `sink` to `value`.
  fn set_attribute(&self, sink: &Self::Sink, name: &str, value: &str);

  /// Writes `declarations` as the element's own inline CSS, each replacing the
  /// property's previous value.
  fn set_declarations(&self, sink: &Self::Sink, declarations: &Declarations);

  /// Replaces the element's text content.
  fn set_text_content(&self, sink: &Self::Sink, text: &str);
}

/// The browser leaf: the live DOM behind a sink.
///
/// Not implemented on the server leaf, where no DOM exists, so a view bound on it fails
/// to compile there rather than at run time.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[diagnostic::on_unimplemented(
  message = "`{Self}` has no DOM",
  label = "a view bound on `WebDom` exists on the browser leaf alone; bound on `WebMarkup` it exists on the server leaf too"
)]
pub trait WebDom: WebMarkup {
  /// The element realizing `sink`; `None` for an anchor or a sink no longer live.
  fn element(&self, sink: &Self::Sink) -> Option<web_sys::Element>;

  /// The owning document.
  fn document(&self) -> web_sys::Document;
}
