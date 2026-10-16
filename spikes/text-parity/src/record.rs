//! The record an engine writes: one [`RecordSet`] per run, one [`Record`] per variant.
//!
//! The schema is the contract between this crate and every platform spike, documented field by
//! field in the README. Coordinates are logical pixels, x to the right and y down from the
//! layout's top left; a text range is `[start, end)` in UTF-8 bytes (`utf8`) or UTF-16 code
//! units (`utf16`) of the case's text. A native record may give either unit;
//! [`RecordSet::normalize`] derives the other.

use std::{
  collections::BTreeMap,
  fmt::Write as _,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{corpus::Corpus, fonts::FontNames, text::Offsets};

/// The schema's version.
pub const SCHEMA: u32 = 1;

/// The margin around the layout box in every image, in logical pixels.
pub const MARGIN: f32 = 8.;

/// Which engine made the records, and how it drew them.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct EngineInfo {
  /// The layout engine: `parley`, `textkit2`, `staticlayout`, `directwrite`, `pango`.
  pub name: String,
  /// Its version, or the OS build that carries it.
  pub version: String,
  /// What drew the images: `vello_cpu`, `coregraphics`, `skia`, `direct2d`, `cairo`.
  pub rasterizer: String,
  /// Anything else that matters to reading the numbers (hinting, smoothing, font options).
  #[serde(default)]
  pub settings: BTreeMap<String, String>,
}

/// Where the records were made.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct PlatformInfo {
  /// `linux`, `ios`, `macos`, `android`, `windows`.
  pub os: String,
  /// The OS version and build.
  pub os_version: String,
  /// The device or simulator, where it matters.
  #[serde(default)]
  pub device: Option<String>,
}

/// Timings of the whole run, in milliseconds.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct SetTimings {
  /// Making the font collection ready to shape: enumerating the system's fonts.
  #[serde(default)]
  pub font_collection_ms: Option<f64>,
  /// The first layout after it, `Hello` at 17: the first font load.
  #[serde(default)]
  pub first_layout_ms: Option<f64>,
  /// The 1000 labels.
  #[serde(default)]
  pub labels: Option<LabelTimings>,
}

/// Many short labels laid out and drawn together, as a screen of a list does.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct LabelTimings {
  /// How many labels.
  pub count: usize,
  /// Their font size.
  pub size: f32,
  /// The width each wraps at.
  pub width: f32,
  /// Building each label's paragraph (shaping included where the engine shapes at build).
  pub build_ms: f64,
  /// Laying every label out at `width`.
  pub layout_ms: f64,
  /// Laying every label out again at `width - 10`.
  #[serde(default)]
  pub relayout_ms: Option<f64>,
  /// Drawing every label into one image at scale 1.
  #[serde(default)]
  pub draw_ms: Option<f64>,
  /// The glyphs drawn.
  #[serde(default)]
  pub glyphs: Option<usize>,
  /// The lines laid out.
  #[serde(default)]
  pub lines: Option<usize>,
}

/// Every variant an engine laid out, with what it ran on.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct RecordSet {
  /// [`SCHEMA`].
  pub schema: u32,
  /// The corpus's `version`.
  pub corpus_version: u32,
  /// The engine.
  pub engine: EngineInfo,
  /// The platform.
  pub platform: PlatformInfo,
  /// The scales images were drawn at.
  #[serde(default)]
  pub scales: Vec<f32>,
  /// Run-wide timings.
  #[serde(default)]
  pub timings: SetTimings,
  /// One per variant.
  pub records: Vec<Record>,
}

/// A text range in either unit; at least one is given.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Range {
  /// `[start, end)` in UTF-8 bytes.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub utf8: Option<[usize; 2]>,
  /// `[start, end)` in UTF-16 code units.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub utf16: Option<[usize; 2]>,
}

impl Range {
  /// A UTF-8 range with its UTF-16 twin.
  #[must_use]
  pub fn of(utf8: [usize; 2], offsets: &Offsets) -> Self {
    Self {
      utf8: Some(utf8),
      utf16: Some(offsets.range_utf16(utf8)),
    }
  }

