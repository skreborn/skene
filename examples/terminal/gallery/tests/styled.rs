//! The gallery's painted frame, its text and its styles, against the snapshot file
//! beside it.
//!
//! `INSTA_UPDATE=always` rewrites the file once a change to the frame is meant.

#![cfg(not(target_arch = "wasm32"))]

use skene_terminal::{self as terminal, prelude::*, testing};

#[test]
fn the_gallery_paints_as_recorded() {
  terminal::install();

  let _ = terminal::mount(|| terminal_gallery::app().build());
  let _ = testing::pump_tasks();
  let styled = testing::render_to_styled(60, 220);

  terminal::uninstall();

  insta::assert_snapshot!("gallery", styled);
}
