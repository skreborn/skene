//! What the realization tells a harness about itself, through the environment, at no cost
//! where nothing asks:
//!
//! - `SKENE_TRACE=1`: a line on standard error per frame (`[frame] ...` with its costs), per
//!   state reported (`[state]`), focus move (`[focus]`), activation, change, key and scroll,
//!   and per window event the backends log;
//! - `SKENE_SHOT=<path>`: every presented frame written there as a PNG (through a temporary
//!   file and a rename, so a reader never sees half a file), and `SKENE_SHOT_POPUP=<path>`
//!   every frame of a picker's open list;
//! - `SKENE_PROBE=<path>`: the pointer's last position in the window's geometry, as `x y`,
//!   which an out-of-process driver reads to find the window on a compositor that does not
//!   say where it put it.

use std::{
  cell::{Cell, OnceCell},
  path::PathBuf,
  time::Duration,
};

use skene_foundation::{
  State,
  layout::Rect,
  seam::{
    content::{Content, PropValue},
    report::{Answer, KeyEvent},
  },
};

use crate::node::Node;

thread_local! {
  static ON: OnceCell<bool> = const { OnceCell::new() };
  static SHOT: OnceCell<Option<PathBuf>> = const { OnceCell::new() };
  static PROBE: OnceCell<Option<PathBuf>> = const { OnceCell::new() };
  static LAST_POINTER: Cell<(u64, Option<(f32, f32)>)> = const { Cell::new((0, None)) };
  static FRAMES: Cell<u64> = const { Cell::new(0) };
}

/// Whether tracing is on.
pub fn on() -> bool {
  ON.with(|on| {
    *on.get_or_init(|| std::env::var_os("SKENE_TRACE").is_some_and(|value| value != "0"))
  })
}

/// Logs `line` where tracing is on.
pub fn log(line: impl FnOnce() -> String) {
  if on() {
    eprintln!("{}", line());
  }
}

fn describe(node: &Node) -> String {
  let text = node
    .text
    .borrow()
    .as_ref()
    .and_then(|cell| cell.value())
    .map(|text| format!(" {text:?}"))
    .unwrap_or_default();

  format!("{:?}#{:?}{text}", node.spec.kind, node.id.get())
}

/// What one frame cost.
pub struct FrameCost {
  /// Giving paragraphs their texts and styles.
  pub prepare: Duration,
  /// Laying the tree out.
  pub layout: Duration,
  /// Recording the paths.
  pub record: Duration,
  /// Rasterizing them.
  pub raster: Duration,
  /// Compositing the glyphs.
  pub glyphs: Duration,
  /// All of it.
  pub total: Duration,
  /// Whether the whole window was painted.
  pub full: bool,
  /// The part painted, in window pixels.
  pub region: Rect,
}

/// Logs a frame's costs.
pub fn frame(cost: FrameCost) {
  let count = FRAMES.with(|frames| {
    frames.set(frames.get() + 1);
    frames.get()
  });

  log(|| {
    let ms = |duration: Duration| duration.as_secs_f64() * 1000.;

    format!(
      "[frame] {count} total {:.2} ms prepare {:.2} layout {:.2} record {:.2} raster {:.2} glyphs {:.2} {} region {:.0}x{:.0}+{:.0}+{:.0} measured {}",
      ms(cost.total),
      ms(cost.prepare),
      ms(cost.layout),
      ms(cost.record),
      ms(cost.raster),
      ms(cost.glyphs),
      if cost.full { "full" } else { "damage" },
      cost.region.width,
      cost.region.height,
      cost.region.x,
      cost.region.y,
      crate::layout::measured(),
    )
  });
}

/// Logs a state reported to the shared half.
pub fn state(node: &Node, state: State, holds: bool) {
  log(|| format!("[state] {} {state:?} {holds}", describe(node)));
}

/// Logs a focus move.
pub fn focus(node: &Node) {
  log(|| format!("[focus] {}", describe(node)));
}

