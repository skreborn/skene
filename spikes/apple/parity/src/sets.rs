//! The record sets of one run, each written to a directory as the harness reads it
//! (`records.json` and `png/`): A by TextKit 2, Core Text's own framesetter breaks, C from B's
//! glyph runs, B and C tuned, and the 1000-label timings of each engine.

use std::{
  collections::{BTreeMap, HashMap},
  path::{Path, PathBuf},
  sync::Arc,
  time::Instant,
};

use objc2_core_foundation::{CFAttributedString, CFRange, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGContext, CGPath};
use objc2_core_text::{CTFramesetter, CTLine};
use text_parity::{
  bench::median,
  corpus::{Case, Corpus, Variant},
  fonts::Fonts,
  layout::{Laid, Options as LayoutOptions, ParleyEngine},
  record::{
    EngineInfo,
    LabelTimings,
    Line,
    PlatformInfo,
    Range,
    Record,
    RecordSet,
    SCHEMA,
    SetTimings,
    VariantTimings,
    scale_key,
  },
  render::{self, RenderOptions},
  text::Offsets,
};

use crate::{
  cg::{Canvas, Ground, Smoothing},
  coretext,
  raster::{DrawRun, Raster, Source},
  textkit::{self, Setup},
  tuned::TunedEngine,
};

/// Where and how a run writes.
#[derive(Clone, Debug)]
pub struct Context {
  /// The run's directory.
  pub out: PathBuf,
  /// The scales images are drawn at.
  pub scales: Vec<f32>,
  /// Warm layouts per variant.
  pub repeats: usize,
  /// Only variants whose id contains this.
  pub only: Option<String>,
  /// The OS version.
  pub os_version: String,
  /// The device.
  pub device: Option<String>,
  /// Labels in the many-label timings.
  pub labels: usize,
}

impl Context {
  /// Whether `variant` is in the run.
  #[must_use]
  pub fn wants(&self, variant: &Variant<'_>) -> bool {
    self
      .only
      .as_ref()
      .is_none_or(|only| variant.id().contains(only.as_str()))
  }

  /// A set's skeleton.
  #[must_use]
  pub fn set(
    &self,
    name: &str,
    version: &str,
    rasterizer: &str,
    settings: &[(&str, &str)],
  ) -> RecordSet {
    RecordSet {
      schema: SCHEMA,
      corpus_version: Corpus::builtin().version,
      engine: EngineInfo {
        name: name.to_owned(),
        version: version.to_owned(),
        rasterizer: rasterizer.to_owned(),
        settings: settings
          .iter()
          .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
          .collect(),
      },
      platform: PlatformInfo {
        os: crate::platform::OS.to_owned(),
        os_version: self.os_version.clone(),
        device: self.device.clone(),
      },
      scales: self.scales.clone(),
      timings: SetTimings::default(),
      records: Vec::new(),
    }
  }
}

/// The path of a variant's image at a scale, relative to its set.
#[must_use]
pub fn image_path(variant: &Variant<'_>, scale: f32) -> String {
  format!("png/{}@{}x.png", variant.file_stem(), scale_key(scale))
}

fn write(image: &text_parity::image::Rgba, path: &Path) {
  if let Err(error) = image.write(path) {
    eprintln!("{error}");
  }
}

