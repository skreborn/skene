//! skene-gui-linux's integration tests, every one through the driver on the display runner's
//! session: a window shown on the leg's display server, read as a reader reads it (AT-SPI)
//! and as the screen shows it, acted on by real input.

mod chrome;
mod contents;
mod desktop;
mod dnd;
mod gestures;
mod glyphs;
#[cfg(feature = "gpu")]
mod gpu;
mod handles;
mod lazy;
mod menu;
mod presented;
mod reader;
mod roles;
mod rows;
mod scale;
mod scroll;
mod secure;
mod selection;

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
