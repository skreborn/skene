//! The WebSocket example rendered headlessly: the URL and the connecting caption
//! paint before anything lands.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::{self as tui, prelude::*};

/// Serializes the tests: the UI-thread designation is process-wide, so each test must
/// install, run, and uninstall without another test's thread interleaving.
static PLATFORM: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The installed platform, uninstalled when dropped, so a failing test leaves the thread
/// clean for the next.
struct Installed;

impl Drop for Installed {
  fn drop(&mut self) {
    tui::uninstall();
  }
}

/// Installs a fresh platform, runs `body`, and uninstalls afterwards.
fn with_platform<R>(body: impl FnOnce() -> R) -> R {
  let _serialized = PLATFORM
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let _installed = Installed;

  body()
}

#[test]
fn the_connecting_socket_renders() {
  let frame = with_platform(|| {
    let _ = tui::mount(|| {
      tui_ws::app(Some("ws://127.0.0.1:1/".to_owned()))
        .into_view()
        .build()
    });

    tui::render_to_text(80, 12)
  });

  assert!(frame.contains("ws://127.0.0.1:1/"), "{frame}");
  assert!(frame.contains("Connecting"), "{frame}");
}