/// A: every variant by TextKit 2, its images drawn by its fragments; with it, TextKit's own
/// glyphs at its own positions drawn by `CTFontDrawGlyphs` (`a-cg`) and by vello (`a-vello`),
/// the rasterizer alone (see [`crate::isolate`]).
pub fn textkit(
  context: &Context,
  corpus: &Corpus,
  dir: &Path,
) -> (RecordSet, RecordSet, RecordSet) {
  let cg_dir = dir.with_file_name("a-cg");
  let vello_dir = dir.with_file_name("a-vello");

  for dir in [dir, &cg_dir, &vello_dir] {
    let _ = std::fs::create_dir_all(dir.join("png"));
  }

  let mut set = context.set(
    "textkit2",
    &context.os_version,
    "coregraphics (NSTextLayoutFragment drawAtPoint)",
    &[
      (
        "container",
        "lineFragmentPadding 0, standalone NSTextContentStorage",
      ),
      (
        "font",
        "systemFontOfSize:weight: (italic by descriptor trait)",
      ),
      ("strategy", "standard (keep-all adds hangulWordPriority)"),
      ("smoothing", "allowed and asked"),
    ],
  );
  let mut cg = set.clone();
  let mut vello = set.clone();
  let mut bytes = crate::isolate::Bytes::default();

  cg.engine.rasterizer = "coretext CTFontDrawGlyphs at TextKit's glyphs".to_owned();
  vello.engine.rasterizer = "vello_cpu at TextKit's glyphs".to_owned();

  for variant in corpus.variants().filter(|variant| context.wants(variant)) {
    let laid = textkit::variant(&variant, &Setup::default(), context.repeats);
    let mut record = laid.record;
    let (width, height) = record.image_box(variant.width);
    let placed = crate::isolate::placed(&laid.laid, variant.case, record.truncated);
    let mut record_cg = record.clone();
    let mut record_vello = record.clone();

    for scale in &context.scales {
      let file = image_path(&variant, *scale);
      let image = textkit::image(
        &laid.laid,
        width,
        height,
        *scale,
        Ground::White,
        Smoothing(true),
      );

      write(&image, &dir.join(&file));
      record.images.insert(scale_key(*scale), file.clone());

      let image = crate::isolate::draw_cg(
        &placed,
        width,
        height,
        *scale,
        Ground::White,
        Smoothing(true),
      );

      write(&image, &cg_dir.join(&file));
      record_cg.images.insert(scale_key(*scale), file.clone());

      let image = crate::isolate::draw_vello(&placed, &mut bytes, width, height, *scale);

      write(&image, &vello_dir.join(&file));
      record_vello.images.insert(scale_key(*scale), file);
    }

    set.records.push(record);
    cg.records.push(record_cg);
    vello.records.push(record_vello);
  }

  set.timings.labels = Some(textkit_labels(corpus, context.labels, 13., 150.));
  (set, cg, vello)
}

/// The platform label's images (`NSTextField`, `UILabel`) over A's records.
pub fn labels(context: &Context, corpus: &Corpus, a: &RecordSet, dir: &Path) -> Option<RecordSet> {
  let mtm = objc2::MainThreadMarker::new()?;

  #[cfg(target_os = "macos")]
  let _ = objc2_app_kit::NSApplication::sharedApplication(mtm);
  let _ = std::fs::create_dir_all(dir.join("png"));
  let mut set = a.clone();

  set.engine.name = if cfg!(target_os = "ios") {
    "uilabel"
  }
  else {
    "nstextfield"
  }
  .to_owned();
  set.timings.labels = None;

  for record in &mut set.records {
    let Some(variant) = corpus.variant(&record.variant)
    else {
      continue;
    };
    let (width, height) = record.image_box(variant.width);

    record.images.clear();

    for scale in &context.scales {
      let file = image_path(&variant, *scale);

      write(
        &crate::label::image(mtm, &variant, width, height, *scale),
        &dir.join(&file),
      );
      record.images.insert(scale_key(*scale), file);
    }
  }

  Some(set)
}

/// The corpus's texts in turn (the empty one left out), as the harness's labels take them.
fn label_variants(corpus: &Corpus, count: usize, size: f32, width: f32) -> Vec<Variant<'_>> {
  let cases: Vec<&Case> = corpus
    .cases
    .iter()
    .filter(|case| !case.text.is_empty())
    .collect();

  (0..count)
    .map(|at| Variant {
      case: cases[at % cases.len()],
      size,
      width: Some(width),
    })
    .collect()
}

