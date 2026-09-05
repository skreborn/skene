//! What the render tests share: the serialized driver install, and the probes reading
//! a frame back.

#![allow(dead_code)]

use skene_tui::{self as tui, Color};

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
pub fn with_driver<R>(body: impl FnOnce() -> R) -> R {
  let _serialized = DRIVER
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let _installed = Installed;

  body()
}

/// The column and row at which `needle` first appears in a rendered frame.
pub fn locate(frame: &str, needle: &str) -> (u16, u16) {
  for (row, line) in frame.lines().enumerate() {
    if let Some(column) = line.find(needle) {
      let column = line[..column].chars().count();

      return (
        u16::try_from(column).expect("a narrow frame"),
        u16::try_from(row).expect("a short frame"),
      );
    }
  }

  panic!("{needle:?} did not paint in:\n{frame}");
}

/// A cell's foreground and background.
pub type CellColors = (Option<(u8, u8, u8)>, Option<(u8, u8, u8)>);

/// The foreground and background of the cell (`x`, `y`) in a `width` x `height` frame.
pub fn colors_at(width: u16, height: u16, x: usize, y: usize) -> CellColors {
  tui::render_to_colors(width, height)[y * usize::from(width) + x]
}

pub fn red() -> Color {
  Color::srgb(1., 0., 0.)
}

pub fn green() -> Color {
  Color::srgb(0., 1., 0.)
}

pub fn blue() -> Color {
  Color::srgb(0., 0., 1.)
}

pub fn white() -> Color {
  Color::srgb(1., 1., 1.)
}

pub fn black() -> Color {
  Color::srgb(0., 0., 0.)
}

/// The RGB triple of `color`, as the frame reports it.
pub fn rgb(color: Color) -> (u8, u8, u8) {
  let [red, green, blue, _] = color.to_rgba();
  let channel = |value: f32| {
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let quantized = (value.clamp(0., 1.) * 255.).round() as u8;

    quantized
  };

  (channel(red), channel(green), channel(blue))
}
