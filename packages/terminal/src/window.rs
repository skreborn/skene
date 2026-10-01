//! The window the app draws in: its title, the latest one a live owner sets in force.
//!
//! The terminal shows the title in force through OSC 2; a terminal without window
//! titles ignores it.

use crate::ActivePlatform;

/// Sets the window's title, the terminal's, while the current owner lives; the latest
/// title a live owner set is in force.
#[inline(always)]
pub fn set_title(title: impl Into<String>) {
  skene_foundation::window::set_title::<ActivePlatform>(title);
}

/// Derives the window's title, the terminal's, from the signals `title` reads, while
/// the current owner lives; the latest title a live owner set is in force.
#[inline(always)]
pub fn derive_title(title: impl Fn() -> String + 'static) {
  skene_foundation::window::derive_title::<ActivePlatform>(title);
}
