//! What the realization tells a harness about itself, through the environment, at no cost
//! where nothing asks:
//!
//! - `SKENE_TRACE=1`: a line on standard error per frame (`[frame] ...` with its costs), per
//!   state reported (`[state]`), focus move (`[focus]`), activation, change, key and scroll,
//!   and per window event the window systems' sides log;
//! - `SKENE_SHOT=<path>`: every presented frame written there as a PNG (through a temporary
//!   file and a rename, so a reader never sees half a file), and `SKENE_SHOT_POPUP=<path>`
//!   every frame of an open overlay (a picker's list);
//! - `SKENE_PROBE=<path>`: the pointer's last position in the window's geometry, as `x y`,
//!   which an out-of-process driver reads to find the window on a compositor that does not
//!   say where it put it.
//!
//! A test records the lines in process instead ([`set_recording`], [`take_recorded`]). No line shows
//! a secure entry's text: a value, a change, a typed key, a command or an input method's
//! text bound for one is shown as a mark per cluster, or left out.

use std::{
  cell::{Cell, OnceCell, RefCell},
  path::PathBuf,
  time::Duration,
};

use skene_foundation::{
  State,
  layout::Rect,
  seam::{
    content::Verdict,
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
  static NODES: OnceCell<bool> = const { OnceCell::new() };
  static RECORDED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Whether tracing is on: `SKENE_TRACE` asks for it, or a test records.
pub fn on() -> bool {
  RECORDED.with(|recorded| recorded.borrow().is_some()) || asked()
}

/// Whether `SKENE_TRACE` asks for the lines on standard error.
fn asked() -> bool {
  ON.with(|on| {
    *on.get_or_init(|| std::env::var_os("SKENE_TRACE").is_some_and(|value| value != "0"))
  })
}

/// Keeps every line logged on this thread from now on where `record`, for
/// [`take_recorded`]; stops and drops what was kept otherwise.
pub fn set_recording(record: bool) {
  RECORDED.with(|recorded| *recorded.borrow_mut() = record.then(Vec::new));
}

/// The lines kept since the last take, while recording.
pub fn take_recorded() -> Vec<String> {
  RECORDED.with(|recorded| {
    recorded
      .borrow_mut()
      .as_mut()
      .map(std::mem::take)
      .unwrap_or_default()
  })
}

/// `answer` as a trace line shows it: a replacement's value left out where it is bound for a
/// secure entry.
pub(crate) fn answer(answer: &Answer, secure: bool) -> String {
  match answer {
    Answer::Verdict(Verdict::Replace(_)) if secure => "Verdict(Replace(..))".to_owned(),
    other => format!("{other:?}"),
  }
}

/// Whether the frames that lay out count the nodes (`SKENE_TRACE_NODES=1`, with tracing on).
pub(crate) fn nodes_on() -> bool {
  on()
    && NODES.with(|nodes| {
      *nodes.get_or_init(|| std::env::var_os("SKENE_TRACE_NODES").is_some_and(|value| value != "0"))
    })
}

/// Logs how many nodes the scene realizes and shows (`scene`) and how many a reader finds
/// (`reader`, the window aside).
pub(crate) fn nodes(scene: usize, reader: usize) {
  log(|| format!("[nodes] scene {scene} reader {reader}"));
}

/// Logs `line` where tracing is on.
pub fn log(line: impl FnOnce() -> String) {
  if !on() {
    return;
  }

  let line = line();

  RECORDED.with(|recorded| {
    if let Some(lines) = recorded.borrow_mut().as_mut() {
      lines.push(line.clone());
    }
  });

  if asked() {
    eprintln!("{line}");
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
pub(crate) struct FrameCost {
  /// Giving paragraphs their texts and styles.
  pub prepare: Duration,
  /// Laying the tree out.
  pub layout: Duration,
  /// Recording the scene, the glyphs placed included.
  pub record: Duration,
  /// Rasterizing it.
  pub raster: Duration,
  /// Placing the glyphs (finding or drawing their masks), within the recording.
  pub glyphs: Duration,
  /// The glyph masks the rasterizer's cache answered and those FreeType drew, so far.
  pub masks: (u64, u64),
  /// The glyph images found cached and those made, so far.
  pub images: (u64, u64),
  /// The shadows whose images were found cached and those whose images were made, so far.
  pub shadows: (u64, u64),
  /// All of it.
  pub total: Duration,
  /// Whether the whole window was painted.
  pub full: bool,
  /// The part painted, in window pixels.
  pub region: Rect,
}

/// Logs a frame's costs.
pub(crate) fn frame(cost: FrameCost) {
  let count = FRAMES.with(|frames| {
    frames.set(frames.get() + 1);
    frames.get()
  });

  log(|| {
    let ms = |duration: Duration| duration.as_secs_f64() * 1000.;

    format!(
      "[frame] {count} total {:.2} ms prepare {:.2} layout {:.2} record {:.2} raster {:.2} glyphs {:.2} {} region {:.0}x{:.0}+{:.0}+{:.0} measured {} masks {}/{} images {}/{} shadows {}/{}",
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
      cost.masks.0,
      cost.masks.1,
      cost.images.0,
      cost.images.1,
      cost.shadows.0,
      cost.shadows.1,
    )
  });
}

/// Logs a state reported to the shared half.
pub(crate) fn state(node: &Node, state: State, holds: bool) {
  log(|| format!("[state] {} {state:?} {holds}", describe(node)));
}

/// Logs a focus move.
pub(crate) fn focus(node: &Node) {
  log(|| format!("[focus] {}", describe(node)));
}

/// Logs an activation.
pub(crate) fn activate(node: &Node) {
  log(|| format!("[activate] {}", describe(node)));
}

/// Logs a key dispatched with the modifiers held: one that types a character into a secure
/// entry as a mark, its physical key left out.
pub(crate) fn key(event: &KeyEvent, target: &Node) {
  log(|| {
    let typed =
      unicode_segmentation::UnicodeSegmentation::graphemes(event.key.as_str(), true).count() == 1;
    let (key, code) = if typed && target.contents.borrow().secure {
      ("•", "")
    } else {
      (event.key.as_str(), event.code.as_str())
    };

    let held = event.modifiers;
    let held = [
      (held.control, "Control+"),
      (held.alt, "Alt+"),
      (held.shift, "Shift+"),
      (held.meta, "Meta+"),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, name)| *name)
    .collect::<String>();

    format!(
      "[key] {held}{key:?} {code:?} {}{} to {}",
      if event.down { "down" } else { "up" },
      if event.repeat { " repeat" } else { "" },
      describe(target)
    )
  });
}

/// Logs a scroll.
pub(crate) fn scroll(node: &Node, offset: (f32, f32)) {
  log(|| {
    format!(
      "[scroll] {} to {:.1},{:.1}",
      describe(node),
      offset.0,
      offset.1
    )
  });
}

/// Where `SKENE_SHOT` asks for presented frames to be written.
fn shot_path() -> Option<PathBuf> {
  SHOT.with(|shot| {
    shot
      .get_or_init(|| {
        std::env::var_os("SKENE_SHOT")
          .filter(|path| !path.is_empty())
          .map(PathBuf::from)
      })
      .clone()
  })
}

/// Whether `SKENE_SHOT` asks for presented frames: what a GPU frame is read back for.
pub(crate) fn shots_on() -> bool {
  shot_path().is_some()
}

/// Writes a presented frame where `SKENE_SHOT` asks.
pub(crate) fn shot(pixmap: &vello_cpu::Pixmap) {
  let Some(path) = shot_path() else {
    return;
  };

  write_png(&path, pixmap);
}

/// The file `SKENE_SHOT_POPUP` names for the open overlay's presented frames, where it names
/// one.
fn popup_shot_path() -> Option<std::path::PathBuf> {
  std::env::var_os("SKENE_SHOT_POPUP")
    .filter(|path| !path.is_empty())
    .map(std::path::PathBuf::from)
}

/// Whether the open overlay's presented frames are written ([`shot_popup`]).
pub(crate) fn popup_shots_on() -> bool {
  popup_shot_path().is_some()
}

/// Writes a presented frame of the open overlay where `SKENE_SHOT_POPUP` asks.
pub(crate) fn shot_popup(pixmap: &vello_cpu::Pixmap) {
  if let Some(path) = popup_shot_path() {
    write_png(&path, pixmap);
  }
}

/// Writes `pixmap` as an RGBA PNG at `path` (unpremultiplied), through a temporary file and a
/// rename.
pub(crate) fn write_png(path: &std::path::Path, pixmap: &vello_cpu::Pixmap) {
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
pub(crate) fn pointer(x: f32, y: f32) {
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
pub(crate) fn last_pointer() -> (u64, Option<(f32, f32)>) {
  LAST_POINTER.with(Cell::get)
}
