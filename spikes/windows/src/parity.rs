//! The text-parity corpus on Windows, three ways:
//!
//! - **A**, DirectWrite's layout ([`crate::native`]), drawn three ways: Direct2D's own
//!   `DrawTextLayout` (`a-d2d`, what WinUI shows), the spike's DirectWrite-rasterized grayscale
//!   atlas composited by vello (`a-atlas`, A as decided) and vello's outlines (`a-floor`);
//! - **B**, Parley and vello through the harness: fontique's DirectWrite backend as it ships (`b`),
//!   and tuned (`bt`) with each case's fallback fonts from DirectWrite's per-run fallback
//!   ([`crate::resolver`]) ahead of fontique's;
//! - **C**, B's glyph runs drawn by DirectWrite: through the same atlas (`c`, `ct`) and by
//!   Direct2D's `DrawGlyphRun` (`c-d2d`, `ct-d2d`).
//!
//! Every set is written under `out/parity/<set>/` (`records.json`, `png/`), every comparison to
//! `out/parity/cmp/`, and the totals to the report.

use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
  rc::Rc,
  time::Instant,
};

use text_parity::{
  bench,
  compare::{self, Comparison, Options as CompareOptions},
  corpus::{Case, Corpus, Variant},
  fonts::{FontSource, Fonts, GenericFamilies},
  layout::{Options as LayoutOptions, ParleyEngine},
  record::{
    self,
    EngineInfo,
    LabelTimings,
    PlatformInfo,
    Record,
    RecordSet,
    SetTimings,
    image_size,
    scale_key,
  },
  render::{self, RenderOptions},
  run::{self, ParleyRun},
};
use windows::Win32::Graphics::DirectWrite::*;
use windows_core::Result as WinResult;

use crate::{
  Report,
  engine::Collected,
  faces::Faces,
  image::Img,
  native::{Native, SYSTEM_FAMILY},
  paint::{AtlasPainter, Floor, Item, Reference, new_context, to_img},
  resolver::Resolver,
};

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// A result whose error is a message.
pub type Res<T> = Result<T, String>;

/// Turns a Windows error into a message.
pub trait Msg<T> {
  fn msg(self, what: &str) -> Res<T>;
}

impl<T> Msg<T> for WinResult<T> {
  fn msg(self, what: &str) -> Res<T> {
    self.map_err(|e| format!("{what}: {e}"))
  }
}

fn ms(started: Instant) -> f64 {
  started.elapsed().as_secs_f64() * 1e3
}

/// The platform every set records.
pub fn platform() -> PlatformInfo {
  PlatformInfo {
    os: "windows".to_owned(),
    os_version: std::env::var("SPIKE_OS_VERSION").unwrap_or_default(),
    device: std::env::var("SPIKE_DEVICE").ok(),
  }
}

/// The painters every image of A and C goes through.
pub struct Painters {
  pub reference: Reference,
  pub atlas: AtlasPainter,
  pub floor: Floor,
}

/// How a set of glyph runs is put on pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paint {
  Atlas,
  Floor,
  Direct2D,
}

impl Painters {
  pub fn new(factory: &IDWriteFactory4) -> Res<Self> {
    let mut atlas = AtlasPainter::new(factory).msg("atlas")?;
    // Raw coverage: the closest to Direct2D's grayscale in the first round.
    atlas.contrast = 0.0;
    let mut floor = Floor::new();
    // As the harness draws B: outlines unhinted.
    floor.hint = false;

    Ok(Self {
      reference: Reference::new().msg("reference")?,
      atlas,
      floor,
    })
  }

  /// `runs` (in DIPs where `paint` is Direct2D, in pixels otherwise) drawn into a `w` by `h` image
  /// at `scale`, the layout's top left at the margin.
  pub fn glyphs(&mut self, paint: Paint, runs: Collected, scale: f32, w: u32, h: u32) -> Res<Img> {
    let margin = record::MARGIN;

    match paint {
      Paint::Direct2D => {
        let items = [Item::Glyphs {
          runs: Rc::new(runs),
          x: margin,
          y: margin,
          color: BLACK,
        }];
        self
          .reference
          .render_at(&items, w, h, scale)
          .msg("d2d glyphs")
      }
      Paint::Atlas | Paint::Floor => {
        let items = [Item::Glyphs {
          runs: Rc::new(runs),
          x: margin * scale,
          y: margin * scale,
          color: BLACK,
        }];
        let mut ctx = new_context(w as u16, h as u16, 0);

        if paint == Paint::Atlas {
          self.atlas.reset();
          Ok(to_img(&self.atlas.render(&mut ctx, &items).msg("atlas")?))
        }
        else {
          Ok(to_img(&self.floor.render(&mut ctx, &items).msg("floor")?))
        }
      }
    }
  }
}

