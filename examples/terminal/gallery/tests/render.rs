//! The gallery rendered headlessly: every section paints and the controls respond.

#![cfg(not(target_arch = "wasm32"))]

use skene_terminal::{
  self as terminal,
  prelude::*,
  testing::{self, with_platform},
};

#[test]
fn the_gallery_renders_and_responds() {
  let (frame, clicked, switched, neutral, indigo) = with_platform(|| {
    let _ = terminal::mount(|| terminal_gallery::app().build());

    let frame = testing::render_to_text(60, 220);
    let neutral = testing::render_to_colors(60, 220);

    // `Tab` lands on the palette's chosen radio; `ArrowRight` chooses the next palette.
    testing::trigger_key("Tab");
    testing::trigger_key("ArrowRight");

    let indigo = testing::render_to_colors(60, 220);

    // The next `Tab` is the text section's link, which `Enter` opens.
    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    assert_eq!(
      testing::take_opened().as_deref(),
      Some("https://example.com")
    );

    // The next `Tab` is the primary button.
    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    let clicked = testing::render_to_text(60, 220);

    // Back on the radio group, a third palette redefines the tokens beneath the click.
    testing::trigger_key("Shift+Tab");
    testing::trigger_key("Shift+Tab");
    testing::trigger_key("ArrowRight");

    (
      frame,
      clicked,
      testing::render_to_text(60, 220),
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
    "Context menu",
    "Show & Switch",
    "Suspense & ErrorBoundary",
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