/// `count` labels by TextKit 2: their attributed strings built, laid out at `width`, laid out
/// again at `width - 10` (the container resized), and drawn into one image at scale 1, eight
/// columns wide.
pub fn textkit_labels(corpus: &Corpus, count: usize, size: f32, width: f32) -> LabelTimings {
  let variants = label_variants(corpus, count, size, width);
  let setup = Setup::default();
  let started = Instant::now();
  let strings: Vec<_> = variants
    .iter()
    .map(|variant| textkit::attributed(variant, &setup))
    .collect();
  let build_ms = started.elapsed().as_secs_f64() * 1e3;
  let started = Instant::now();
  let mut laid: Vec<textkit::TkLaid> = strings
    .iter()
    .map(|string| textkit::lay_out(string, f64::from(width), 0, false))
    .collect();
  let layout_ms = started.elapsed().as_secs_f64() * 1e3;
  let lines: usize = laid.iter().map(|laid| laid.lines.len()).sum();
  let started = Instant::now();

  for laid in &mut laid {
    let _ = laid.relayout(f64::from(width - 10.));
  }

  let relayout_ms = started.elapsed().as_secs_f64() * 1e3;

  for laid in &mut laid {
    let _ = laid.relayout(f64::from(width));
  }

  let column = f64::from(width + 10.);
  let mut tops = [0f64; 8];
  let mut placed = Vec::new();

  for (at, laid) in laid.iter().enumerate() {
    let slot = at % 8;

    placed.push((laid, slot as f64 * column, tops[slot]));
    tops[slot] += laid.usage.size.height + 4.;
  }

  let canvas = Canvas::new(
    (column * 8.).ceil() as usize,
    tops.iter().copied().fold(1., f64::max).ceil() as usize,
    1.,
    Ground::White,
    Smoothing(true),
  );
  let context = &*canvas.context;

  CGContext::translate_ctm(Some(context), 0., canvas.height as f64);
  CGContext::scale_ctm(Some(context), 1., -1.);

  let started = Instant::now();

  crate::platform::with_context(context, || {
    for (laid, x, y) in &placed {
      laid.draw_in(context, *x, *y);
    }
  });

  let draw_ms = started.elapsed().as_secs_f64() * 1e3;

  LabelTimings {
    count,
    size,
    width,
    build_ms,
    layout_ms,
    relayout_ms: Some(relayout_ms),
    draw_ms: Some(draw_ms),
    glyphs: None,
    lines: Some(lines),
  }
}

/// Core Text's own breaks: each variant through `CTFramesetter` in a frame of its width (no
/// images, no line limit).
pub fn framesetter(context: &Context, corpus: &Corpus) -> RecordSet {
  let mut set = context.set(
    "ctframesetter",
    &context.os_version,
    "none",
    &[("path", "a rectangle of the variant's width")],
  );

  set.scales = Vec::new();

  for variant in corpus.variants().filter(|variant| context.wants(variant)) {
    let attributed = textkit::attributed(&variant, &Setup::default());
    let cf: &CFAttributedString = unsafe { &*std::ptr::from_ref(&*attributed).cast() };
    let width = variant.width.map_or(textkit::UNBOUNDED, f64::from);
    let framesetter = unsafe { CTFramesetter::with_attributed_string(cf) };
    let path = unsafe {
      CGPath::with_rect(
        CGRect::new(CGPoint::new(0., 0.), CGSize::new(width, 1.0e6)),
        std::ptr::null(),
      )
    };
    let started = Instant::now();
    let frame = unsafe {
      framesetter.frame(
        CFRange {
          location: 0,
          length: 0,
        },
        &path,
        None,
      )
    };
    let cold_ms = started.elapsed().as_secs_f64() * 1e3;
    let lines = unsafe { frame.lines() };
    let lines: &objc2_core_foundation::CFArray<CTLine> = unsafe { lines.cast_unchecked() };
    let count = lines.len();
    let mut origins = vec![CGPoint::new(0., 0.); count];

    if count > 0 {
      unsafe {
        frame.line_origins(
          CFRange {
            location: 0,
            length: 0,
          },
          std::ptr::NonNull::new_unchecked(origins.as_mut_ptr()),
        )
      };
    }

    let text = &variant.case.text;
    let offsets = Offsets::new(text);
    let mut record_lines = Vec::new();
    let mut runs = Vec::new();
    let mut clusters = Vec::new();

    for (index, line) in lines.iter().enumerate() {
      let range = unsafe { line.string_range() };
      let (typographic, ascent, descent, leading) = coretext::line_metrics(&line);
      let trailing = unsafe { line.trailing_whitespace_width() };
      let baseline = 1.0e6 - origins[index].y;
      let start = range.location as usize;
      let end = (range.location + range.length) as usize;
      let parts = coretext::runs(&line);
      let (line_runs, line_clusters) =
        coretext::record_parts(&parts, 0, index, origins[index].x, &offsets);

      runs.extend(line_runs);
      clusters.extend(line_clusters);
      record_lines.push(Line {
        range: Range {
          utf8: Some(offsets.range_utf8([start, end])),
          utf16: Some([start, end]),
        },
        x: origins[index].x as f32,
        width: (typographic - trailing) as f32,
        top: (baseline - ascent) as f32,
        height: (ascent + descent + leading) as f32,
        baseline: baseline as f32,
        ascent: ascent as f32,
        descent: descent as f32,
        leading: leading as f32,
        hyphen: false,
        ellipsis: false,
      });
    }

    if record_lines.is_empty() {
      record_lines.push(Line {
        range: Range {
          utf8: Some([0, 0]),
          utf16: Some([0, 0]),
        },
        ..Line::default()
      });
    }

    let top = record_lines.first().map_or(0., |line| line.top);

    for line in &mut record_lines {
      line.top -= top;
      line.baseline -= top;
    }

    set.records.push(Record {
      variant: variant.id(),
      width: record_lines
        .iter()
        .map(|line| line.width)
        .fold(0., f32::max),
      height: record_lines
        .iter()
        .map(|line| line.top + line.height)
        .fold(0., f32::max),
      lines: record_lines,
      truncated: false,
      runs,
      clusters,
      intrinsic: None,
      timings: Some(VariantTimings {
        cold_ms,
        warm_ms: cold_ms,
      }),
      images: BTreeMap::new(),
      glyph_runs: Vec::new(),
      notes: if variant.case.max_lines.is_some() {
        vec!["no line limit: CTFramesetter fills the frame".to_owned()]
      }
      else {
        Vec::new()
      },
    });
  }

  set
}

