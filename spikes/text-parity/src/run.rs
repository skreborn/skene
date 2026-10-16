//! One Parley run over the corpus: the records, the images and the timings, written to a
//! directory. The CLI's `parley` command is this; a platform spike embedding the crate (the
//! Android app's Rust library, an iOS test's static library) calls [`parley`](fn@parley) or
//! [`parley_json`] the same way.

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
  bench,
  corpus::Corpus,
  fonts::{FontSource, Fonts},
  layout::{Options as LayoutOptions, ParleyEngine},
  record::{self, EngineInfo, PlatformInfo, RecordSet, SetTimings},
  render::{self, RenderOptions},
};

/// What a run does.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ParleyRun {
  /// The directory `records.json` and `png/` are written to.
  pub out: PathBuf,
  /// The scales images are drawn at.
  pub scales: Vec<f32>,
  /// The fonts: the system's, or a source naming them.
  pub fonts: FontSource,
  /// Vertical hinting in the images.
  pub hint: bool,
  /// Line metrics rounded to whole logical pixels.
  pub quantize: bool,
  /// OpenType features on the whole text, as CSS's `font-feature-settings` (`"chws" 1`).
  pub features: String,
  /// Parley's own emoji fallback, without Skene's emoji-first rule.
  pub plain_emoji: bool,
  /// Warm layouts timed per variant.
  pub repeats: usize,
  /// Labels in the many-labels timing; none where 0.
  pub labels: usize,
  /// Whether images are drawn.
  pub images: bool,
  /// Whether records carry Parley's glyph runs.
  pub glyphs: bool,
  /// Only the variants whose id contains this.
  pub only: Option<String>,
  /// The OS version to record, read from the system where `None`.
  pub os_version: Option<String>,
  /// The device to record.
  pub device: Option<String>,
}

impl Default for ParleyRun {
  fn default() -> Self {
    Self {
      out: PathBuf::from("parley"),
      scales: vec![1., 2., 3.],
      fonts: FontSource::system(),
      hint: false,
      quantize: false,
      features: String::new(),
      plain_emoji: false,
      repeats: 5,
      labels: 1000,
      images: true,
      glyphs: false,
      only: None,
      os_version: None,
      device: None,
    }
  }
}

fn os_version() -> String {
  std::fs::read_to_string("/proc/sys/kernel/osrelease")
    .map(|release| release.trim().to_owned())
    .unwrap_or_default()
}

/// Lays out, draws and times every variant as `run` says, and writes `records.json` and the
/// images under `run.out`.
///
/// # Errors
///
/// Where the font source fails, or a file cannot be written.
pub fn parley(run: &ParleyRun) -> Result<RecordSet, String> {
  let corpus = Corpus::builtin();
  let fonts = Fonts::new(&run.fonts)?;
  let startup = fonts.startup_ms;
  let render_options = RenderOptions { hint: run.hint };
  let layout_options = LayoutOptions {
    quantize: run.quantize,
    plain_emoji: run.plain_emoji,
    features: run.features.clone(),
  };
  let mut engine = ParleyEngine::new(fonts, layout_options);
  let first = bench::first_layout(&mut engine);
  let mut records = Vec::new();

  std::fs::create_dir_all(run.out.join("png"))
    .map_err(|error| format!("{}: {error}", run.out.display()))?;

  for variant in corpus.variants() {
    if run
      .only
      .as_ref()
      .is_some_and(|only| !variant.id().contains(only.as_str()))
    {
      continue;
    }

    let (laid, timings) = bench::time_variant(&mut engine, &variant, run.repeats);
    let intrinsic = engine.shape(&variant).intrinsic();
    let mut record = laid.record(&variant, &mut engine.fonts, Some(intrinsic), run.glyphs);

    record.timings = Some(timings);

    if run.images {
      let (width, height) = record.image_box(variant.width);

      for scale in &run.scales {
        let file = format!(
          "png/{}@{}x.png",
          variant.file_stem(),
          record::scale_key(*scale)
        );

        render::write_png(
          render::render(&laid, width, height, *scale, render_options),
          &run.out.join(&file),
        )?;
        record.images.insert(record::scale_key(*scale), file);
      }
    }

    records.push(record);
  }

  let labels = (run.labels > 0).then(|| bench::labels(&mut engine, &corpus, run.labels, 13., 150.));
  let set = RecordSet {
    schema: record::SCHEMA,
    corpus_version: corpus.version,
    engine: EngineInfo {
      name: "parley".to_owned(),
      version: "0.12.0".to_owned(),
      rasterizer: "vello_cpu 0.3.0".to_owned(),
      settings: BTreeMap::from([
        (
          "fonts".to_owned(),
          if run.fonts == FontSource::system() {
            "system".to_owned()
          }
          else {
            format!(
              "{} files, system {}",
              run.fonts.files.len(),
              run.fonts.system
            )
          },
        ),
        ("hint".to_owned(), run.hint.to_string()),
        ("quantize".to_owned(), run.quantize.to_string()),
        ("features".to_owned(), run.features.clone()),
        ("emoji_first".to_owned(), (!run.plain_emoji).to_string()),
        (
          "build".to_owned(),
          if cfg!(debug_assertions) {
            "debug"
          }
          else {
            "release"
          }
          .to_owned(),
        ),
      ]),
    },
    platform: PlatformInfo {
      os: std::env::consts::OS.to_owned(),
      os_version: run.os_version.clone().unwrap_or_else(os_version),
      device: run.device.clone(),
    },
    scales: if run.images {
      run.scales.clone()
    }
    else {
      Vec::new()
    },
    timings: SetTimings {
      font_collection_ms: Some(startup),
      first_layout_ms: Some(first),
      labels,
    },
    records,
  };

  set.write(&run.out.join("records.json"))?;

  Ok(set)
}

/// [`parley`](fn@parley) configured by a [`ParleyRun`] as JSON (every field optional), answering
/// the set's summary: one call for a wrapper over JNI or a C ABI.
///
/// # Errors
///
/// Where the JSON is not a run, or the run fails.
pub fn parley_json(config: &str) -> Result<String, String> {
  let run: ParleyRun = serde_json::from_str(config).map_err(|error| error.to_string())?;

  parley(&run).map(|set| set.summary())
}