/// Logs an activation.
pub fn activate(node: &Node) {
  log(|| format!("[activate] {}", describe(node)));
}

/// Logs a change reported and its answer.
pub fn change(node: &Node, content: Content, proposed: &PropValue, answer: &Answer) {
  log(|| {
    format!(
      "[change] {} {content:?} {proposed:?} -> {answer:?}",
      describe(node)
    )
  });
}

/// Logs a key dispatched.
pub fn key(event: &KeyEvent, target: &Node) {
  log(|| {
    format!(
      "[key] {:?} {:?} {}{} to {}",
      event.key,
      event.code,
      if event.down { "down" } else { "up" },
      if event.repeat { " repeat" } else { "" },
      describe(target)
    )
  });
}

/// Logs a scroll.
pub fn scroll(node: &Node, offset: (f32, f32)) {
  log(|| {
    format!(
      "[scroll] {} to {:.1},{:.1}",
      describe(node),
      offset.0,
      offset.1
    )
  });
}

/// Writes a presented frame where `SKENE_SHOT` asks.
pub fn shot(pixmap: &vello_cpu::Pixmap) {
  let Some(path) = SHOT.with(|shot| {
    shot
      .get_or_init(|| {
        std::env::var_os("SKENE_SHOT")
          .filter(|path| !path.is_empty())
          .map(PathBuf::from)
      })
      .clone()
  }) else {
    return;
  };

  write_png(&path, pixmap);
}

/// Writes a presented frame of a picker's open list where `SKENE_SHOT_POPUP` asks.
pub fn shot_popup(pixmap: &vello_cpu::Pixmap) {
  if let Some(path) = std::env::var_os("SKENE_SHOT_POPUP").filter(|path| !path.is_empty()) {
    write_png(std::path::Path::new(&path), pixmap);
  }
}

/// Writes `pixmap` as an RGBA PNG at `path` (unpremultiplied), through a temporary file and a
/// rename.
pub fn write_png(path: &std::path::Path, pixmap: &vello_cpu::Pixmap) {
  let temporary = path.with_extension("png.tmp");
  let Ok(file) = std::fs::File::create(&temporary) else {
    return;
  };
  let mut encoder = png::Encoder::new(
    std::io::BufWriter::new(file),
    u32::from(pixmap.width()),
    u32::from(pixmap.height()),
  );

  encoder.set_color(png::ColorType::Rgba);
  encoder.set_depth(png::BitDepth::Eight);

  let Ok(mut writer) = encoder.write_header() else {
    return;
  };
  let data: Vec<u8> = pixmap
    .data()
    .iter()
    .flat_map(|pixel| {
      let alpha = pixel.a;
      let unpremultiply = |channel: u8| {
        if alpha == 0 {
          0
        } else {
          ((u32::from(channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
        }
      };

      [
        unpremultiply(pixel.r),
        unpremultiply(pixel.g),
        unpremultiply(pixel.b),
        alpha,
      ]
    })
    .collect();

  if writer.write_image_data(&data).is_ok() {
    drop(writer);
    let _ = std::fs::rename(&temporary, path);
  }
}

/// Records the pointer's position in the window's geometry, and writes it where
/// `SKENE_PROBE` asks.
pub fn pointer(x: f32, y: f32) {
  LAST_POINTER.with(|last| last.set((last.get().0 + 1, Some((x, y)))));

  let Some(path) = PROBE.with(|probe| {
    probe
      .get_or_init(|| {
        std::env::var_os("SKENE_PROBE")
          .filter(|path| !path.is_empty())
          .map(PathBuf::from)
      })
      .clone()
  }) else {
    return;
  };
  let temporary = path.with_extension("tmp");

  if std::fs::write(&temporary, format!("{x} {y}\n")).is_ok() {
    let _ = std::fs::rename(&temporary, &path);
  }
}

/// How many times the pointer was seen over a window of this thread, and where it was seen
/// last, in the window's geometry.
pub fn last_pointer() -> (u64, Option<(f32, f32)>) {
  LAST_POINTER.with(Cell::get)
}