/// A set written to disk, with where its images are relative to.
pub struct Written {
  pub name: String,
  pub set: RecordSet,
  pub root: PathBuf,
}

fn engine_info(name: &str, rasterizer: &str, settings: &[(&str, &str)]) -> EngineInfo {
  EngineInfo {
    name: name.to_owned(),
    version: std::env::var("SPIKE_OS_VERSION").unwrap_or_default(),
    rasterizer: rasterizer.to_owned(),
    settings: settings
      .iter()
      .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
      .chain([(
        "build".to_owned(),
        if cfg!(debug_assertions) {
          "debug"
        }
        else {
          "release"
        }
        .to_owned(),
      )])
      .collect(),
  }
}

fn write_set(dir: &Path, name: &str, set: RecordSet) -> Res<Written> {
  set.write(&dir.join("records.json"))?;

  Ok(Written {
    name: name.to_owned(),
    set,
    root: dir.to_path_buf(),
  })
}

fn png_name(variant: &Variant<'_>, scale: f32) -> String {
  format!("png/{}@{}x.png", variant.file_stem(), scale_key(scale))
}

/// The suite's settings.
pub struct Suite {
  pub out: PathBuf,
  pub scales: Vec<f32>,
  pub corpus: Corpus,
  pub factory: IDWriteFactory4,
}

impl Suite {
  fn dir(&self, set: &str) -> Res<PathBuf> {
    let dir = self.out.join(set);
    std::fs::create_dir_all(dir.join("png")).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
  }

