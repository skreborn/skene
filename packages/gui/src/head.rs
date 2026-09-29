//! The document head: the title and the elements alongside it.
//!
//! Callable from anywhere in a build. Each registration holds while the owner that made
//! it lives, so a route's title, description and stylesheet leave with the route. The
//! latest live title is in force, and so is the latest live element of what it stands
//! for (a `meta` by its name or property, a `base`, any other element by its
//! attributes), so a nested route's description replaces its parent's until it leaves
//! and identical registrations write one element. On a server render the registrations
//! in force serialize into the page's `<head>`; in the browser they apply to the live
//! document, adopting an identical element a server render left. Off the web the
//! headless platform records them for a test to read back.

use skene_common::gui::HeadId;
#[cfg(skene_leaf = "host")]
use skene_headless::{
  add_head_element,
  add_head_title,
  remove_head_entry,
  set_document_language,
  set_head_title,
};
use skene_reactive::{Effect, on_cleanup, untrack};
#[cfg(not(skene_leaf = "host"))]
use skene_web::{
  add_head_element,
  add_head_title,
  remove_head_entry,
  set_document_language,
  set_head_title,
};

/// Removes the registration `id` when the current owner is disposed.
#[inline(always)]
fn hold(id: HeadId) {
  on_cleanup(move || remove_head_entry(id));
}

/// Sets the document title.
#[inline(always)]
pub fn set_title(title: impl Into<String>) {
  hold(add_head_title(&title.into()));
}

/// Derives the document title, re-applied whenever it changes.
pub fn derive_title(title: impl Fn() -> String + 'static) {
  let id = add_head_title(&untrack(&title));

  Effect::new(move |_: Option<()>| set_head_title(id, &title()));
  hold(id);
}

/// Sets the document's language, a BCP 47 tag such as `en` or `pt-BR`, which a served
/// page carries on its root.
#[inline(always)]
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
