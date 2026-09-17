//! The document head: the title and the elements alongside it.
//!
//! Callable from anywhere in a build. On a server render the registrations serialize
//! into the page's `<head>`; in the browser they apply to the live document, each
//! element added once (an identical element left by a server render is adopted, not
//! duplicated). Off the web the headless platform records the registrations for a test
//! to read back.

#[cfg(not(target_arch = "wasm32"))]
use skene_headless::{add_head_element, set_document_title};
#[cfg(target_arch = "wasm32")]
use skene_web::{add_head_element, set_document_title};

/// Sets the document title.
#[inline(always)]
pub fn set_title(title: impl Into<String>) {
  set_document_title(&title.into());
}

/// Derives the document title, re-applied whenever it changes.
#[inline(always)]
pub fn derive_title(title: impl Fn() -> String + 'static) {
  skene_reactive::Effect::new(move |_: Option<()>| set_document_title(&title()));
}

/// Adds a `<meta name="…" content="…">` to the document head.
#[inline(always)]
pub fn add_meta(name: &str, content: &str) {
  add_head_element("meta", &[("name", name), ("content", content)]);
}

/// Adds a `<meta property="…" content="…">` to the document head (the Open Graph
/// form).
#[inline(always)]
pub fn add_meta_property(property: &str, content: &str) {
  add_head_element("meta", &[("property", property), ("content", content)]);
}

/// Adds a `<link rel="…" href="…">` to the document head.
#[inline(always)]
pub fn add_link(rel: &str, href: &str) {
  add_head_element("link", &[("rel", rel), ("href", href)]);
}

/// Adds a stylesheet link to the document head.
#[inline(always)]
pub fn add_stylesheet(href: &str) {
  add_link("stylesheet", href);
}

/// Adds a `<script src="…">` to the document head, deferred so it never blocks
/// parsing.
#[inline(always)]
pub fn add_script(src: &str) {
  add_head_element("script", &[("src", src), ("defer", "")]);
}
