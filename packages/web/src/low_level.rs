//! The escape traits a low-level view written against the web states as its bound.
//!
//! [`WebMarkup`] is what both leaves offer: an element by tag, its attributes, raw CSS
//! declarations and text content, so a view bound on it renders in the browser and
//! serializes on the server alike. [`WebDom`] exists on the browser leaf only and hands
//! out the live [`Element`](web_sys::Element) and the [`Document`](web_sys::Document),
//! so a view bound on it does not compile for the server leaf and is wrapped in a
//! facade's `client_only` (or given a [`WebMarkup`] fallback) by the app that uses it.
//! The node an element realizes is minted through `skene_foundation::LowLevel`, and it
//! composes as a built-in does.

use skene_foundation::{NodeId, Platform};

use crate::declarations::Declarations;

/// A web platform: an element for a low-level view, on either leaf.
///
/// Every element counts in creation order on both leaves, so a view creating the same
/// elements on the server and in the browser hydrates in place.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a web platform",
  label = "a view bound on `WebMarkup` exists on the web platform's leaves alone"
)]
pub trait WebMarkup: Platform {
  /// Realizes `node` as a detached element of `tag`.
  fn create_element(node: NodeId, tag: &'static str);

  /// Sets the attribute `name` of the element realizing `node` to `value`.
  fn set_attribute(node: NodeId, name: &str, value: &str);

  /// Writes `declarations` as the element's own inline CSS, each replacing the
  /// property's previous value.
  fn set_declarations(node: NodeId, declarations: &Declarations);

  /// Replaces the element's text content.
  fn set_text(node: NodeId, text: &str);
}

/// The browser leaf: the live DOM behind a node.
///
/// Not implemented on the server leaf, where no DOM exists, so a view bound on it fails
/// to compile there rather than at run time.
#[cfg(skene_web_platform = "browser")]
#[diagnostic::on_unimplemented(
  message = "`{Self}` has no DOM",
  label = "a view bound on `WebDom` exists on the browser leaf alone; bound on `WebMarkup` it exists on the server leaf too"
)]
pub trait WebDom: WebMarkup {
  /// The element realizing `node`; `None` for an anchor or a node no longer live.
  fn element(node: NodeId) -> Option<web_sys::Element>;

  /// The owning document.
  fn document() -> web_sys::Document;
}
