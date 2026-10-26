//! The WebSocket example in the terminal's in-process tier: the URL and the connecting caption
//! paint before anything lands.

#![cfg(skene_platform = "terminal")]

use skene_tui::{
  platform::terminal::mount,
  prelude::*,
  testing::{self, with_platform},
};

#[test]
fn the_connecting_socket_renders() {
  let frame = with_platform(|| {
    let _ = mount(|| {
      tui_ws::app(Some("ws://127.0.0.1:1/".to_owned()))
        .into_any()
        .build()
    });

    testing::render_to_text(80, 12)
  });

  assert!(frame.contains("ws://127.0.0.1:1/"), "{frame}");
  assert!(frame.contains("Connecting"), "{frame}");
}
