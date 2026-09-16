//! The enhance example rendered headlessly: the clock's caption stands until the
//! first tick, the picture's fallback stands in for the pixels, and the stripes paint
//! once the terminal proves its graphics.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::{self as tui, prelude::*};

/// Serializes the tests: the UI-thread designation is process-wide, so each test must
/// install, run, and uninstall without another test's thread interleaving.
static DRIVER: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The installed driver, uninstalled when dropped, so a failing test leaves the thread
/// clean for the next.
struct Installed;

impl Drop for Installed {
  fn drop(&mut self) {
    tui::uninstall();
  }
}

/// Installs a fresh driver, runs `body`, and uninstalls afterwards.
fn with_driver<R>(body: impl FnOnce() -> R) -> R {
  let _serialized = DRIVER
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let _installed = Installed;

  body()
}

#[test]
fn the_forms_the_terminal_carries_render() {
  let frame = with_driver(|| {
    let _ = tui::mount(|| tui_enhance::app().into_view().build());

    tui::render_to_text(60, 30)
  });

  for expected in [
    "ENHANCE",
    "Clock",
    "Waiting for the first tick.",
    "Picture",
    "Eight blue stripes",
  ] {
    assert!(
      frame.contains(expected),
      "{expected} did not paint:\n{frame}"
    );
  }

  let frame = with_driver(|| {
    let _ = tui::mount(|| tui_enhance::app().into_view().build());
    let _ = tui::render_to_text(60, 30);

    tui::trigger_frame(std::time::Duration::from_secs(1));

    tui::render_to_text(60, 30)
  });

  assert!(frame.contains("0 s since the first tick."), "{frame}");
  assert!(!frame.contains("Waiting for the first tick"), "{frame}");
}

#[test]
fn the_picture_paints_where_the_terminal_proves_graphics() {
  with_driver(|| {
    let _ = tui::mount(|| tui_enhance::app().into_view().build());

    assert!(tui::render_to_pictures(60, 30).is_empty());

    tui::trigger_graphics(true);

    let frame = tui::render_to_text(60, 30);
    let pictures: Vec<_> = tui::render_to_pictures(60, 30)
      .into_iter()
      .filter(|picture| picture.layer == 0)
      .collect();

    assert!(!frame.contains("Eight blue stripes"), "{frame}");
    assert_eq!(
      pictures.len(),
      1,
      "the cards' backdrops lie on their own layer"
    );
    assert_eq!(pictures[0].rows, 3);
    assert_eq!(
      &pictures[0].pixels[..4],
      &[51, 89, 229, 255],
      "the first stripe's blue"
    );
  });
}
