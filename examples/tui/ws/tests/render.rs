//! The WebSocket example rendered headlessly: the URL and the connecting caption
//! paint before anything lands.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::{
  self as tui,
  prelude::*,
  testing::{self, with_platform},
};

#[test]
fn the_connecting_socket_renders() {
  let frame = with_platform(|| {
    let _ = tui::mount(|| {
      tui_ws::app(Some("ws://127.0.0.1:1/".to_owned()))
        .into_view()
        .build()
    });

    testing::render_to_text(80, 12)
  });

  assert!(frame.contains("ws://127.0.0.1:1/"), "{frame}");
  assert!(frame.contains("Connecting"), "{frame}");
}
