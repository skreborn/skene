//! What every probe shares: running a body on the UI thread, showing a view in a window,
//! settling, real input through `xdotool` at the bounds GTK laid out, the window's pixels, and
//! a screen reader's view of the app over AT-SPI (`atspi-dump`, run beside the app while its
//! main loop answers).
//!
//! Every check prints `RESULT <name>: PASS|FAIL <detail>`; a probe exits with failure where
//! one failed.

use std::{
  fmt::Display,
  process::{Command, ExitCode},
  sync::atomic::{AtomicU32, Ordering},
  time::{Duration, Instant},
};

use skene_gui::{
  __private::{present, settle as settle_for},
  NodeHandle, View,
  platform::linux::gtk::{self, gdk, glib, graphene, prelude::*},
};

/// The application name the probe's windows are shown under.
const APPLICATION: &str = "skene-probe";

static FAILURES: AtomicU32 = AtomicU32::new(0);
static CHECKS: AtomicU32 = AtomicU32::new(0);

/// Runs a probe's `body` on the UI thread and reports how many checks failed.
pub fn run(name: &'static str, body: fn()) -> ExitCode {
  skene_gui::__private::test(name, body);

  let (checks, failures) = (
    CHECKS.load(Ordering::SeqCst),
    FAILURES.load(Ordering::SeqCst),
  );

  println!(
    "SUMMARY {name}: {} of {checks} checks passed",
    checks - failures
  );

  if failures == 0 {
    ExitCode::SUCCESS
  } else {
    ExitCode::FAILURE
  }
}

/// Prints one check's outcome.
pub fn result(name: &str, passed: bool, detail: impl Display) {
  CHECKS.fetch_add(1, Ordering::SeqCst);

  if !passed {
    FAILURES.fetch_add(1, Ordering::SeqCst);
  }

  println!(
    "RESULT {name}: {} {detail}",
    if passed { "PASS" } else { "FAIL" }
  );
}

/// Runs the main loop until one 40 ms window passes with no frame and no report.
pub fn settle() {
  settle_for(Duration::from_millis(40));
}

/// Runs the main loop for `milliseconds`.
pub fn wait(milliseconds: u64) {
  let until = Instant::now() + Duration::from_millis(milliseconds);
  let context = glib::MainContext::default();

  while Instant::now() < until {
    while context.pending() {
      context.iteration(false);
    }

    std::thread::sleep(Duration::from_millis(1));
  }
}

/// A view shown in a window.
pub struct Shown {
  /// The window.
  pub window: gtk::Window,
  origin: (i32, i32),
}

/// Shows `view` in a new window `width` by `height` pixels (`-1` for its natural height),
/// settled.
pub fn show<V: View>(width: i32, height: i32, view: impl FnOnce() -> V) -> Shown {
  if glib::application_name().is_none() {
    glib::set_application_name(APPLICATION);
  }

  let window = present(view, width, height);

  settle();

  let origin = origin(&window);

  Shown { window, origin }
}

fn origin(window: &gtk::Window) -> (i32, i32) {
  let title = format!("probe-{:p}", window.as_ptr());

  window.set_title(Some(&title));
  settle();

  let output = Command::new("xdotool")
    .args(["search", "--sync", "--name", &title, "getwindowgeometry"])
    .output()
    .expect("xdotool runs");
  let text = String::from_utf8_lossy(&output.stdout);
  let position = text
    .lines()
    .find_map(|line| line.trim().strip_prefix("Position: "))
    .and_then(|rest| rest.split_whitespace().next())
    .unwrap_or("0,0");
  let (x, y) = position.split_once(',').unwrap_or(("0", "0"));

  (x.parse().unwrap_or(0), y.parse().unwrap_or(0))
}

/// The widget realizing the node `handle` captured.
///
/// # Panics
///
/// Where the handle captured no node, or the node has no widget.
#[must_use]
pub fn widget(handle: &NodeHandle) -> gtk::Widget {
  skene_gui::__private::widget(handle.node().expect("the handle captured a node"))
    .expect("the node has a widget")
}

impl Shown {
  /// `widget`'s bounds in the window: `(x, y, width, height)`.
  ///
  /// # Panics
  ///
  /// Where the widget is not in the window.
  #[must_use]
  pub fn bounds(&self, widget: &gtk::Widget) -> (f32, f32, f32, f32) {
    let bounds = widget
      .compute_bounds(&self.window)
      .unwrap_or_else(|| graphene::Rect::new(-1., -1., 0., 0.));

    (bounds.x(), bounds.y(), bounds.width(), bounds.height())
  }