  /// The UTF-8 range, derived where only UTF-16 is given.
  #[must_use]
  pub fn bytes(&self, offsets: &Offsets) -> [usize; 2] {
    self
      .utf8
      .or_else(|| self.utf16.map(|utf16| offsets.range_utf8(utf16)))
      .unwrap_or_default()
  }

  fn fill(&mut self, offsets: &Offsets) {
    match (self.utf8, self.utf16) {
      (Some(utf8), None) => self.utf16 = Some(offsets.range_utf16(utf8)),
      (None, Some(utf16)) => self.utf8 = Some(offsets.range_utf8(utf16)),
      _ => {}
    }
  }
}

/// One line as laid out.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Line {
  /// The text the line holds, its trailing blanks and line break included.
  #[serde(flatten)]
  pub range: Range,
  /// Where the line's content starts, after alignment.
  pub x: f32,
  /// The content's advance without trailing blanks, with a drawn hyphen or ellipsis.
  pub width: f32,
  /// The top of the line box.
  pub top: f32,
  /// The line box's height.
  pub height: f32,
  /// The baseline, from the layout's top.
  pub baseline: f32,
  /// The line's ascent above the baseline (positive).
  pub ascent: f32,
  /// The line's descent below the baseline (positive).
  pub descent: f32,
  /// The leading the line box adds beyond ascent and descent.
  #[serde(default)]
  pub leading: f32,
  /// Whether the line ends in a drawn hyphen (at a soft hyphen or an automatic break).
  #[serde(default)]
  pub hyphen: bool,
  /// Whether the line ends in a drawn ellipsis.
  #[serde(default)]
  pub ellipsis: bool,
}

/// A run of glyphs in one font.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Run {
  /// The text it draws.
  #[serde(flatten)]
  pub range: Range,
  /// The line it is on.
  pub line: usize,
  /// The font's family name, as the platform names it.
  #[serde(default)]
  pub family: Option<String>,
  /// The font's PostScript name.
  #[serde(default)]
  pub postscript: Option<String>,
  /// The font's file, where the platform tells it.
  #[serde(default)]
  pub path: Option<String>,
  /// The face's index in its file (a collection).
  #[serde(default)]
  pub index: Option<u32>,
  /// The font size it is drawn at.
  pub size: f32,
  /// The face's weight.
  #[serde(default)]
  pub weight: Option<f32>,
  /// Whether the face is italic or oblique.
  #[serde(default)]
  pub italic: Option<bool>,
  /// Whether bold is synthesized.
  #[serde(default)]
  pub synthetic_bold: bool,
  /// Whether the slant is synthesized.
  #[serde(default)]
  pub synthetic_italic: bool,
  /// Whether the run is right to left.
  #[serde(default)]
  pub rtl: bool,
}

/// One cluster: the text one or more glyphs draw together.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Cluster {
  /// The text it draws.
  #[serde(flatten)]
  pub range: Range,
  /// The line it is on.
  pub line: usize,
  /// Its left edge.
  pub x: f32,
  /// Its advance.
  pub advance: f32,
}

/// The intrinsic widths taffy asks a text leaf for.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Intrinsic {
  /// Min-content: every break opportunity taken.
  pub min: f32,
  /// Max-content: no soft break taken.
  pub max: f32,
}

/// One variant's timings, in milliseconds.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct VariantTimings {
  /// The first layout of the variant in the run (shaping and breaking).
  pub cold_ms: f64,
  /// The median of later layouts of it.
  pub warm_ms: f64,
}

/// A font as a platform rasterizer opens it.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct FontIdentity {
  /// The family name.
  pub family: String,
  /// The PostScript name.
  pub postscript: String,
  /// The file, where the font came from one.
  pub path: Option<String>,
  /// The face's index in the file.
  pub index: u32,
  /// The file's length in bytes.
  pub data_len: usize,
}

/// A variation axis's user-space value.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Variation {
  /// The axis tag, `wght`.
  pub tag: String,
  /// The value, `700`.
  pub value: f32,
}

/// A glyph to draw.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct GlyphRecord {
  /// The glyph's id in its font.
  pub id: u32,
  /// Its origin, from the layout's left.
  pub x: f32,
  /// Its baseline, from the layout's top.
  pub y: f32,
  /// The byte where its cluster starts.
  pub cluster: usize,
}

