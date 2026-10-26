//! The gallery in the terminal: every control, link, picture and dialog it shows is
//! named, as a reader finds it.

#![cfg(skene_platform = "terminal")]

use skene_tui::{
  platform::terminal::{install, mount, uninstall},
  prelude::*,
  testing,
};

#[test]
fn every_control_the_gallery_shows_is_named() {
  install();

  let _ = mount(|| tui_gallery::app().build());
  let _ = testing::render_to_text(60, 220);
  let unnamed = testing::find_all_unnamed();

  uninstall();

  assert!(unnamed.is_empty(), "unnamed: {unnamed:?}");
}
