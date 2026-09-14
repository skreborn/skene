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
  let (frame, clicked, switched, neutral, indigo) = with_driver(|| {
    let _ = tui::mount(|| tui_gallery::app().into_view().build());

    let frame = tui::render_to_text(60, 160);
    let neutral = tui::render_to_colors(60, 160);

    // `Tab` lands on the palette's chosen radio; `ArrowRight` chooses the next palette.
    tui::trigger_key("Tab");
    tui::trigger_key("ArrowRight");

    let indigo = tui::render_to_colors(60, 160);

    // The next `Tab` is the text section's link, which `Enter` opens.
    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    assert_eq!(tui::take_opened().as_deref(), Some("https://example.com"));

    // The next `Tab` is the primary button.
    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    let clicked = tui::render_to_text(60, 160);

    // Back on the radio group, a third palette redefines the tokens beneath the click.
    tui::trigger_key("Shift+Tab");
    tui::trigger_key("Shift+Tab");
    tui::trigger_key("ArrowRight");

    (
      frame,
      clicked,
      tui::render_to_text(60, 160),
      neutral,
      indigo,
    )
  });

  for title in [
    "GALLERY",
    "Text",
    "Button",
    "Checkbox & Radio",
    "TextField, TextEditor, Slider & Picker",
    "BorderGlyphs",
    "Grid, Stack & Overlay",
    "LazyColumn",
    "Third party",
  ] {
    assert!(
      frame.contains(title),
      "section {title} did not paint:\n{frame}"
    );
  }

  assert!(frame.contains('╭'), "rounded borders did not paint");
  assert!(frame.contains('║'), "double borders did not paint");
  assert!(frame.contains('+'), "ascii borders did not paint");
  assert!(frame.contains("Row 0"), "the lazy column did not paint");
  assert!(
    frame
      .lines()
      .any(|line| line.contains("acme-ui") && line.contains("a badge")),
    "the badge did not paint one row tall beside its caption:\n{frame}"
  );
  assert!(frame.contains('█'), "the sparkline did not paint");
  assert!(
    !frame.contains("Row 999"),
    "the lazy column painted beyond its viewport",
  );
  assert!(
    clicked.contains("clicked 1 time(s)"),
    "the button did not respond:\n{clicked}"
  );
  assert_ne!(
    neutral, indigo,
    "the palette switch did not re-ink the frame"
  );
  assert!(
    switched.contains("clicked 1 time(s)"),
    "the palette switch lost the click count:\n{switched}"
  );
}
