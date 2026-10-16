//! Start-up and memory of the tuned engine's fonts (option C: Parley over Core Text's fonts,
//! drawn by `CTFontDrawGlyphs`) under each [`Storage`], in a fresh process: the time from `main`
//! to the first Latin, the first Chinese and the first emoji label, each laid out and drawn, with
//! the bytes registered and the process's footprint after each; then the whole corpus, which
//! registers every font an eager start would, with each variant's lines so storages can be
//! compared for layout; and how each font was stored.
//!
//! Registration is lazy throughout: a font is registered when a label first needs it.

use std::time::Instant;

use serde_json::{Value, json};
use text_parity::corpus::{Corpus, Variant};

use crate::{
  cg::{Canvas, Ground, Smoothing},
  coretext,
  probes,
  raster::Raster,
  sets,
  tuned::{Storage, TunedEngine},
};

/// `task_vm_info` (`<mach/task_info.h>`, revision 3), as far as this probe reads it.
#[repr(C, packed(4))]
#[derive(Default)]
struct TaskVmInfo {
  virtual_size: u64,
  region_count: i32,
  page_size: i32,
  resident_size: u64,
  resident_size_peak: u64,
  device: u64,
  device_peak: u64,
  internal: u64,
  internal_peak: u64,
  external: u64,
  external_peak: u64,
  reusable: u64,
  reusable_peak: u64,
  purgeable_volatile_pmap: u64,
  purgeable_volatile_resident: u64,
  purgeable_volatile_virtual: u64,
  compressed: u64,
  compressed_peak: u64,
  compressed_lifetime: u64,
  phys_footprint: u64,
  min_address: u64,
  max_address: u64,
  ledger_phys_footprint_peak: i64,
}

const TASK_VM_INFO: u32 = 22;

/// The process's memory as the kernel accounts it: `phys_footprint` (what jetsam reads),
/// resident, internal (anonymous) and external (file-backed) bytes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Footprint {
  /// `phys_footprint`.
  pub footprint: u64,
  /// `resident_size`.
  pub resident: u64,
  /// `internal`: anonymous memory.
  pub internal: u64,
  /// `external`: file-backed memory.
  pub external: u64,
  /// `compressed`.
  pub compressed: u64,
}

impl Footprint {
  /// This process's, now.
  #[must_use]
  pub fn now() -> Self {
    let mut info = TaskVmInfo::default();
    let mut count = (size_of::<TaskVmInfo>() / 4) as libc::mach_msg_type_number_t;

    unsafe extern "C" {
      static mach_task_self_: libc::mach_port_t;
    }

    // SAFETY: `info` is as large as `count` says.
    let status = unsafe {
      libc::task_info(
        mach_task_self_,
        TASK_VM_INFO,
        std::ptr::from_mut(&mut info).cast(),
        &mut count,
      )
    };

    if status != 0 {
      return Self::default();
    }

    Self {
      footprint: info.phys_footprint,
      resident: info.resident_size,
      internal: info.internal,
      external: info.external,
      compressed: info.compressed,
    }
  }

  fn json(self, start: Self) -> Value {
    let mb = |bytes: u64| bytes as f64 / 1e6;
    let delta = |now: u64, then: u64| (now as f64 - then as f64) / 1e6;

    json!({
      "phys_footprint_mb": mb(self.footprint),
      "phys_footprint_delta_mb": delta(self.footprint, start.footprint),
      "resident_mb": mb(self.resident),
      "resident_delta_mb": delta(self.resident, start.resident),
      "internal_delta_mb": delta(self.internal, start.internal),
      "external_delta_mb": delta(self.external, start.external),
      "compressed_mb": mb(self.compressed),
    })
  }
}

fn ms(since: Instant) -> f64 {
  since.elapsed().as_secs_f64() * 1e3
}

/// Lays `variant` out through `engine` and draws it with Core Text at 2x: layout and drawing
/// milliseconds, and the families asked for.
fn label(engine: &mut TunedEngine, raster: &mut Raster, variant: &Variant<'_>) -> (f64, f64, Vec<String>) {
  let started = Instant::now();
  let laid = engine.lay_out(variant);
  let layout = ms(started);
  let started = Instant::now();
  let record = engine.record(variant, &laid, None);
  let (width, height) = record.image_box(variant.width);
  let runs = sets::tuned_runs(&laid.laid, engine);
  let canvas = Canvas::for_box(width, height, 2., Ground::White, Smoothing(true));

  raster.draw(&canvas, &runs);
  std::hint::black_box(canvas.to_rgba());

  (layout, ms(started), laid.families)
}

