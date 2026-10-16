//! A stand-in native engine: Pango fills the record schema on a Linux host, so the harness can
//! be exercised end to end. It proves the tool, not a result: Pango is not a platform Skene
//! targets. It is also the shape of a platform spike written in Rust (the macOS and Windows
//! ones): read the corpus, lay each variant out with the platform's engine, draw it by the image
//! convention, and write the records with this crate's types.
//!
//! usage: `text-parity-pango OUT_DIR [--scales 1,2,3] [--labels 1000]`

use std::{collections::BTreeMap, fs::File, path::Path, time::Instant};

use pango::{glib::translate::IntoGlib as _, prelude::*};
use text_parity::{
  bench::{median, since},
  corpus::{Alignment, Corpus, Direction, Variant, number},
  record::{
    self,
    Cluster,
    EngineInfo,
    LabelTimings,
    Line,
    MARGIN,
    PlatformInfo,
    Range,
    Record,
    RecordSet,
    Run,
    SetTimings,
    VariantTimings,
  },
  text::{Offsets, is_invisible},
};

const SCALE: f32 = pango::SCALE as f32;

fn units(value: i32) -> f32 {
  value as f32 / SCALE
}

fn weight(value: u16) -> pango::Weight {
  match value {
    ..=150 => pango::Weight::Thin,
    151..=250 => pango::Weight::Ultralight,
    251..=340 => pango::Weight::Light,
    341..=370 => pango::Weight::Semilight,
    371..=390 => pango::Weight::Book,
    391..=450 => pango::Weight::Normal,
    451..=550 => pango::Weight::Medium,
    551..=650 => pango::Weight::Semibold,
    651..=750 => pango::Weight::Bold,
    751..=850 => pango::Weight::Ultrabold,
    _ => pango::Weight::Heavy,
  }
}

struct Engine {
  context: pango::Context,
}

impl Engine {
  fn new() -> Self {
    let fontmap = pangocairo::FontMap::new();
    let context = fontmap.create_context();
    let mut options = cairo::FontOptions::new().expect("font options");

    options.set_antialias(cairo::Antialias::Gray);
    options.set_hint_style(cairo::HintStyle::None);
    options.set_hint_metrics(cairo::HintMetrics::Off);
    context.set_round_glyph_positions(false);
    pangocairo::functions::context_set_font_options(&context, Some(&options));
    pangocairo::functions::context_set_resolution(&context, 72.);

    Self { context }
  }

  fn layout(&self, variant: &Variant<'_>) -> pango::Layout {
    let case = variant.case;

    self
      .context
      .set_base_dir(if case.direction == Direction::Rtl {
        pango::Direction::Rtl
      }
      else {
        pango::Direction::Ltr
      });

    let layout = pango::Layout::new(&self.context);
    let family = match case.family.as_str() {
      "system" => "system-ui",
      other => other,
    };
    let mut description = pango::FontDescription::from_string(family);
    let attributes = pango::AttrList::new();

    description.set_absolute_size(f64::from(variant.size) * f64::from(pango::SCALE));
    description.set_weight(weight(case.weight));
    description.set_style(if case.italic {
      pango::Style::Italic
    }
    else {
      pango::Style::Normal
    });
    attributes.insert(pango::AttrLanguage::new(&pango::Language::from_string(
      &case.locale,
    )));
    layout.set_auto_dir(case.direction == Direction::Auto);
    layout.set_font_description(Some(&description));
    layout.set_attributes(Some(&attributes));
    layout.set_text(&case.text);

    if let Some(width) = variant.width {
      layout.set_width((width * SCALE).round() as i32);
      layout.set_wrap(pango::WrapMode::WordChar);
    }

    layout.set_alignment(match case.alignment {
      Alignment::Start | Alignment::Justify => pango::Alignment::Left,
      Alignment::Center => pango::Alignment::Center,
      Alignment::End => pango::Alignment::Right,
    });
    layout.set_justify(case.alignment == Alignment::Justify);

    if let (Some(lines), true) = (case.max_lines, case.ellipsis) {
      layout.set_ellipsize(pango::EllipsizeMode::End);
      layout.set_height(-(lines as i32));
    }

    layout
  }
}

/// Each line with its logical rectangle and baseline, in Pango units.
fn lines_of(layout: &pango::Layout) -> Vec<(pango::LayoutLine, pango::Rectangle, i32)> {
  let mut lines = Vec::new();
  let mut iterator = layout.iter();

  loop {
    let (_, logical) = iterator.line_extents();
    let baseline = iterator.baseline();

    if let Some(line) = iterator.line_readonly() {
      lines.push((line, logical, baseline));
    }

    if !iterator.next_line() {
      break;
    }
  }

  lines
}