type FileCache = std::sync::Mutex<HashMap<String, Option<Arc<Vec<u8>>>>>;

static FILES: std::sync::OnceLock<FileCache> = std::sync::OnceLock::new();

/// The bytes of the font file at `path`, read once per process (an emoji font is hundreds of
/// megabytes).
#[must_use]
pub fn read_file(path: &str) -> Option<Arc<Vec<u8>>> {
  let cache = FILES.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
  let mut cache = cache
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  cache
    .entry(path.to_owned())
    .or_insert_with(|| std::fs::read(path).ok().map(Arc::new))
    .clone()
}

/// Font files read once, by path.
#[derive(Default)]
pub struct Files;

impl Files {
  /// The bytes of the file at `path`.
  pub fn get(&mut self, path: &str) -> Option<Arc<Vec<u8>>> {
    read_file(path)
  }
}

/// C over a set of Parley records: each record's glyph runs drawn by Core Text from the font
/// files they name, the records otherwise Parley's.
pub fn c_from_records(
  b: &RecordSet,
  raster: &mut Raster,
  files: &mut Files,
  dir: &Path,
) -> RecordSet {
  let _ = std::fs::create_dir_all(dir.join("png"));

  let corpus = Corpus::builtin();
  let mut set = b.clone();

  set.engine.rasterizer = "coretext CTFontDrawGlyphs".to_owned();
  set.engine.settings.insert(
    "instance".to_owned(),
    "every axis from Parley's coordinates".to_owned(),
  );
  set.timings.labels = None;

  for record in &mut set.records {
    let Some(variant) = corpus.variant(&record.variant)
    else {
      continue;
    };
    let mut missing = Vec::new();
    let mut owned = Vec::new();

    for run in &record.glyph_runs {
      let Some(path) = run.font.path.clone()
      else {
        missing.push(run.font.postscript.clone());

        continue;
      };
      let Some(data) = files.get(&path)
      else {
        missing.push(format!("{path} unreadable"));

        continue;
      };

      owned.push((run, path, data));
    }

    let runs: Vec<DrawRun<'_>> = owned
      .iter()
      .map(|(run, path, data)| DrawRun {
        source: Source::File {
          path: path.clone(),
          postscript: run.font.postscript.clone(),
          index: run.font.index,
        },
        key: format!("{path}#{}", run.font.index),
        data: data.as_slice(),
        index: run.font.index,
        size: run.size,
        coords: run.coords.clone(),
        embolden: run.embolden,
        skew: run.skew,
        glyphs: run
          .glyphs
          .iter()
          .map(|glyph| (glyph.id, glyph.x, glyph.y))
          .collect(),
      })
      .collect();
    let (width, height) = record.image_box(variant.width);

    record.images.clear();

    for scale in &b.scales {
      let canvas = Canvas::for_box(width, height, *scale, Ground::White, Smoothing(true));

      raster.draw(&canvas, &runs);

      let file = image_path(&variant, *scale);

      write(&canvas.to_rgba(), &dir.join(&file));
      record.images.insert(scale_key(*scale), file);
    }

    if !missing.is_empty() {
      record
        .notes
        .push(format!("C could not open: {}", missing.join(", ")));
    }

    record.glyph_runs.clear();
  }

  set
}