/// One probe run under `storage`, `main` having started at `main`: the steps, the fonts as
/// stored and the corpus's lines, as JSON.
#[must_use]
pub fn run(storage: Storage, main: Instant) -> Value {
  let start = Footprint::now();
  let since_main = ms(main);
  let mut engine = probes::tuned_engine();

  engine.storage = storage;

  let mut raster = Raster::default();
  let corpus = Corpus::builtin();
  let base = corpus.case("latin-kerning").cloned().expect("the latin-kerning case");
  let mut steps = Vec::new();

  for (name, text, locale) in [
    ("latin", "Hello, world", "en"),
    ("chinese", "你好，世界。", "zh-Hans"),
    ("emoji", "😀👍🏽", "en"),
  ] {
    let mut case = base.clone();

    case.id = format!("fontmem-{name}");
    case.text = text.to_owned();
    case.locale = locale.to_owned();

    let before = (engine.registered_bytes, engine.mapped_bytes, engine.stored.len(), engine.register_ms);
    let started = Instant::now();
    let variant = Variant {
      case: &case,
      size: 17.,
      width: None,
    };
    let (layout, draw, families) = label(&mut engine, &mut raster, &variant);

    steps.push(json!({
      "step": name,
      "ms": ms(started),
      "layout_ms": layout,
      "draw_ms": draw,
      "register_ms": engine.register_ms - before.3,
      "since_main_ms": ms(main),
      "fonts_registered": engine.stored.len() - before.2,
      "copied_mb": (engine.registered_bytes - before.0) as f64 / 1e6,
      "mapped_mb": (engine.mapped_bytes - before.1) as f64 / 1e6,
      "families": families,
      "memory": Footprint::now().json(start),
    }));
  }

  let started = Instant::now();
  let before = engine.stored.len();
  let mut lines = serde_json::Map::new();

  for variant in corpus.variants() {
    let laid = engine.lay_out(&variant);
    let record = engine.record(&variant, &laid, None);

    lines.insert(
      variant.id(),
      Value::Array(
        record
          .lines
          .iter()
          .map(|line| json!([line.range.utf8.map_or(0, |range| range[0]), line.width, line.height]))
          .collect(),
      ),
    );
  }

  steps.push(json!({
    "step": "corpus",
    "ms": ms(started),
    "since_main_ms": ms(main),
    "fonts_registered": engine.stored.len() - before,
    "memory": Footprint::now().json(start),
  }));

  json!({
    "storage": storage.name(),
    "os": crate::suite::os_version(),
    "main_to_probe_ms": since_main,
    "start_phys_footprint_mb": start.footprint as f64 / 1e6,
    "steps": steps,
    "copied_mb": engine.registered_bytes as f64 / 1e6,
    "mapped_mb": engine.mapped_bytes as f64 / 1e6,
    "register_ms": engine.register_ms,
    "fonts": engine.stored,
    "rejected": engine.rejected,
    "lines": lines,
  })
}

/// One line of `run`'s answer: the storage, the bytes and each step's times and footprint.
#[must_use]
pub fn summary(json: &Value) -> String {
  let steps: Vec<String> = json["steps"]
    .as_array()
    .map(|steps| {
      steps
        .iter()
        .map(|step| {
          format!(
            "{} {:.1} ms (since main {:.1}), +{} fonts, footprint {:+.1} MB",
            step["step"].as_str().unwrap_or_default(),
            step["ms"].as_f64().unwrap_or_default(),
            step["since_main_ms"].as_f64().unwrap_or_default(),
            step["fonts_registered"],
            step["memory"]["phys_footprint_delta_mb"].as_f64().unwrap_or_default()
          )
        })
        .collect()
    })
    .unwrap_or_default();

  format!(
    "{}: copied {:.1} MB, mapped {:.1} MB, register {:.1} ms; {}",
    json["storage"].as_str().unwrap_or_default(),
    json["copied_mb"].as_f64().unwrap_or_default(),
    json["mapped_mb"].as_f64().unwrap_or_default(),
    json["register_ms"].as_f64().unwrap_or_default(),
    steps.join("; ")
  )
}

/// The tables of the fonts the first labels use (the system font, the Chinese and the emoji
/// fallback) by size, and the bytes a layout-only copy keeps of each.
#[must_use]
pub fn tables() -> Value {
  let base = crate::platform::system_font(17., 400, false);
  let base = crate::platform::as_ct(&base);
  let mut fonts = vec![("system", unsafe { objc2_core_foundation::CFRetained::retain(std::ptr::NonNull::from(base)) })];

  for (name, text, locale) in [("chinese", "你", "zh-Hans"), ("emoji", "😀", "en")] {
    let string = objc2_core_foundation::CFString::from_str(text);
    let range = objc2_core_foundation::CFRange {
      location: 0,
      length: string.length(),
    };
    let font = unsafe {
      objc2_core_text::CTFont::for_string_with_language(
        base,
        &string,
        range,
        Some(&objc2_core_foundation::CFString::from_str(locale)),
      )
    };

    fonts.push((name, font));
  }

  let out: Vec<Value> = fonts
    .iter()
    .map(|(name, font)| {
      let info = coretext::info(font);
      let sizes = coretext::table_sizes(font);
      let total: usize = sizes.iter().map(|(_, size)| size).sum();
      let layout: usize = sizes
        .iter()
        .filter(|(tag, _)| coretext::LAYOUT_TABLES.contains(&tag.as_str()))
        .map(|(_, size)| size)
        .sum();
      let file = info
        .path
        .as_ref()
        .and_then(|path| std::fs::metadata(path).ok())
        .map(|metadata| metadata.len());
      let access = info.path.as_ref().map(|path| match crate::mapped::Mapped::open(path, None) {
        Ok((_, access)) | Err(access) => access,
      });

      json!({
        "font": name, "postscript": info.postscript, "path": info.path, "file_bytes": file,
        "tables_bytes": total, "layout_bytes": layout, "tables": sizes, "map": access,
      })
    })
    .collect();

  Value::Array(out)
}
