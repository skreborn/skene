//! The sparkline rendered headlessly: one bar per value, scaled to the range, in the
//! color it was given; and the bars as pixels over the glyphs once the terminal proves
//! its graphics.

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

/// Without graphics the glyphs are the sparkline; once the terminal proves them the
/// same box shows the bars as pixels, the tallest bar reaching the top and the lowest
/// leaving it clear, and the glyphs still stand beneath.
#[test]
fn a_sparkline_upgrades_to_pixels_where_graphics_are_proven() {
  let _serialized = PLATFORM
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let _ = tui::mount(|| {
    Sparkline::new([0., 0.5, 1., 0.25])
      .set_color(Color::srgb(1., 0., 0.))
      .build()
  });

  assert!(tui::render_to_pictures(10, 2).is_empty());

  tui::trigger_graphics(true);

  let frame = tui::render_to_text(10, 2);
  let pictures = tui::render_to_pictures(10, 2);

  assert_eq!(frame.lines().next(), Some("▁▅█▃"), "{frame}");
  assert_eq!(pictures.len(), 1);

  let picture = &pictures[0];
  let width = usize::from(picture.width);
  let pixel = |x: usize, y: usize| &picture.pixels[(y * width + x) * 4..(y * width + x) * 4 + 4];

  assert_eq!((picture.columns, picture.rows), (4, 1));
  // The third bar is the tallest: its top-left pixel is inked.
  assert_eq!(pixel(width / 2, 0), &[255, 0, 0, 255]);
  // The first bar is the lowest: above it the picture is clear.
  assert_eq!(pixel(0, 0)[3], 0);
  // Every bar reaches the bottom.
  assert_eq!(pixel(0, usize::from(picture.height) - 1), &[255, 0, 0, 255]);

  tui::uninstall();
}