/// A run of glyphs in one font, exactly as Parley placed them: what a platform rasterizer
/// draws for option C.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct GlyphRunRecord {
  /// The line it is on.
  pub line: usize,
  /// The font.
  pub font: FontIdentity,
  /// The font size in logical pixels per em.
  pub size: f32,
  /// The normalized variation coordinates (F2Dot14 bits), in the font's axis order.
  pub coords: Vec<i16>,
  /// The user-space values the coordinates come from.
  pub variations: Vec<Variation>,
  /// Whether to embolden the outlines.
  pub embolden: bool,
  /// The synthetic slant, in degrees, where one is asked.
  pub skew: Option<f32>,
  /// The glyphs.
  pub glyphs: Vec<GlyphRecord>,
}

/// One variant as an engine laid it out.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Record {
  /// The variant's id, `case/size/width`.
  pub variant: String,
  /// The lines, top to bottom; an empty text has one.
  pub lines: Vec<Line>,
  /// The widest line's `width`.
  pub width: f32,
  /// The layout's height: the last line box's bottom.
  pub height: f32,
  /// Whether `max_lines` cut text off.
  #[serde(default)]
  pub truncated: bool,
  /// The runs, line by line in visual order.
  #[serde(default)]
  pub runs: Vec<Run>,
  /// The clusters, line by line in visual order, where the engine reports them.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub clusters: Vec<Cluster>,
  /// Min- and max-content widths, where the engine reports them.
  #[serde(default)]
  pub intrinsic: Option<Intrinsic>,
  /// Timings, where taken.
  #[serde(default)]
  pub timings: Option<VariantTimings>,
  /// The image at each scale (`"2"`), a path relative to the record file.
  #[serde(default)]
  pub images: BTreeMap<String, String>,
  /// Parley's glyph runs, for a platform rasterizer.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub glyph_runs: Vec<GlyphRunRecord>,
  /// What the engine could not honour (`word_break phrase unsupported`).
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub notes: Vec<String>,
}

/// The image key of a scale: `1`, `2`, `2.5`.
#[must_use]
pub fn scale_key(scale: f32) -> String {
  crate::corpus::number(scale)
}

/// The pixel size of an image of a layout `width` by `height` at `scale`, with the margin.
#[must_use]
pub fn image_size(width: f32, height: f32, scale: f32) -> (u32, u32) {
  let pixels = |extent: f32| {
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "an image's extent"
    )]
    let pixels = ((extent.max(0.).ceil() + 2. * MARGIN) * scale).round() as u32;

    pixels.max(1)
  };

  (pixels(width), pixels(height))
}

impl Record {
  /// The box an image of this record covers before the margin: the variant's width where it
  /// has one, else the record's own width; the record's height.
  #[must_use]
  pub fn image_box(&self, width: Option<f32>) -> (f32, f32) {
    (width.unwrap_or(self.width), self.height)
  }
}

impl RecordSet {
  /// Reads a record set.
  ///
  /// # Errors
  ///
  /// Where the file cannot be read or does not hold a record set.
  pub fn read(path: &Path) -> Result<Self, String> {
    let json =
      std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;

    serde_json::from_str(&json).map_err(|error| format!("{}: {error}", path.display()))
  }

  /// Writes a record set, pretty-printed.
  ///
  /// # Errors
  ///
  /// Where the file cannot be written.
  pub fn write(&self, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;

    std::fs::write(path, json + "\n").map_err(|error| format!("{}: {error}", path.display()))
  }

  /// The record of `variant`.
  #[must_use]
  pub fn record(&self, variant: &str) -> Option<&Record> {
    self.records.iter().find(|record| record.variant == variant)
  }