fn record(engine: &Engine, variant: &Variant<'_>, scales: &[f32], out: &Path) -> Record {
  let text = &variant.case.text;
  let offsets = Offsets::new(text);
  let started = Instant::now();
  let layout = engine.layout(variant);
  let _ = layout.extents();
  let cold_ms = since(started);
  let warm = (0..5)
    .map(|_| {
      let started = Instant::now();
      let _ = engine.layout(variant).extents();

      since(started)
    })
    .collect();
  let all = lines_of(&layout);
  let mut truncated = layout.is_ellipsized();
  let mut kept = all.clone();

  if let (Some(max), false) = (variant.case.max_lines, variant.case.ellipsis)
    && all.len() > max as usize
  {
    kept.truncate(max as usize);
    truncated = true;
  }

  let mut lines = Vec::new();
  let mut runs = Vec::new();

  for (index, (line, logical, baseline)) in kept.iter().enumerate() {
    let start = line.start_index() as usize;
    let end = (start + line.length() as usize).min(text.len());
    let ellipsized = truncated && variant.case.ellipsis && index + 1 == kept.len();
    let mut edges = Vec::new();

    for (at, character) in text[start..end].char_indices() {
      let byte = start + at;
      let last = byte + character.len_utf8() >= end;

      if is_invisible(character) && !(character == '\u{AD}' && last) {
        continue;
      }

      let rect = layout.index_to_pos(byte as i32);

      edges.extend([rect.x(), rect.x() + rect.width()]);
    }

    // The width runs from the line's start edge to its last visible character's far edge.
    let rtl = line.resolved_direction() == pango::Direction::Rtl;
    let (start_edge, end_edge) = if rtl {
      (logical.x() + logical.width(), edges.iter().copied().min())
    }
    else {
      (logical.x(), edges.iter().copied().max())
    };
    let (left, right) = match (ellipsized, end_edge) {
      (true, _) => (logical.x(), logical.x() + logical.width()),
      (false, None) => (start_edge, start_edge),
      (false, Some(end)) if rtl => (end, start_edge),
      (false, Some(end)) => (start_edge, end),
    };

    lines.push(Line {
      range: Range::of([start, end], &offsets),
      x: units(left),
      width: units(right - left),
      top: units(logical.y()),
      height: units(logical.height()),
      baseline: units(*baseline),
      ascent: units(baseline - logical.y()),
      descent: units(logical.y() + logical.height() - baseline),
      leading: 0.,
      hyphen: text[start..end].ends_with('\u{AD}'),
      ellipsis: ellipsized,
    });

    for run in line.runs() {
      let item = run.item();
      let analysis = item.analysis();
      let description = analysis.font().describe();
      let begin = item.offset() as usize;

      runs.push(Run {
        range: Range::of(
          [begin, (begin + item.length() as usize).min(text.len())],
          &offsets,
        ),
        line: index,
        family: description.family().map(|family| family.to_string()),
        size: variant.size,
        weight: Some(description.weight().into_glib() as f32),
        italic: Some(description.style() != pango::Style::Normal),
        rtl: analysis.level() % 2 == 1,
        ..Run::default()
      });
    }
  }

  let mut clusters = Vec::new();

  for (start, cluster) in
    unicode_segmentation::UnicodeSegmentation::grapheme_indices(text.as_str(), true)
  {
    let Some(line) = kept.iter().position(|(line, _, _)| {
      let begin = line.start_index() as usize;

      begin <= start && start < begin + (line.length() as usize).max(1)
    })
    else {
      continue;
    };
    let rects: Vec<pango::Rectangle> = cluster
      .char_indices()
      .map(|(at, _)| layout.index_to_pos((start + at) as i32))
      .collect();

    clusters.push(Cluster {
      range: Range::of([start, start + cluster.len()], &offsets),
      line,
      x: units(
        rects
          .iter()
          .map(|r| r.x().min(r.x() + r.width()))
          .min()
          .unwrap_or(0),
      ),
      advance: units(rects.iter().map(|r| r.width().abs()).sum()),
    });
  }

  let width = lines.iter().map(|line| line.width).fold(0., f32::max);
  let height = kept
    .last()
    .map_or(0., |(_, logical, _)| units(logical.y() + logical.height()));
  let box_width = variant.width.unwrap_or(width);
  let mut images = BTreeMap::new();

  for scale in scales {
    let (wide, high) = record::image_size(box_width, height, *scale);
    let surface =
      cairo::ImageSurface::create(cairo::Format::Rgb24, wide as i32, high as i32).expect("surface");
    let cr = cairo::Context::new(&surface).expect("context");

    cr.set_source_rgb(1., 1., 1.);
    let _ = cr.paint();
    cr.scale(f64::from(*scale), f64::from(*scale));
    cr.translate(f64::from(MARGIN), f64::from(MARGIN));
    cr.set_source_rgb(0., 0., 0.);

    for (line, logical, baseline) in &kept {
      cr.move_to(f64::from(units(logical.x())), f64::from(units(*baseline)));
      pangocairo::functions::show_layout_line(&cr, line);
    }

    drop(cr);

    let name = format!("png/{}@{}x.png", variant.file_stem(), number(*scale));
    let mut file = File::create(out.join(&name)).expect("png file");

    surface.write_to_png(&mut file).expect("png");
    images.insert(number(*scale), name);
  }

  Record {
    variant: variant.id(),
    lines,
    width,
    height,
    truncated,
    runs,
    clusters,
    intrinsic: None,
    timings: Some(VariantTimings {
      cold_ms,
      warm_ms: median(warm),
    }),
    images,
    glyph_runs: Vec::new(),
    notes: Vec::new(),
  }
}