  /// A: every variant through DirectWrite, its records once and its images three ways.
  pub fn native(&self, painters: &mut Painters, r: &mut Report) -> Res<Vec<Written>> {
    let native = Native::new(&self.factory);
    let started = Instant::now();
    let isolated: IDWriteFactory =
      unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED) }.msg("isolated factory")?;
    let mut collection = None;
    unsafe { isolated.GetSystemFontCollection(&mut collection, false) }.msg("collection")?;
    let font_collection_ms = ms(started);
    let families = collection.map_or(0, |c| unsafe { c.GetFontFamilyCount() });
    let hello = text_parity::bench::plain_case("hello", "Hello");
    let started = Instant::now();
    let _ = native
      .lay_out(&Variant {
        case: &hello,
        size: 17.0,
        width: None,
      })
      .msg("hello")?;
    let first_layout_ms = ms(started);

    r.line(format!(
      "a.fonts: isolated factory's system collection {font_collection_ms:.2} ms, {families} families (WWS); first layout {first_layout_ms:.2} ms"
    ));

    let kinds = [
      (
        "a-d2d",
        Paint::Direct2D,
        "direct2d DrawTextLayout (grayscale, WIC software)",
      ),
      (
        "a-atlas",
        Paint::Atlas,
        "directwrite CreateGlyphRunAnalysis atlas (grayscale, raw coverage) composited by vello_cpu",
      ),
      ("a-floor", Paint::Floor, "vello_cpu outlines (unhinted)"),
    ];
    let dirs: Vec<PathBuf> = kinds
      .iter()
      .map(|(name, ..)| self.dir(name))
      .collect::<Res<_>>()?;
    let mut records: Vec<Vec<Record>> = vec![Vec::new(); kinds.len()];
    let mut problems = 0;

    for variant in self.corpus.variants() {
      let result = (|| -> Res<()> {
        let (laid, timings) = native.time(&variant, 5).msg("lay out")?;
        let mut record = native.record(&variant, &laid).msg("record")?;
        record.intrinsic = native.intrinsic(&variant).ok();
        record.timings = Some(timings);

        let (bw, bh) = record.image_box(variant.width);

        for (at, (_, paint, _)) in kinds.iter().enumerate() {
          let mut record = record.clone();

          for &scale in &self.scales {
            let (w, h) = image_size(bw, bh, scale);
            let img = match paint {
              Paint::Direct2D => painters
                .reference
                .render_at(
                  &[Item::Layout {
                    layout: laid.layout.clone(),
                    x: record::MARGIN,
                    y: record::MARGIN,
                    color: BLACK,
                  }],
                  w,
                  h,
                  scale,
                )
                .msg("d2d")?,
              _ => {
                let runs = native.drawn(&laid, scale).msg("runs")?;
                painters.glyphs(*paint, runs, scale, w, h)?
              }
            };
            let file = png_name(&variant, scale);
            img.save(&dirs[at].join(&file));
            record.images.insert(scale_key(scale), file);
          }

          records[at].push(record);
        }

        Ok(())
      })();

      if let Err(e) = result {
        problems += 1;
        r.line(format!("a.error {}: {e}", variant.id()));
      }
    }

    r.line(format!(
      "a.records: {} of {} variants ({problems} failed); floor glyph errors {} unresolved faces {} simulated runs {}; atlas cells {} errors {} colour cells {} colour path {:?}",
      records[0].len(),
      self.corpus.variants().count(),
      painters.floor.glyph_errors,
      painters.floor.fonts.unresolved,
      painters.floor.fonts.simulated_runs,
      painters.atlas.rasterized,
      painters.atlas.raster_errors,
      painters.atlas.colour_cells,
      painters.atlas.colour_path,
    ));

    let labels = self.native_labels(&native, painters, r)?;
    let mut written = Vec::new();

    for ((at, (name, _, rasterizer)), records) in kinds.iter().enumerate().zip(records) {
      let set = RecordSet {
        schema: record::SCHEMA,
        corpus_version: self.corpus.version,
        engine: engine_info(
          "directwrite",
          rasterizer,
          &[
            ("family", SYSTEM_FAMILY),
            ("wrapping", "EMERGENCY_BREAK"),
            ("measuring", "NATURAL"),
            ("trimming", "SetMaxHeight + ellipsis trimming sign"),
          ],
        ),
        platform: platform(),
        scales: self.scales.clone(),
        timings: SetTimings {
          font_collection_ms: Some(font_collection_ms),
          first_layout_ms: Some(first_layout_ms),
          labels: (at == 0).then(|| labels.clone()).flatten(),
        },
        records,
      };
      written.push(write_set(&dirs[at], name, set)?);
    }

    Ok(written)
  }

  /// The harness's 1000 labels through DirectWrite: each case's text in turn at 13 wrapped at
  /// 150, laid out again at 140 (a fresh layout, the decided immutable form), drawn through the
  /// atlas in eight columns.
  fn native_labels(
    &self,
    native: &Native,
    painters: &mut Painters,
    r: &mut Report,
  ) -> Res<Option<LabelTimings>> {
    let cases: Vec<&Case> = self
      .corpus
      .cases
      .iter()
      .filter(|c| !c.text.is_empty())
      .collect();
    let count = 1000;
    let texts: Vec<(Vec<u16>, &Case)> = (0..count)
      .map(|at| {
        let case = cases[at % cases.len()];
        (case.text.encode_utf16().collect(), case)
      })
      .collect();
    let formats: Vec<IDWriteTextFormat> = texts
      .iter()
      .map(|(_, case)| native.format(case, 13.0, true))
      .collect::<WinResult<_>>()
      .msg("formats")?;
    let make = |width: f32| -> Res<Vec<IDWriteTextLayout>> {
      texts
        .iter()
        .zip(&formats)
        .map(|((utf16, _), format)| unsafe {
          IDWriteFactory::CreateTextLayout(&native.factory, utf16, format, width, 1.0e6)
        })
        .collect::<WinResult<_>>()
        .msg("layouts")
    };
    let started = Instant::now();
    let layouts = make(150.0)?;
    let build_ms = ms(started);
    let started = Instant::now();
    let mut lines = 0usize;

    for layout in &layouts {
      lines += crate::engine::line_metrics(layout).msg("metrics")?.len();
    }

    let layout_ms = ms(started);
    let started = Instant::now();

    for layout in make(140.0)? {
      let _ = crate::engine::line_metrics(&layout).msg("metrics")?;
    }

    let relayout_ms = ms(started);
    let started = Instant::now();

    for layout in &layouts {
      unsafe { layout.SetMaxWidth(140.0) }.msg("max width")?;
      let _ = crate::engine::line_metrics(layout).msg("metrics")?;
    }

    let rewrap_ms = ms(started);

    for layout in &layouts {
      unsafe { layout.SetMaxWidth(150.0) }.msg("max width")?;
    }

    // Eight columns, as the harness draws Parley's.
    let column = 160.0f32;
    let mut tops = [0f32; 8];
    let mut items = Vec::new();
    let mut d2d_items = Vec::new();
    let mut glyphs = 0;

    for (at, layout) in layouts.iter().enumerate() {
      let slot = at % 8;
      let mut m = DWRITE_TEXT_METRICS::default();
      unsafe { layout.GetMetrics(&mut m) }.msg("metrics")?;
      let runs = crate::engine::collect(layout, 0.0, 0.0, 1.0, true).msg("collect")?;
      glyphs += runs.runs.iter().map(|r| r.glyphs.len()).sum::<usize>();
      items.push(Item::Glyphs {
        runs: Rc::new(runs),
        x: slot as f32 * column,
        y: tops[slot].round(),
        color: BLACK,
      });
      d2d_items.push(Item::Layout {
        layout: layout.clone(),
        x: slot as f32 * column,
        y: tops[slot].round(),
        color: BLACK,
      });
      tops[slot] += m.height + 4.0;
    }

    let (w, h) = (
      (column * 8.0).ceil() as u32,
      tops.iter().copied().fold(1.0, f32::max).ceil() as u32,
    );
    let mut ctx = new_context(w as u16, h as u16, 0);
    painters.atlas.reset();
    let started = Instant::now();
    let _ = painters
      .atlas
      .render(&mut ctx, &items)
      .msg("labels atlas")?;
    let atlas_cold = ms(started);
    let started = Instant::now();
    let img = painters
      .atlas
      .render(&mut ctx, &items)
      .msg("labels atlas")?;
    let draw_ms = ms(started);
    to_img(&img).save(&self.out.join("labels-a-atlas.png"));
    let started = Instant::now();
    let _ = painters
      .reference
      .render(&d2d_items, w, h)
      .msg("labels d2d")?;
    let d2d_ms = ms(started);

    r.line(format!(
      "a.labels: 1000 corpus labels at 13 / 150: create {build_ms:.2} ms, first metrics (shape + break) {layout_ms:.2} ms, fresh layouts at 140 {relayout_ms:.2} ms, SetMaxWidth(140) on the same layouts {rewrap_ms:.2} ms; {lines} lines {glyphs} glyphs; draw atlas cold {atlas_cold:.2} ms warm {draw_ms:.2} ms, Direct2D WIC {d2d_ms:.2} ms ({w}x{h})"
    ));

    Ok(Some(LabelTimings {
      count,
      size: 13.0,
      width: 150.0,
      build_ms,
      layout_ms,
      relayout_ms: Some(relayout_ms),
      draw_ms: Some(draw_ms),
      glyphs: Some(glyphs),
      lines: Some(lines),
    }))
  }

  /// B as it ships: the harness's own run, fontique's DirectWrite backend.
  pub fn parley(&self, r: &mut Report) -> Res<Written> {
    let dir = self.out.join("b");
    let started = Instant::now();
    let set = run::parley(&ParleyRun {
      out: dir.clone(),
      scales: self.scales.clone(),
      glyphs: true,
      os_version: Some(platform().os_version),
      device: platform().device,
      ..ParleyRun::default()
    })?;

    r.line(format!(
      "b.run: {} records in {:.0} ms; fontique start-up {:.2} ms, first layout {:.2} ms; labels {:?}",
      set.records.len(),
      ms(started),
      set.timings.font_collection_ms.unwrap_or_default(),
      set.timings.first_layout_ms.unwrap_or_default(),
      set.timings.labels
    ));

    Ok(Written {
      name: "b".to_owned(),
      set,
      root: dir,
    })
  }

  /// B tuned: per case, the families DirectWrite's per-run fallback maps the text to (the
  /// requested family as base, the case's weight, style and locale) set as `system-ui` after Segoe
  /// UI, ahead of fontique's per-script fallback.
  pub fn tuned(&self, resolver: &Resolver, r: &mut Report) -> Res<Written> {
    let dir = self.dir("bt")?;
    let mut stock = Fonts::system();
    let mut records = Vec::new();
    let mut setup_ms = Vec::new();
    let mut lists = BTreeMap::new();

    for case in &self.corpus.cases {
      let mapped = resolver
        .map(
          &case.text,
          &case.locale,
          Some(SYSTEM_FAMILY),
          i32::from(case.weight),
          case.italic,
        )
        .msg("map")?;
      let mut families = vec![SYSTEM_FAMILY.to_owned()];

      for m in &mapped {
        if let Some(family) = &m.family
          && !families.contains(family)
        {
          if stock.context.collection.family_id(family).is_some() {
            families.push(family.clone());
          }
          else {
            r.line(format!(
              "bt.missing {}: {family} is not in fontique's collection",
              case.id
            ));
          }
        }
      }

      lists.insert(case.id.clone(), families.join(" > "));

      let source = FontSource {
        system: true,
        files: Vec::new(),
        generics: vec![GenericFamilies {
          generic: "system-ui".to_owned(),
          families,
        }],
        fallbacks: Vec::new(),
      };
      let started = Instant::now();
      let fonts = Fonts::new(&source)?;
      setup_ms.push(ms(started));
      let mut engine = ParleyEngine::new(fonts, LayoutOptions::default());

      for variant in self.corpus.variants().filter(|v| v.case.id == case.id) {
        let (laid, timings) = bench::time_variant(&mut engine, &variant, 5);
        let intrinsic = engine.shape(&variant).intrinsic();
        let mut record = laid.record(&variant, &mut engine.fonts, Some(intrinsic), true);
        record.timings = Some(timings);
        let (bw, bh) = record.image_box(variant.width);

        for &scale in &self.scales {
          let file = png_name(&variant, scale);
          render::write_png(
            render::render(&laid, bw, bh, scale, RenderOptions::default()),
            &dir.join(&file),
          )?;
          record.images.insert(scale_key(scale), file);
        }

        records.push(record);
      }
    }

    for (case, list) in &lists {
      r.line(format!("bt.families {case}: {list}"));
    }

    setup_ms.sort_by(f64::total_cmp);
    r.line(format!(
      "bt.collections: {} per-case collections, median {:.2} ms, max {:.2} ms",
      setup_ms.len(),
      setup_ms[setup_ms.len() / 2],
      setup_ms.last().copied().unwrap_or_default()
    ));

    let set = RecordSet {
      schema: record::SCHEMA,
      corpus_version: self.corpus.version,
      engine: engine_info(
        "parley-tuned",
        "vello_cpu 0.3.0",
        &[(
          "fonts",
          "system + DirectWrite MapCharacters families per case as system-ui",
        )],
      ),
      platform: platform(),
      scales: self.scales.clone(),
      timings: SetTimings::default(),
      records,
    };

    write_set(&dir, "bt", set)
  }

  /// C: a Parley set's glyph runs drawn by DirectWrite, through the atlas and through Direct2D.
  pub fn platform_drawn(
    &self,
    from: &Written,
    names: (&str, &str),
    painters: &mut Painters,
    faces: &mut Faces,
    r: &mut Report,
  ) -> Res<(Written, Written)> {
    let mut out = Vec::new();

    for (name, paint, rasterizer) in [
      (
        names.0,
        Paint::Atlas,
        "directwrite CreateGlyphRunAnalysis atlas over Parley's glyph runs",
      ),
      (
        names.1,
        Paint::Direct2D,
        "direct2d DrawGlyphRun(WithColorSupport) over Parley's glyph runs",
      ),
    ] {
      let dir = self.dir(name)?;
      let mut records = Vec::new();
      let started = Instant::now();

      for record in &from.set.records {
        let Some(variant) = self.corpus.variant(&record.variant)
        else {
          continue;
        };
        let mut restamped = record.clone();
        restamped.images.clear();
        let dips = faces.runs(&record.glyph_runs);
        let (bw, bh) = record.image_box(variant.width);

        for &scale in &self.scales {
          let (w, h) = image_size(bw, bh, scale);
          let runs = if paint == Paint::Direct2D {
            dips.scaled(1.0)
          }
          else {
            dips.scaled(scale)
          };
          let img = painters.glyphs(paint, runs, scale, w, h)?;
          let file = png_name(&variant, scale);
          img.save(&dir.join(&file));
          restamped.images.insert(scale_key(scale), file);
        }

        restamped.glyph_runs.clear();
        records.push(restamped);
      }

      r.line(format!(
        "{name}.drawn: {} records in {:.0} ms; faces failed {:?}; glyphs without a face {}",
        records.len(),
        ms(started),
        faces.failures.iter().take(5).collect::<Vec<_>>(),
        faces.skipped_glyphs
      ));

      let mut set = from.set.clone();
      set.engine.rasterizer = rasterizer.to_owned();
      set.records = records;
      out.push(write_set(&dir, name, set)?);
    }

    let d2d = out.pop().unwrap();
    let atlas = out.pop().unwrap();
    Ok((atlas, d2d))
  }

  /// Compares `a` against `b` at every scale, writing markdown and JSON, and reports the totals.
  pub fn compare(&self, a: &Written, b: &Written, per_case: bool, r: &mut Report) -> Res<()> {
    let dir = self.out.join("cmp");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = format!("{}_vs_{}", a.name, b.name);

    for &scale in &self.scales {
      let diff_dir = (per_case && scale == *self.scales.last().unwrap())
        .then(|| dir.join(format!("diff-{name}")));
      let comparison = compare::compare(
        &self.corpus,
        (&a.set, &a.root),
        (&b.set, &b.root),
        &CompareOptions {
          scale: Some(scale),
          diff_dir,
          ..CompareOptions::default()
        },
      );
      let stem = format!("{name}@{}x", scale_key(scale));
      std::fs::write(
        dir.join(format!("{stem}.md")),
        comparison.markdown(&self.corpus),
      )
      .map_err(|e| e.to_string())?;
      std::fs::write(
        dir.join(format!("{stem}.json")),
        serde_json::to_string_pretty(&comparison).map_err(|e| e.to_string())?,
      )
      .map_err(|e| e.to_string())?;
      report_totals(&stem, &comparison, r);

      if per_case && scale == *self.scales.last().unwrap() {
        for (tag, t) in &comparison.by_tag {
          r.line(format!("cmp.{stem}.tag.{tag}: {}", totals_line(t)));
        }

        for v in &comparison.variants {
          let px = comparison.scale.as_ref().and_then(|s| v.pixels.get(s));
          r.line(format!(
            "cmp.{stem}.v {}: breaks={} lines={:?} maxw={:.2} dh={:.2} dbase={:?} dlh={:.2} dad={:.2} fonts={}/{} clusters={} intrinsic={:?} ssim={} iou={} mad={}",
            v.variant,
            v.breaks_identical,
            v.lines,
            v.max_width_delta,
            v.height_delta,
            v.first_baseline_delta.map(|d| (d * 100.0).round() / 100.0),
            v.max_line_height_delta,
            v.max_ascent_descent_delta,
            v.fonts.same_family,
            v.fonts.characters,
            v.clusters
              .as_ref()
              .map_or("-".to_owned(), |c| format!("{}/{} mean {:.3} max {:.3}", c.matched, c.of_a, c.mean_abs, c.max_abs)),
            v.intrinsic_delta.map(|(a, b)| ((a * 100.0).round() / 100.0, (b * 100.0).round() / 100.0)),
            px.map_or("-".to_owned(), |p| format!("{:.3}", p.ssim)),
            px.map_or("-".to_owned(), |p| format!("{:.3}", p.ink_iou)),
            px.map_or("-".to_owned(), |p| format!("{:.2}", p.mean_abs_diff)),
          ));

          if !v.breaks_identical
            && let Some((la, lb)) = &v.break_texts
          {
            r.line(format!(
              "cmp.{stem}.breaks {}: a={la:?} b={lb:?}",
              v.variant
            ));
          }

          if !v.fonts.differing.is_empty() {
            r.line(format!(
              "cmp.{stem}.fonts {}: {:?}",
              v.variant,
              v.fonts.differing.iter().take(6).collect::<Vec<_>>()
            ));
          }
        }
      }
    }

    Ok(())
  }
}

