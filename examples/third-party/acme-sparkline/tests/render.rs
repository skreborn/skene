//! The sparkline rendered headlessly: one bar per value, scaled to the range, in the
//! color it was given.

#![cfg(not(target_arch = "wasm32"))]

use acme_sparkline::Sparkline;
use skene_tui::{self as tui, Color, View as _};

/// Serializes the tests: the UI-thread designation is process-wide.
static PLATFORM: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn a_sparkline_paints_scaled_bars() {
  let _serialized = PLATFORM
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let _ = tui::mount(|| {
    Sparkline::new([0., 0.5, 1., 0.25])
      .set_color(Color::srgb(1., 0., 0.))
      .build()
  });

  let frame = tui::render_to_text(10, 2);
  let colors = tui::render_to_colors(10, 2);

  tui::uninstall();

  assert_eq!(frame.lines().next(), Some("▁▅█▃"), "{frame}");
  assert_eq!(colors[0].0, Some((255, 0, 0)));
  assert_eq!(colors[3].0, Some((255, 0, 0)));
  assert_eq!(colors[4].0, None);
}