fn labels(engine: &Engine, corpus: &Corpus, count: usize) -> LabelTimings {
  let cases: Vec<_> = corpus
    .cases
    .iter()
    .filter(|case| !case.text.is_empty())
    .collect();
  let variants: Vec<Variant<'_>> = (0..count)
    .map(|at| Variant {
      case: cases[at % cases.len()],
      size: 13.,
      width: Some(150.),
    })
    .collect();
  let started = Instant::now();
  let layouts: Vec<pango::Layout> = variants
    .iter()
    .map(|variant| engine.layout(variant))
    .collect();
  let build_ms = since(started);
  let started = Instant::now();

  layouts.iter().for_each(|layout| {
    let _ = layout.extents();
  });

  let layout_ms = since(started);
  let started = Instant::now();

  layouts.iter().for_each(|layout| {
    layout.set_width(140 * pango::SCALE);
    let _ = layout.extents();
  });

  LabelTimings {
    count,
    size: 13.,
    width: 150.,
    build_ms,
    layout_ms,
    relayout_ms: Some(since(started)),
    draw_ms: None,
    glyphs: None,
    lines: Some(
      layouts
        .iter()
        .map(|layout| layout.line_count() as usize)
        .sum(),
    ),
  }
}

fn main() {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let out = Path::new(
    arguments
      .first()
      .expect("usage: text-parity-pango OUT_DIR [--scales 1,2,3] [--labels 1000]"),
  );
  let option = |name: &str| {
    arguments
      .iter()
      .position(|argument| argument == name)
      .and_then(|at| arguments.get(at + 1))
  };
  let scales: Vec<f32> = option("--scales")
    .map_or("1,2,3", String::as_str)
    .split(',')
    .map(|scale| scale.parse().expect("a scale"))
    .collect();
  let count: usize = option("--labels").map_or(1000, |count| count.parse().expect("a count"));
  let corpus = Corpus::builtin();

  std::fs::create_dir_all(out.join("png")).expect("output directory");

  let started = Instant::now();
  let engine = Engine::new();
  let startup = since(started);
  let started = Instant::now();
  let hello = text_parity::bench::plain_case("hello", "Hello");
  let _ = engine
    .layout(&Variant {
      case: &hello,
      size: 17.,
      width: None,
    })
    .extents();
  let first = since(started);
  let records: Vec<Record> = corpus
    .variants()
    .map(|variant| record(&engine, &variant, &scales, out))
    .collect();
  let set = RecordSet {
    schema: record::SCHEMA,
    corpus_version: corpus.version,
    engine: EngineInfo {
      name: "pango".to_owned(),
      version: pango::version_string().to_string(),
      rasterizer: format!("cairo {}", cairo::version_string()),
      settings: BTreeMap::from([
        ("antialias".to_owned(), "gray".to_owned()),
        ("hint_style".to_owned(), "none".to_owned()),
        ("hint_metrics".to_owned(), "off".to_owned()),
        ("round_glyph_positions".to_owned(), "false".to_owned()),
      ]),
    },
    platform: PlatformInfo {
      os: "linux".to_owned(),
      os_version: std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_default()
        .trim()
        .to_owned(),
      device: None,
    },
    scales,
    timings: SetTimings {
      font_collection_ms: Some(startup),
      first_layout_ms: Some(first),
      labels: (count > 0).then(|| labels(&engine, &corpus, count)),
    },
    records,
  };
  let path = out.join("records.json");

  set.write(&path).expect("records");
  eprintln!("{} -> {}", set.summary(), path.display());
}