fn totals_line(t: &compare::Totals) -> String {
  format!(
    "variants={} breaks={} lines_equal={} widths_within={} heights_within={} mean_abs_h={:.3} same_family={}/{} imaged={} mad={:.3} over={:.4} ssim={:.4} iou={:.4} cold_ms=({:.2},{:.2}) warm_ms=({:.2},{:.2})",
    t.variants,
    t.breaks_identical,
    t.lines_equal,
    t.widths_within,
    t.heights_within,
    t.mean_abs_height_delta,
    t.same_family,
    t.characters,
    t.imaged,
    t.mean_abs_diff,
    t.mean_share_over,
    t.mean_ssim,
    t.mean_ink_iou,
    t.cold_ms.0,
    t.cold_ms.1,
    t.warm_ms.0,
    t.warm_ms.1,
  )
}

fn report_totals(stem: &str, c: &Comparison, r: &mut Report) {
  r.line(format!("cmp.{stem}: {}", totals_line(&c.totals)));

  if !c.missing.is_empty() {
    r.line(format!("cmp.{stem}.missing: {:?}", c.missing));
  }
}

/// Colour pixels in each emoji variant's image, per set: whether colour glyphs drew.
pub fn emoji_colour(corpus: &Corpus, sets: &[&Written], scale: f32, r: &mut Report) {
  for variant in corpus
    .variants()
    .filter(|v| v.case.tags.iter().any(|t| t == "emoji"))
  {
    let mut counts = Vec::new();

    for set in sets {
      let count = set
        .set
        .record(&variant.id())
        .and_then(|rec| rec.images.get(&scale_key(scale)))
        .and_then(|file| text_parity::image::Rgba::read(&set.root.join(file)).ok())
        .map_or(-1, |img| {
          img
            .data
            .chunks_exact(4)
            .filter(|p| {
              let (mx, mn) = (p[0].max(p[1]).max(p[2]), p[0].min(p[1]).min(p[2]));
              mx - mn > 40
            })
            .count() as i64
        });
      counts.push(format!("{}={count}", set.name));
    }

    r.line(format!(
      "emoji.colour_pixels@{}x {}: {}",
      scale_key(scale),
      variant.id(),
      counts.join(" ")
    ));
  }
}