/// The draw runs of a laid-out Parley layout whose fonts were registered from Core Text.
#[must_use]
pub fn tuned_runs<'a>(laid: &'a Laid, engine: &TunedEngine) -> Vec<DrawRun<'a>> {
  laid
    .glyph_runs()
    .into_iter()
    .filter_map(|run| {
      let id = run.font.data.id();
      let source = engine.sources.get(&id)?.clone();
      let data: &'a [u8] = unsafe {
        std::slice::from_raw_parts(run.font.data.data().as_ptr(), run.font.data.data().len())
      };

      Some(DrawRun {
        source: Source::Font(source),
        key: format!("blob{id}#{}", run.font.index),
        data,
        index: run.font.index,
        size: run.size,
        coords: run.coords.clone(),
        embolden: run.synthesis.embolden(),
        skew: run.synthesis.skew(),
        glyphs: run
          .glyphs
          .iter()
          .map(|glyph| (glyph.id, glyph.x, glyph.y))
          .collect(),
      })
    })
    .collect()
}

/// B and C tuned: every variant through [`TunedEngine`], drawn by vello (B) and by Core Text
/// (C).
pub fn tuned(
  context: &Context,
  corpus: &Corpus,
  engine_b: &mut TunedEngine,
  engine_c: &mut TunedEngine,
  raster: &mut Raster,
  dir_b: &Path,
  dir_c: &Path,
) -> (RecordSet, RecordSet) {
  let _ = std::fs::create_dir_all(dir_b.join("png"));
  let _ = std::fs::create_dir_all(dir_c.join("png"));

  let tuning_b = format!("{:?}", engine_b.tuning);
  let tuning_c = format!("{:?}", engine_c.tuning);
  let mut b = context.set("parley-tuned", "0.12.0", "vello_cpu 0.3.0", &[
    ("fonts", "Core Text: NSFont/UIFont system font, CTFontCreateForStringWithLanguage fallback, sfnt from CTFontCopyTable"),
    ("tuning", &tuning_b),
  ]);
  let mut c = context.set(
    "parley-tuned",
    "0.12.0",
    "coretext CTFontDrawGlyphs",
    &[
      (
        "fonts",
        "as B tuned, hvgl fonts shaped from their other tables",
      ),
      ("tuning", &tuning_c),
    ],
  );

  for variant in corpus.variants().filter(|variant| context.wants(variant)) {
    for (engine, set, dir, vello) in [
      (&mut *engine_b, &mut b, dir_b, true),
      (&mut *engine_c, &mut c, dir_c, false),
    ] {
      let started = Instant::now();
      let laid = engine.lay_out(&variant);
      let cold_ms = started.elapsed().as_secs_f64() * 1e3;
      let warm: Vec<f64> = (0..context.repeats)
        .map(|_| {
          let started = Instant::now();
          let again = engine.lay_out(&variant);

          std::hint::black_box(again.laid.layout.len());

          started.elapsed().as_secs_f64() * 1e3
        })
        .collect();
      let intrinsic = engine.intrinsic(&variant);
      let mut record = engine.record(&variant, &laid, Some(intrinsic));

      record.timings = Some(VariantTimings {
        cold_ms,
        warm_ms: median(warm),
      });
      record
        .notes
        .push(format!("families: {}", laid.families.join(", ")));

      let (width, height) = record.image_box(variant.width);
      let runs = if vello {
        Vec::new()
      }
      else {
        tuned_runs(&laid.laid, engine)
      };

      for scale in &context.scales {
        let file = image_path(&variant, *scale);

        if vello {
          let _ = render::write_png(
            render::render(&laid.laid, width, height, *scale, RenderOptions::default()),
            &dir.join(&file),
          );
        }
        else {
          let canvas = Canvas::for_box(width, height, *scale, Ground::White, Smoothing(true));

          raster.draw(&canvas, &runs);
          write(&canvas.to_rgba(), &dir.join(&file));
        }

        record.images.insert(scale_key(*scale), file);
      }

      record.glyph_runs.clear();
      set.records.push(record);
    }
  }

  b.timings.labels = Some(tuned_labels(corpus, engine_b, context.labels, 13., 150.));
  (b, c)
}

