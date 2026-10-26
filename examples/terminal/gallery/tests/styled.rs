//! The gallery's painted frame, its text and its styles, against the snapshot file
//! beside it.
//!
//! `INSTA_UPDATE=always` rewrites the file once a change to the frame is meant.

#![cfg(skene_platform = "terminal")]

use skene_terminal::{
  platform::terminal::{install, mount, uninstall},
  prelude::*,
  testing,
};

#[test]
fn the_gallery_paints_as_recorded() {
  install();

  let _ = mount(|| terminal_gallery::app().build());
  let _ = testing::pump_tasks();
  let styled = testing::render_to_styled(60, 240);

  uninstall();

  insta::assert_snapshot!("gallery", styled);
}