  /// The bounds of the node `handle` captured, in the window's content (its root box).
  #[must_use]
  pub fn rect(&self, handle: &NodeHandle) -> (f32, f32, f32, f32) {
    let content = self.window.child().expect("the window shows the root box");
    let bounds = widget(handle)
      .compute_bounds(&content)
      .unwrap_or_else(|| graphene::Rect::new(-1., -1., 0., 0.));

    (bounds.x(), bounds.y(), bounds.width(), bounds.height())
  }

  /// Closes the window.
  pub fn close(self) {
    self.window.destroy();
    settle();
  }

  /// The screen point at `(x, y)` in the window.
  fn screen(&self, x: f32, y: f32) -> (i32, i32) {
    let (dx, dy) = self.window.surface_transform();

    (
      self.origin.0 + (dx + f64::from(x)) as i32,
      self.origin.1 + (dy + f64::from(y)) as i32,
    )
  }

  /// The screen point at `widget`'s center.
  #[must_use]
  pub fn center(&self, widget: &gtk::Widget) -> (i32, i32) {
    let (x, y, width, height) = self.bounds(widget);

    self.screen(x + width / 2., y + height / 2.)
  }

  /// Moves the pointer over the window's point `(x, y)`.
  pub fn hover_at(&self, x: f32, y: f32) {
    let (x, y) = self.screen(x, y);

    xdotool(&["mousemove", &(x + 1).to_string(), &(y + 1).to_string()]);
    xdotool(&["mousemove", &x.to_string(), &y.to_string()]);
  }

  /// Moves the pointer over `widget`'s center.
  pub fn hover(&self, widget: &gtk::Widget) {
    let (x, y, width, height) = self.bounds(widget);

    self.hover_at(x + width / 2., y + height / 2.);
  }

  /// Clicks `widget`'s center with the primary button.
  pub fn click(&self, widget: &gtk::Widget) {
    self.hover(widget);
    xdotool(&["click", "1"]);
  }

  /// Clicks the window's point `(x, y)`.
  pub fn click_at(&self, x: f32, y: f32) {
    self.hover_at(x, y);
    xdotool(&["click", "1"]);
  }

  /// The window drawn now, through its own renderer.
  #[must_use]
  pub fn texture(&self) -> gdk::Texture {
    settle();
    skene_gui::__private::render(self.window.upcast_ref())
  }

  /// The pixel at the window's point `(x, y)`, RGBA.
  #[must_use]
  pub fn pixel(&self, x: f32, y: f32) -> [u8; 4] {
    pixel(&self.texture(), x, y)
  }

  /// The pixel at `(dx, dy)` from the top leading corner of the node `handle` captured.
  #[must_use]
  pub fn pixel_in(&self, handle: &NodeHandle, dx: f32, dy: f32) -> [u8; 4] {
    let (x, y, _, _) = self.bounds(&widget(handle));

    self.pixel(x + dx, y + dy)
  }

  /// The same, drawn now without settling: a frame in the middle of a transition.
  #[must_use]
  pub fn pixel_now(&self, handle: &NodeHandle, dx: f32, dy: f32) -> [u8; 4] {
    let (x, y, _, _) = self.bounds(&widget(handle));

    pixel(
      &skene_gui::__private::render(self.window.upcast_ref()),
      x + dx,
      y + dy,
    )
  }

  /// How many pixels of the node `handle` captured are within 40 of `color`.
  #[must_use]
  pub fn count_in(&self, handle: &NodeHandle, color: u32) -> usize {
    count(&self.texture(), self.bounds(&widget(handle)), color)
  }

  /// Writes the window's pixels to `path`.
  pub fn screenshot(&self, path: &str) {
    let _ = self.texture().save_to_png(path);
  }

  /// Resizes the window to `width` by `height`.
  pub fn resize(&self, width: i32, height: i32) {
    self.window.set_default_size(width, height);
    settle();
    wait(100);
    settle();
  }
}

