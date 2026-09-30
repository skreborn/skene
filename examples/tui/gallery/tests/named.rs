//! The gallery in the terminal: every control, link, picture and dialog it shows is
//! named, as a reader finds it.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::{self as tui, prelude::*};

#[test]
fn every_control_the_gallery_shows_is_named() {
  tui::install();

  let _ = tui::mount(|| tui_gallery::app().into_view().build());
  let _ = tui::render_to_text(60, 160);
  let unnamed = tui::find_all_unnamed();

  tui::uninstall();

  assert!(unnamed.is_empty(), "unnamed: {unnamed:?}");
}