/// The whole parity suite.
pub fn run(out: &Path, factory: &IDWriteFactory4, r: &mut Report) -> Res<()> {
  let suite = Suite {
    out: out.join("parity"),
    scales: vec![1.0, 2.0],
    corpus: Corpus::builtin(),
    factory: factory.clone(),
  };
  let mut painters = Painters::new(factory)?;
  let mut faces = Faces::new(factory).msg("faces")?;
  let resolver = Resolver::new(factory).msg("resolver")?;
  let started = Instant::now();
  let a = suite.native(&mut painters, r)?;
  r.line(format!("time.a: {:.0} ms", ms(started)));
  let started = Instant::now();
  let b = suite.parley(r)?;
  r.line(format!("time.b: {:.0} ms", ms(started)));
  let started = Instant::now();
  let bt = suite.tuned(&resolver, r)?;
  r.line(format!("time.bt: {:.0} ms", ms(started)));
  let started = Instant::now();
  let (c, c_d2d) = suite.platform_drawn(&b, ("c", "c-d2d"), &mut painters, &mut faces, r)?;
  let (ct, ct_d2d) = suite.platform_drawn(&bt, ("ct", "ct-d2d"), &mut painters, &mut faces, r)?;
  r.line(format!("time.c: {:.0} ms", ms(started)));
  let (a_d2d, a_atlas, a_floor) = (&a[0], &a[1], &a[2]);
  let started = Instant::now();

  for (x, y, per_case) in [
    (a_atlas, &b, true),
    (a_atlas, &bt, true),
    (a_atlas, &ct, true),
    (&b, &c, true),
    (a_d2d, a_atlas, false),
    (a_d2d, a_floor, false),
    (a_atlas, a_floor, false),
    (a_atlas, &c, false),
    (&bt, &ct, false),
    (a_d2d, &ct_d2d, true),
    (a_d2d, &c_d2d, false),
    (&ct, &ct_d2d, false),
    (a_floor, &bt, false),
  ] {
    suite.compare(x, y, per_case, r)?;
  }

  r.line(format!("time.compare: {:.0} ms", ms(started)));

  for scale in [1.0, 2.0] {
    emoji_colour(
      &suite.corpus,
      &[a_d2d, a_atlas, a_floor, &b, &bt, &c, &ct, &c_d2d, &ct_d2d],
      scale,
      r,
    );
  }

  Ok(())
}