  /// Fills what a native record may leave out: each range's other unit, and a run's family
  /// and PostScript name from its font file where only the file is given.
  pub fn normalize(&mut self, corpus: &Corpus) {
    let mut names = FontNames::default();

    for record in &mut self.records {
      let Some(variant) = corpus.variant(&record.variant)
      else {
        continue;
      };
      let offsets = Offsets::new(&variant.case.text);

      for line in &mut record.lines {
        line.range.fill(&offsets);
      }

      for run in &mut record.runs {
        run.range.fill(&offsets);

        if let (Some(path), true) = (&run.path, run.family.is_none() || run.postscript.is_none())
          && let Some((family, postscript)) = names.of_file(Path::new(path), run.index.unwrap_or(0))
        {
          run.family.get_or_insert(family);
          run.postscript.get_or_insert(postscript);
        }
      }

      for cluster in &mut record.clusters {
        cluster.range.fill(&offsets);
      }
    }
  }

  /// What is wrong with the set against `corpus`, and with its images under `root`: one line
  /// per problem, none where it holds.
  #[must_use]
  pub fn validate(&self, corpus: &Corpus, root: &Path) -> Vec<String> {
    let mut problems = Vec::new();

    if self.schema != SCHEMA {
      problems.push(format!("schema {} (this tool reads {SCHEMA})", self.schema));
    }

    if self.corpus_version != corpus.version {
      problems.push(format!(
        "corpus version {} (the corpus is {})",
        self.corpus_version, corpus.version
      ));
    }

    for variant in corpus.variants() {
      if self.record(&variant.id()).is_none() {
        problems.push(format!("{}: missing", variant.id()));
      }
    }

    for record in &self.records {
      let Some(variant) = corpus.variant(&record.variant)
      else {
        problems.push(format!("{}: not a variant of the corpus", record.variant));

        continue;
      };
      let text = &variant.case.text;
      let offsets = Offsets::new(text);
      let mut problem = |what: String| problems.push(format!("{}: {what}", record.variant));

      if record.lines.is_empty() {
        problem("no lines (an empty text has one)".to_owned());
      }

      let mut previous = 0;

      for (at, line) in record.lines.iter().enumerate() {
        if line.range.utf8.is_none() && line.range.utf16.is_none() {
          problem(format!("line {at} has no range"));
        }

        let [start, end] = line.range.bytes(&offsets);

        if start < previous || end < start || end > text.len() {
          problem(format!("line {at} range {start}..{end} out of order"));
        }

        if let (Some(utf8), Some(utf16)) = (line.range.utf8, line.range.utf16)
          && offsets.range_utf16(utf8) != utf16
        {
          problem(format!(
            "line {at}: utf8 {utf8:?} and utf16 {utf16:?} disagree"
          ));
        }

        previous = start;
      }

      for (at, run) in record.runs.iter().enumerate() {
        let [start, end] = run.range.bytes(&offsets);

        if end < start || end > text.len() {
          problem(format!("run {at} range {start}..{end} outside the text"));
        }

        if run.family.is_none() && run.postscript.is_none() && run.path.is_none() {
          problem(format!("run {at} names no font"));
        }

        if run.line >= record.lines.len() {
          problem(format!(
            "run {at} on line {} of {}",
            run.line,
            record.lines.len()
          ));
        }
      }

      for (scale, image) in &record.images {
        let path = root.join(image);
        let Ok(scale) = scale.parse::<f32>()
        else {
          problem(format!("image scale {scale:?} is not a number"));

          continue;
        };

        match crate::image::Rgba::read(&path) {
          Ok(read) => {
            let (box_width, box_height) = record.image_box(variant.width);
            let (width, height) = image_size(box_width, box_height, scale);

            if (read.width, read.height) != (width, height) {
              problem(format!(
                "image @{scale}x is {}x{}, the convention gives {width}x{height}",
                read.width, read.height
              ));
            }
          }
          Err(error) => problem(format!("image @{scale}x: {error}")),
        }
      }
    }

    problems
  }

  /// A one-paragraph summary: engine, platform and counts.
  #[must_use]
  pub fn summary(&self) -> String {
    let mut out = String::new();
    let _ = write!(
      out,
      "{} {} drawn by {} on {} {}: {} records",
      self.engine.name,
      self.engine.version,
      self.engine.rasterizer,
      self.platform.os,
      self.platform.os_version,
      self.records.len()
    );

    out
  }
}

/// The directory a record file's images are relative to.
#[must_use]
pub fn root_of(record_file: &Path) -> PathBuf {
  record_file
    .parent()
    .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}
