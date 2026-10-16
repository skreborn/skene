//! One run of everything on this platform, written to one directory:
//!
//! ```text
//! a/ a-cg/ a-vello/      TextKit 2, and its glyphs drawn by Core Text and by vello
//! label/                 NSTextField or UILabel over A's records
//! ct-frame/              CTFramesetter's breaks
//! b/ c/                  stock Parley (harness), and its glyph runs drawn by Core Text
//! bt/ ct/                tuned Parley drawn by vello, and by Core Text
//! compare/               every pairing, markdown and JSON; diff/ images for A-B, A-BT, A-CT
//! emoji-sizes/ dynamic-type/ bold-text/ genmoji/ smoothing/
//! results.txt probes.json summary.json
//! ```

use std::{fmt::Write as _, path::PathBuf, time::Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use text_parity::{
  compare::{self, Options as CompareOptions, Totals},
  corpus::Corpus,
  fonts::{FontSource, Fonts},
  layout::{Options as LayoutOptions, ParleyEngine},
  record::RecordSet,
  run::ParleyRun,
};

use crate::{
  probes::{self, Report},
  raster::Raster,
  sets::{self, Context, Files},
};

/// What a run does.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
  /// The run's directory.
  pub out: PathBuf,
  /// Image scales.
  pub scales: Vec<f32>,
  /// Warm layouts per variant.
  pub repeats: usize,
  /// Labels in the label timings.
  pub labels: usize,
  /// Only variants whose id contains this.
  pub only: Option<String>,
  /// The device, for the records.
  pub device: Option<String>,
  /// The parts to run (`corpus`, `probes`, `subsets`, `smoothing`, `genmoji`); all where empty.
  pub parts: Vec<String>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      out: PathBuf::from("parity"),
      scales: if cfg!(target_os = "ios") {
        vec![2., 3.]
      }
      else {
        vec![1., 2.]
      },
      repeats: 5,
      labels: 1000,
      only: None,
      device: None,
      parts: Vec::new(),
    }
  }
}

impl Config {
  fn wants(&self, part: &str) -> bool {
    self.parts.is_empty() || self.parts.iter().any(|wanted| wanted == part)
  }
}

/// The OS version as the platform words it.
#[must_use]
pub fn os_version() -> String {
  objc2_foundation::NSProcessInfo::processInfo()
    .operatingSystemVersionString()
    .to_string()
}

fn totals_line(name: &str, totals: &Totals) -> String {
  format!(
    "{name:<14} n={:<3} breaks {:>3} lines {:>3} widths {:>3} heights {:>3} |dh| {:.2} family {}/{} | MAD {:.2} >thr {:.2}% SSIM {:.3} IoU {:.3} | cold {:.1}/{:.1} warm {:.1}/{:.1} ms",
    totals.variants,
    totals.breaks_identical,
    totals.lines_equal,
    totals.widths_within,
    totals.heights_within,
    totals.mean_abs_height_delta,
    totals.same_family,
    totals.characters,
    totals.mean_abs_diff,
    totals.mean_share_over * 100.,
    totals.mean_ssim,
    totals.mean_ink_iou,
    totals.cold_ms.0,
    totals.cold_ms.1,
    totals.warm_ms.0,
    totals.warm_ms.1
  )
}

/// Runs `body`, reporting a panic in it rather than ending the run.
pub fn guard(report: &mut Report, name: &str, body: impl FnOnce(&mut Report)) {
  let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&mut *report)));

  if let Err(panic) = result {
    let message = panic
      .downcast_ref::<String>()
      .cloned()
      .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
      .unwrap_or_default();

    report.line(format!("  {name} PANICKED: {message}"));
  }
}