/// `count` labels through the tuned engine: shaped (fallback resolved), broken at `width`, at
/// `width - 10`, and drawn by vello into one image at scale 1.
pub fn tuned_labels(
  corpus: &Corpus,
  engine: &mut TunedEngine,
  count: usize,
  size: f32,
  width: f32,
) -> LabelTimings {
  let variants = label_variants(corpus, count, size, width);
  let started = Instant::now();
  let shaped: Vec<_> = variants
    .iter()
    .map(|variant| engine.shape(variant.case, size).0)
    .collect();
  let build_ms = started.elapsed().as_secs_f64() * 1e3;
  let started = Instant::now();
  let laid: Vec<Laid> = shaped
    .iter()
    .map(|shaped| shaped.break_at(Some(width)))
    .collect();
  let layout_ms = started.elapsed().as_secs_f64() * 1e3;
  let started = Instant::now();
  let again: Vec<Laid> = shaped
    .iter()
    .map(|shaped| shaped.break_at(Some(width - 10.)))
    .collect();
  let relayout_ms = started.elapsed().as_secs_f64() * 1e3;

  drop(again);

  let (draw_ms, glyphs) = vello_labels(&laid, width);

  LabelTimings {
    count,
    size,
    width,
    build_ms,
    layout_ms,
    relayout_ms: Some(relayout_ms),
    draw_ms: Some(draw_ms),
    glyphs: Some(glyphs),
    lines: Some(laid.iter().map(|laid| laid.layout.len()).sum()),
  }
}

fn placements(laid: &[Laid], width: f32) -> (Vec<(f32, f32)>, f32, f32) {
  let column = width + 10.;
  let mut tops = [0f32; 8];
  let mut at = Vec::new();

  for (index, laid) in laid.iter().enumerate() {
    let slot = index % 8;

    at.push((slot as f32 * column, tops[slot]));
    tops[slot] += laid.layout.height() + 4.;
  }

  (at, column * 8., tops.iter().copied().fold(1., f32::max))
}

/// Draws `laid` by vello into one image at scale 1: milliseconds and glyphs.
#[must_use]
pub fn vello_labels(laid: &[Laid], width: f32) -> (f64, usize) {
  let (at, wide, high) = placements(laid, width);
  let placed: Vec<(&Laid, (f32, f32))> = laid.iter().zip(at).collect();
  let started = Instant::now();
  let (_, glyphs) = render::render_many(
    &placed,
    wide.ceil().min(65535.) as u16,
    high.ceil().min(65535.) as u16,
  );

  (started.elapsed().as_secs_f64() * 1e3, glyphs)
}

/// The 1000 Parley labels of the harness (fontique's system fonts) laid out, then drawn by Core
/// Text into one image at `scale`: milliseconds, glyphs.
pub fn coretext_labels(
  corpus: &Corpus,
  count: usize,
  scale: f32,
  raster: &mut Raster,
  files: &mut Files,
) -> (f64, usize) {
  let mut engine = ParleyEngine::new(Fonts::system(), LayoutOptions::default());
  let variants = label_variants(corpus, count, 13., 150.);
  let laid: Vec<Laid> = variants
    .iter()
    .map(|variant| engine.lay_out(variant))
    .collect();
  let (at, wide, high) = placements(&laid, 150.);
  let mut owned = Vec::new();

  for (laid, (x, y)) in laid.iter().zip(at) {
    for run in laid.glyph_runs() {
      let identity = engine.fonts.identity(&run.font);
      let Some(path) = identity.path.clone()
      else {
        continue;
      };
      let Some(data) = files.get(&path)
      else {
        continue;
      };

      owned.push((run, identity, path, data, x, y));
    }
  }

  let runs: Vec<DrawRun<'_>> = owned
    .iter()
    .map(|(run, identity, path, data, x, y)| DrawRun {
      source: Source::File {
        path: path.clone(),
        postscript: identity.postscript.clone(),
        index: identity.index,
      },
      key: format!("{path}#{}", identity.index),
      data: data.as_slice(),
      index: identity.index,
      size: run.size,
      coords: run.coords.clone(),
      embolden: run.synthesis.embolden(),
      skew: run.synthesis.skew(),
      glyphs: run
        .glyphs
        .iter()
        .map(|glyph| {
          (
            glyph.id,
            glyph.x + x - text_parity::record::MARGIN,
            glyph.y + y - text_parity::record::MARGIN,
          )
        })
        .collect(),
    })
    .collect();
  let glyphs = runs.iter().map(|run| run.glyphs.len()).sum();
  let canvas = Canvas::new(
    (f64::from(wide) * f64::from(scale)).ceil() as usize,
    (f64::from(high) * f64::from(scale)).ceil() as usize,
    f64::from(scale),
    Ground::White,
    Smoothing(true),
  );

  for run in &runs {
    let _ = raster.font(run);
  }

  let started = Instant::now();

  raster.draw(&canvas, &runs);

  (started.elapsed().as_secs_f64() * 1e3, glyphs)
}
