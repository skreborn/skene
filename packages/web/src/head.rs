//! The document head: the elements alongside the window's title, which
//! [`window`](crate::window) sets.
//!
//! Callable from anywhere in a build. Each registration holds while the owner that made
//! it lives, so a route's description and stylesheet leave with the route. The latest
//! live element of what it stands for is in force (a `meta` by its name or property, a
//! `base`, any other element by its attributes), so a nested route's description
//! replaces its parent's until it leaves and identical registrations write one element. On a server
//! render the registrations in force serialize into the page's `<head>`, which a streamed render
//! flushes with its shell, so a registration inside a `Deferred` reaches that page only once the
//! client hydrates; in the browser they apply to the live document, adopting an identical
//! element a server render left. Off the web the headless platform records them for a
//! test to read back.

use skene_foundation::web::HeadId;
#[cfg(skene_web_platform = "headless")]
use skene_headless::{add_head_element, remove_head_entry, set_document_language};
use skene_reactive::on_cleanup;

#[cfg(not(skene_web_platform = "headless"))]
use crate::platform::{add_head_element, remove_head_entry, set_document_language};

/// Removes the registration `id` when the current owner is disposed.
#[inline(always)]
fn hold(id: HeadId) {
  on_cleanup(move || remove_head_entry(id));
}

/// Sets the document's language, a BCP 47 tag such as `en` or `pt-BR`, which a served
/// page carries on its root.
pub fn set_language(language: impl Into<String>) {
  set_document_language(&language.into());
}

/// Adds a `<meta name="…" content="…">` to the document head.
#[inline(always)]
pub fn add_meta(name: &str, content: &str) {
  hold(add_head_element(
    "meta",
    &[("name", name), ("content", content)],
  ));
}

/// Adds a `<meta property="…" content="…">` to the document head (the Open Graph
/// form).
#[inline(always)]
pub fn add_meta_property(property: &str, content: &str) {
  hold(add_head_element(
    "meta",
    &[("property", property), ("content", content)],
  ));
}

/// Adds a `<link rel="…" href="…">` to the document head.
#[inline(always)]
pub fn add_link(rel: &str, href: &str) {
  hold(add_head_element("link", &[("rel", rel), ("href", href)]));
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
  hold(add_head_element("script", &[("src", src), ("defer", "")]));
}
