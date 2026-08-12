//! The gallery rendered headlessly: every section paints and the controls respond.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::{self as tui, prelude::*};

/// Serializes the tests: the UI-thread designation is process-wide, so each test must
/// install, run, and uninstall without another test's thread interleaving.
static DRIVER: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Installs a fresh driver, runs `body`, and uninstalls afterwards.
fn with_driver<R>(body: impl FnOnce() -> R) -> R {
  let _serialized = DRIVER
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let result = body();

  tui::uninstall();

  result
}

#[test]
fn the_gallery_renders_and_responds() {
  let (frame, clicked) = with_driver(|| {
    tui::mount(|| tui_gallery::app().into_view().build());

    let frame = tui::render_to_text(60, 140);

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    (frame, tui::render_to_text(60, 140))
  });

  for title in [
    "GALLERY",
    "Text",
    "Button",
    "Checkbox & Radio",
    "TextField, Slider & Picker",
    "BorderGlyphs",
    "Grid, Stack & Overlay",
    "LazyColumn",
  ] {
    assert!(frame.contains(title), "section {title} did not paint");
  }

  assert!(frame.contains('╭'), "rounded borders did not paint");
  assert!(frame.contains('║'), "double borders did not paint");
  assert!(frame.contains('+'), "ascii borders did not paint");
  assert!(frame.contains("Row 0"), "the lazy column did not paint");
  assert!(
    !frame.contains("Row 999"),
    "the lazy column painted beyond its viewport",
  );
  assert!(
    clicked.contains("clicked 1 time(s)"),
    "the button did not respond"
  );
}