/// The pixel at `(x, y)` of `texture`, RGBA, scaled to the texture's device pixels.
#[must_use]
pub fn pixel(texture: &gdk::Texture, x: f32, y: f32) -> [u8; 4] {
  let mut downloader = gdk::TextureDownloader::new(texture);

  downloader.set_format(gdk::MemoryFormat::R8g8b8a8);

  let (bytes, stride) = downloader.download_bytes();
  let (x, y) = (x.max(0.) as usize, y.max(0.) as usize);
  let offset = y * stride + x * 4;

  bytes
    .get(offset..offset + 4)
    .map_or([0; 4], |pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
}

/// How many pixels of `texture` within `(x, y, width, height)` are within 40 of `color`.
#[must_use]
pub fn count(texture: &gdk::Texture, area: (f32, f32, f32, f32), color: u32) -> usize {
  let mut downloader = gdk::TextureDownloader::new(texture);

  downloader.set_format(gdk::MemoryFormat::R8g8b8a8);

  let (bytes, stride) = downloader.download_bytes();
  let mut found = 0;

  for y in area.1.max(0.) as usize..(area.1 + area.3).max(0.) as usize {
    for x in area.0.max(0.) as usize..(area.0 + area.2).max(0.) as usize {
      let offset = y * stride + x * 4;

      if let Some(pixel) = bytes.get(offset..offset + 4)
        && near_within([pixel[0], pixel[1], pixel[2], pixel[3]], color, 40)
      {
        found += 1;
      }
    }
  }

  found
}

/// Runs `xdotool` with `arguments`, then settles.
///
/// # Panics
///
/// Where `xdotool` fails.
pub fn xdotool(arguments: &[&str]) {
  let status = Command::new("xdotool")
    .args(arguments)
    .status()
    .expect("xdotool runs");

  assert!(status.success(), "xdotool {arguments:?}");
  settle();
}

/// Presses a key chord (`"Tab"`, `"ctrl+z"`), focus where it is.
pub fn key(chord: &str) {
  xdotool(&["key", "--delay", "30", chord]);
}

/// Types `text`, focus where it is.
pub fn type_text(text: &str) {
  xdotool(&["type", "--delay", "40", text]);
}

/// `#rrggbb` of a pixel.
#[must_use]
pub fn hex(pixel: [u8; 4]) -> String {
  format!("#{:02x}{:02x}{:02x}", pixel[0], pixel[1], pixel[2])
}

/// Whether a pixel is within 3 of `color` (`0xrrggbb`) in every channel.
#[must_use]
pub fn near(pixel: [u8; 4], color: u32) -> bool {
  near_within(pixel, color, 3)
}

/// Whether a pixel is within `tolerance` of `color` (`0xrrggbb`) in every channel.
#[must_use]
pub fn near_within(pixel: [u8; 4], color: u32, tolerance: u8) -> bool {
  let expected = [(color >> 16) as u8, (color >> 8) as u8, color as u8];

  pixel
    .iter()
    .zip(expected)
    .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
}

/// A screen reader's view of this process's app: `atspi-dump`, run beside it while the main
/// loop answers it, one line per accessible.
#[must_use]
pub fn atspi() -> Vec<String> {
  settle();

  let dump = std::env::current_exe()
    .expect("the probe's path")
    .with_file_name("atspi-dump");
  // The probe's private session bus holds its app alone, which GTK 4.14 names "Unnamed" (its
  // application name is the description), so every app is dumped.
  let mut child = Command::new(dump)
    .arg(std::env::var("ATSPI_FILTER").unwrap_or_default())
    .stdout(std::process::Stdio::piped())
    .spawn()
    .expect("atspi-dump runs");
  let context = glib::MainContext::default();
  let started = Instant::now();

  loop {
    while context.pending() {
      context.iteration(false);
    }

    if child.try_wait().ok().flatten().is_some() || started.elapsed() > Duration::from_secs(30) {
      break;
    }

    std::thread::sleep(Duration::from_millis(2));
  }

  let output = child.wait_with_output().expect("atspi-dump ends");

  String::from_utf8_lossy(&output.stdout)
    .lines()
    .map(|line| line.trim().to_owned())
    .collect()
}

/// The first line of `dump` that contains every one of `parts`.
#[must_use]
pub fn line<'a>(dump: &'a [String], parts: &[&str]) -> Option<&'a str> {
  dump
    .iter()
    .find(|line| parts.iter().all(|part| line.contains(part)))
    .map(String::as_str)
}
