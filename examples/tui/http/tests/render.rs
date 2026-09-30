//! The HTTP example rendered headlessly: the URL and the pending caption paint before
//! anything lands.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::{
  self as tui,
  prelude::*,
  testing::{self, with_platform},
};

#[test]
fn the_pending_fetch_renders() {
  let frame = with_platform(|| {
    let _ = tui::mount(|| {
      tui_http::app(Some("http://127.0.0.1:1/".to_owned()))
        .into_view()
        .build()
    });

    testing::render_to_text(80, 12)
  });

  assert!(frame.contains("GET http://127.0.0.1:1/"), "{frame}");
  assert!(frame.contains("Fetching"), "{frame}");
}
