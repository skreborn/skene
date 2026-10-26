//! The sparkline in the terminal's in-process tier: one bar per value, scaled to the range, in the
//! color it was given; and the bars as pixels over the glyphs once the terminal proves
//! its graphics.

#![cfg(skene_platform = "terminal")]

use acme_sparkline::Sparkline;
use skene_terminal::{
  Color,
  View as _,
  platform::terminal::{install, mount, uninstall},
  testing,
};

/// Serializes the tests: the UI-thread designation is process-wide.
static PLATFORM: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn a_sparkline_paints_scaled_bars() {
  let _serialized = PLATFORM
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  install();

  let _ = mount(|| {
    Sparkline::new([0., 0.5, 1., 0.25])
      .set_color(Color::srgb(1., 0., 0.))
      .build()
  });

  let frame = testing::render_to_text(10, 2);
  let colors = testing::render_to_colors(10, 2);

  uninstall();

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

  install();

  let _ = mount(|| {
    Sparkline::new([0., 0.5, 1., 0.25])
      .set_color(Color::srgb(1., 0., 0.))
      .build()
  });

  assert_eq!(testing::render_to_pictures(10, 2), []);

  testing::trigger_graphics(true);

  let frame = testing::render_to_text(10, 2);
  let pictures = testing::render_to_pictures(10, 2);

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

  uninstall();
}
