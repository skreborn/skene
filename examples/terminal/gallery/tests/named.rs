//! The gallery in the terminal: every control, link, picture and dialog it shows is
//! named, as a reader finds it.

#![cfg(skene_platform = "terminal")]

use skene_terminal::{self as terminal, prelude::*, testing};

#[test]
fn every_control_the_gallery_shows_is_named() {
  terminal::install();

  let _ = terminal::mount(|| terminal_gallery::app().build());
  let _ = testing::render_to_text(60, 220);
  let unnamed = testing::find_all_unnamed();

  terminal::uninstall();

  assert!(unnamed.is_empty(), "unnamed: {unnamed:?}");
}
