//! The conformance suite on Linux and skene-gui-linux's own tests, every one through the driver
//! on the display runner's session: a window shown on the leg's display server, read as a reader
//! reads it (AT-SPI) and as the screen shows it, acted on by real input.

mod chrome;
mod contents;
mod desktop;
mod dnd;
mod focus;
mod gestures;
mod glyphs;
mod gpu;
mod handles;
mod hits;
mod lazy;
mod menu;
mod popups;
mod presented;
mod reader;
mod roles;
mod rows;
mod scale;
mod scroll;
mod secure;
mod selection;
mod suite;

use skene_foundation::{
  NodeId,
  seam::{node::NodeSpec, record::Record, style::ScopeChain, tree},
};
use skene_gui::testing::Screenshot;
use skene_gui_linux::Linux;

/// A node of `spec` with `record`, built through the seam as a facade builds it.
fn node(spec: NodeSpec, record: Record) -> NodeId {
  let id = tree::create::<Linux>(spec, &ScopeChain::root(), None);

  tree::set_record::<Linux>(id, record);

  id
}

/// The bounding box of the pixels that differ between `before` and `after` (shots of one
/// rectangle at `rect` in the pointer's space) by more than a faint shade (a light gray on the
/// window's near white is more), in the pointer's space; `None` where none do.
fn changed(
  rect: (f64, f64, f64, f64),
  before: &Screenshot,
  after: &Screenshot,
) -> Option<(f64, f64, f64, f64)> {
  let (width, height) = (before.width as usize, before.height as usize);
  let mut bounds: Option<(usize, usize, usize, usize)> = None;

  for row in 0..height.min(after.height as usize) {
    for column in 0..width.min(after.width as usize) {
      let at = (row * width + column) * 4;
      let there = (row * after.width as usize + column) * 4;
      let differs =
        (0..3).any(|channel| before.rgba[at + channel].abs_diff(after.rgba[there + channel]) > 8);

      if differs {
        bounds = Some(match bounds {
          None => (column, row, column, row),
          Some((left, top, right, bottom)) => (
            left.min(column),
            top.min(row),
            right.max(column),
            bottom.max(row),
          ),
        });
      }
    }
  }

  bounds.map(|(left, top, right, bottom)| {
    let (x, y) = (rect.2 / width as f64, rect.3 / height as f64);

    (
      rect.0 + left as f64 * x,
      rect.1 + top as f64 * y,
      (right + 1 - left) as f64 * x,
      (bottom + 1 - top) as f64 * y,
    )
  })
}

/// Turns the window's loop until the probe `name` reads `wanted`, for up to two seconds; what
/// it reads then.
fn probe_until(screen: &skene_gui::testing::Screen, name: &str, wanted: &str) -> String {
  use skene_gui::testing::Driver as _;

  let start = std::time::Instant::now();

  loop {
    let read = screen.probe(name);

    if read == wanted || start.elapsed() > std::time::Duration::from_secs(2) {
      return read;
    }

    screen.driver().wait(std::time::Duration::from_millis(20));
  }
}

/// The name of what a reader finds focused, the window itself read as `<none>`.
fn focused_name(screen: &skene_gui::testing::Screen) -> String {
  use skene_gui::testing::{Driver as _, Query};

  let mut driver = screen.driver();
  let found = driver.find_now(&Query::Focused);

  found
    .first()
    .map(|found| driver.read(found))
    .filter(|read| !read.name.starts_with("skene-test"))
    .map_or_else(|| "<none>".to_owned(), |read| read.name.to_string())
}
