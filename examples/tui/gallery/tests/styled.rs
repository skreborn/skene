//! The gallery's painted frame, its text and its styles, against the snapshot file
//! beside it.
//!
//! `INSTA_UPDATE=always` rewrites the file once a change to the frame is meant.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::{self as tui, prelude::*, testing};

#[test]
fn the_gallery_paints_as_recorded() {
  tui::install();

  let _ = tui::mount(|| tui_gallery::app().into_view().build());
  let styled = testing::render_to_styled(60, 160);

  tui::uninstall();

  insta::assert_snapshot!("gallery", styled);
}
