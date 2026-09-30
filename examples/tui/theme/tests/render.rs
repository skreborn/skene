//! The theme example rendered headlessly: every section paints, an accent switch
//! re-inks the frame, and a scheme choice re-inks it again.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::{
  self as tui,
  prelude::*,
  testing::{self, with_platform},
};

#[test]
fn the_theme_renders_and_switches() {
  let (frame, indigo, teal, dark) = with_platform(|| {
    let _ = tui::mount(|| tui_theme::app().build());

    let frame = testing::render_to_text(60, 60);
    let indigo = testing::render_to_colors(60, 60);

    // `Tab` walks the scheme buttons, then lands on the accent's chosen radio;
    // `ArrowRight` chooses teal.
    for _ in 0..4 {
      testing::trigger_key("Tab");
    }

    testing::trigger_key("ArrowRight");

    let teal = testing::render_to_colors(60, 60);

    // Back to the `Dark` button: the scheme changes and the tree stands.
    testing::trigger_key("Shift+Tab");
    testing::trigger_key("Enter");

    (frame, indigo, teal, testing::render_to_colors(60, 60))
  });

  for title in [
    "THEME",
    "System",
    "Light",
    "Dark",
    "Accent",
    "Tokens",
    "Nested theme",
  ] {
    assert!(
      frame.contains(title),
      "section {title} did not paint:\n{frame}"
    );
  }

  assert!(frame.contains('╭'), "rounded swatch borders did not paint");
  assert_ne!(indigo, teal, "the accent switch did not re-ink the frame");
  assert_ne!(teal, dark, "the scheme choice did not re-ink the frame");
}
