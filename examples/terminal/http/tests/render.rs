//! The HTTP example in the terminal's in-process tier: the URL and the pending caption paint before
//! anything lands.

#![cfg(skene_platform = "terminal")]

use skene_terminal::{
  self as terminal,
  prelude::*,
  testing::{self, with_platform},
};

#[test]
fn the_pending_fetch_renders() {
  let frame = with_platform(|| {
    let _ = terminal::mount(|| {
      terminal_http::app(Some("http://127.0.0.1:1/".to_owned()))
        .into_any()
        .build()
    });

    testing::render_to_text(80, 12)
  });

  assert!(frame.contains("GET http://127.0.0.1:1/"), "{frame}");
  assert!(frame.contains("Fetching"), "{frame}");
}
