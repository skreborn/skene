//! The window the page draws in: its title, the latest one a live owner sets in force.
//!
//! On a server render the title in force serializes into the page's head; in the browser
//! it is the document's title; on the host the headless platform records it.

use crate::Active;

/// Sets the window's title, the document's, while the current owner lives; the latest
/// title a live owner set is in force.
#[inline(always)]
pub fn set_title(title: impl Into<String>) {
  skene_foundation::window::set_title::<Active>(title);
}

/// Derives the window's title, the document's, from the signals `title` reads, while
/// the current owner lives; the latest title a live owner set is in force.
#[inline(always)]
pub fn derive_title(title: impl Fn() -> String + 'static) {
  skene_foundation::window::derive_title::<Active>(title);
}