/// Runs everything `config` asks for, answering the results summary.
///
/// # Errors
///
/// Where the stock Parley run fails.
pub fn run(config: &Config) -> Result<String, String> {
  let started = Instant::now();
  let out = &config.out;
  let _ = std::fs::create_dir_all(out);

  // Images the probes write go here too.
  unsafe { std::env::set_var("PARITY_OUT", out) };

  let mut report = Report::default();
  let os_version = os_version();
  let device = config.device.clone().or_else(|| {
    std::env::var("SIMULATOR_MODEL_IDENTIFIER")
      .ok()
      .map(|model| format!("{model} simulator"))
  });

  report.line(format!(
    "apple-parity on {} {os_version} ({})",
    crate::platform::OS,
    device.clone().unwrap_or_else(|| "this machine".to_owned())
  ));
  report.line(format!(
    "scales {:?}, repeats {}, labels {}, only {:?}",
    config.scales, config.repeats, config.labels, config.only
  ));

  let context = Context {
    out: out.clone(),
    scales: config.scales.clone(),
    repeats: config.repeats,
    only: config.only.clone(),
    os_version: os_version.clone(),
    device: device.clone(),
    labels: config.labels,
  };
  let corpus = Corpus::builtin();
  let mut raster = Raster::default();
  let mut files = Files::default();
  let mut tuned = probes::tuned_engine();
  let mut tuned_c = probes::tuned_engine();
  let mut summary = serde_json::Map::new();
  let mut b_set = None;

  if config.wants("probes") {
    guard(&mut report, "fontique", |report| {
      drop(probes::fontique(report))
    });
    guard(&mut report, "system fonts", probes::system_fonts);
  }

  if config.wants("corpus") {
    report.line(
      "\n[corpus: A TextKit 2, B stock Parley, C stock Parley + CTFontDrawGlyphs, BT/CT tuned]",
    );

    let step = Instant::now();
    let b = text_parity::run::parley(&ParleyRun {
      out: out.join("b"),
      scales: config.scales.clone(),
      fonts: FontSource::system(),
      repeats: config.repeats,
      labels: config.labels,
      glyphs: true,
      only: config.only.clone(),
      os_version: Some(os_version.clone()),
      device: device.clone(),
      ..ParleyRun::default()
    })?;

    report.line(format!(
      "  B (harness) {:.1} s",
      step.elapsed().as_secs_f64()
    ));

    let step = Instant::now();
    let (a, a_cg, a_vello) = sets::textkit(&context, &corpus, &out.join("a"));

    report.line(format!("  A {:.1} s", step.elapsed().as_secs_f64()));

    let step = Instant::now();
    let frame = sets::framesetter(&context, &corpus);
    let c = sets::c_from_records(&b, &mut raster, &mut files, &out.join("c"));

    report.line(format!(
      "  CTFramesetter and C {:.1} s",
      step.elapsed().as_secs_f64()
    ));

    let step = Instant::now();
    let (bt, ct) = sets::tuned(
      &context,
      &corpus,
      &mut tuned,
      &mut tuned_c,
      &mut raster,
      &out.join("bt"),
      &out.join("ct"),
    );

    report.line(format!("  BT and CT {:.1} s", step.elapsed().as_secs_f64()));

    let step = Instant::now();
    let label = sets::labels(&context, &corpus, &a, &out.join("label"));

    report.line(format!("  label {:.1} s", step.elapsed().as_secs_f64()));

    let mut named: Vec<(&str, RecordSet)> = vec![
      ("a", a),
      ("a-cg", a_cg),
      ("a-vello", a_vello),
      ("ct-frame", frame),
      ("b", b.clone()),
      ("c", c),
      ("bt", bt),
      ("ct", ct),
    ];

    if let Some(label) = label {
      named.push(("label", label));
    }

    for (name, set) in &named {
      let _ = std::fs::create_dir_all(out.join(name));

      if *name != "b" {
        let mut stripped = set.clone();

        if *name == "bt" || *name == "ct" {
          for record in &mut stripped.records {
            record.glyph_runs.clear();
          }
        }

        let _ = stripped.write(&out.join(name).join("records.json"));
      }
    }

    let find = |name: &str| {
      named
        .iter()
        .find(|(set, _)| *set == name)
        .map(|(_, set)| set)
    };
    let pairs = [
      ("a", "b"),
      ("a", "c"),
      ("b", "c"),
      ("a", "bt"),
      ("a", "ct"),
      ("bt", "ct"),
      ("a", "a-cg"),
      ("a", "a-vello"),
      ("a", "ct-frame"),
      ("label", "a"),
      ("label", "b"),
      ("label", "c"),
      ("label", "bt"),
      ("label", "ct"),
    ];
    let largest = config.scales.iter().copied().fold(1f32, f32::max);
    let _ = std::fs::create_dir_all(out.join("compare"));

    report.line("\n[comparisons: totals (A is the reference where first); widths/heights within 0.5 pt; pixels at the largest scale]");

    let mut compared = serde_json::Map::new();

    for (x, y) in pairs {
      let (Some(set_x), Some(set_y)) = (find(x), find(y))
      else {
        continue;
      };
      let diff = matches!(
        (x, y),
        ("a", "b") | ("a", "bt") | ("a", "ct") | ("a", "a-vello")
      )
      .then(|| out.join("diff").join(format!("{x}-{y}")));
      let options = CompareOptions {
        scale: Some(largest),
        diff_dir: diff,
        ..CompareOptions::default()
      };
      let comparison = compare::compare(
        &corpus,
        (set_x, &out.join(x)),
        (set_y, &out.join(y)),
        &options,
      );
      let name = format!("{x}-{y}");

      report.line(format!("  {}", totals_line(&name, &comparison.totals)));

      let _ = std::fs::write(
        out.join("compare").join(format!("{name}.md")),
        comparison.markdown(&corpus),
      );
      let _ = std::fs::write(
        out.join("compare").join(format!("{name}.json")),
        serde_json::to_string(&comparison).unwrap_or_default(),
      );

      if matches!(
        name.as_str(),
        "a-b" | "a-bt" | "a-ct" | "a-c" | "a-a-vello" | "label-ct"
      ) {
        for (tag, totals) in &comparison.by_tag {
          report.line(format!("      {}", totals_line(tag, totals)));
        }

        let differing: Vec<String> = comparison
          .variants
          .iter()
          .filter(|variant| !variant.breaks_identical)
          .map(|variant| variant.variant.clone())
          .collect();

        report.line(format!("      breaks differ in: {}", differing.join(", ")));
      }

      compared.insert(
        name,
        json!({"totals": comparison.totals, "by_tag": comparison.by_tag}),
      );
    }

    summary.insert("comparisons".to_owned(), Value::Object(compared));
    report.line("\n[label timings: 1000 labels at 13pt, width 150]");

    for (name, set) in &named {
      if let Some(labels) = &set.timings.labels {
        report.line(format!(
          "  {name:<6} build {:.1} layout {:.1} relayout {:.1} draw {:.1} ms (glyphs {:?}, lines {:?})",
          labels.build_ms,
          labels.layout_ms,
          labels.relayout_ms.unwrap_or_default(),
          labels.draw_ms.unwrap_or_default(),
          labels.glyphs,
          labels.lines
        ));
      }
    }

    if let Some(collection) = b.timings.font_collection_ms {
      report.line(format!(
        "  B font collection {collection:.1} ms, first layout {:.1} ms",
        b.timings.first_layout_ms.unwrap_or_default()
      ));
    }

    for scale in [1f32, 2.] {
      let (ms, glyphs) =
        sets::coretext_labels(&corpus, config.labels, scale, &mut raster, &mut files);

      report.line(format!("  C draw of the 1000 Parley labels by CTFontDrawGlyphs at {scale}x: {ms:.1} ms ({glyphs} glyphs)"));
    }

    for (name, engine) in [("B tuned", &tuned), ("C tuned", &tuned_c)] {
      report.line(format!(
        "  {name} engine: {} fonts registered from CTFontCopyTable, {:.1} MB, {:.1} ms; refused: {:?}; notes: {:?}",
        engine.log.len(),
        engine.registered_bytes as f64 / 1e6,
        engine.register_ms,
        engine.rejected,
        engine.notes
      ));

      for (family, postscript, path) in &engine.log {
        let _ = writeln!(report.text, "    {family}: {postscript} {path:?}");
      }
    }

    if !raster.instance_mismatches.is_empty() || !raster.problems.is_empty() {
      report.line(format!(
        "  C raster problems {:?}; instance mismatches {:?}",
        raster.problems, raster.instance_mismatches
      ));
    }

    b_set = Some(b);
  }

  if config.wants("probes") {
    guard(&mut report, "pingfang", |report| {
      probes::pingfang(report, b_set.as_ref())
    });
    guard(&mut report, "shaping", probes::shaping);
    guard(&mut report, "metrics", |report| {
      probes::metrics(report, &mut tuned)
    });
    guard(&mut report, "korean", probes::korean);
    guard(&mut report, "hyphenation", probes::hyphenation);
    guard(&mut report, "emoji", |report| {
      probes::emoji(report, &mut tuned)
    });
    guard(&mut report, "timings", |report| {
      probes::timings(report, &mut tuned)
    });
    guard(&mut report, "cjk spacing", crate::cjk::spacing);
    guard(&mut report, "run instances", crate::cjk::instances);
    guard(&mut report, "script shaping", crate::cjk::scripts);
  }

  if config.wants("genmoji") {
    guard(&mut report, "genmoji", |report| {
      crate::genmoji::probe(report, &mut tuned_c, &mut raster)
    });
  }

  if config.wants("smoothing") {
    guard(&mut report, "smoothing", |report| {
      crate::smoothing::study(report, &config.scales)
    });
  }

  if config.wants("subsets") {
    report.line("\n[emoji at 15, 17 and 40 pt: A against B, BT and CT]");

    let ids = [
      ("emoji-zwj", None),
      ("emoji-skin-tones", None),
      ("emoji-flags", None),
      ("emoji-keycaps", None),
      ("emoji-unicode-15", None),
      ("emoji-unicode-16", None),
    ];
    let emoji_corpus = Corpus {
      version: 1,
      cases: crate::subset::cases(&ids, "sized", &[15., 17., 40.]),
    };
    let dir = out.join("emoji-sizes");
    let mut stock = ParleyEngine::new(Fonts::system(), LayoutOptions::default());
    guard(&mut report, "emoji sizes", |report| {
      let sets = crate::subset::run(
        &context,
        &emoji_corpus,
        &dir,
        &|_| crate::textkit::Setup::default(),
        &|case| case.weight,
        None,
        (&mut tuned, &mut tuned_c),
        &mut raster,
        &mut stock,
      );
      let emoji = crate::subset::summarize(report, "emoji 15/17/40", &emoji_corpus, &dir, &sets);

      report.put("emoji_sizes", emoji);
    });

    #[cfg(target_os = "ios")]
    {
      guard(&mut report, "dynamic type", |report| {
        crate::ios::dynamic_type(report, &context, (&mut tuned, &mut tuned_c), &mut raster)
      });
      guard(&mut report, "bold text", |report| {
        crate::ios::bold_text(report, &context, (&mut tuned, &mut tuned_c), &mut raster)
      });
    }
  }

  #[cfg(target_os = "ios")]
  if config.wants("probes") {
    guard(&mut report, "language", crate::ios::language);
  }

  report.line(format!(
    "\nrun took {:.1} s",
    started.elapsed().as_secs_f64()
  ));
  summary.insert("probes".to_owned(), Value::Object(report.json.clone()));

  let _ = std::fs::write(out.join("results.txt"), &report.text);
  let _ = std::fs::write(
    out.join("summary.json"),
    serde_json::to_string_pretty(&Value::Object(summary)).unwrap_or_default(),
  );

  Ok(report.text)
}

/// [`run`] configured by JSON.
///
/// # Errors
///
/// Where the JSON is not a configuration, or the run fails.
pub fn run_json(config: &str) -> Result<String, String> {
  let config: Config = serde_json::from_str(config).map_err(|error| error.to_string())?;

  run(&config)
}
