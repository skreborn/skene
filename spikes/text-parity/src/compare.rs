//! Two record sets of one corpus compared: line breaks, line and layout metrics, the font of
//! every character, cluster advances, intrinsic widths, timings and pixels.
//!
//! The first set (`a`) is the reference, the platform's own engine; the second (`b`) is the
//! candidate, Parley. Every delta is `b - a`.

use std::{
  collections::BTreeMap,
  fmt::Write as _,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
  corpus::Corpus,
  image::{self, PixelMetrics, Rgba},
  record::{Record, RecordSet, VariantTimings},
  text::{Offsets, is_invisible},
};

/// How to compare.
#[derive(Clone, PartialEq, Debug)]
pub struct Options {
  /// A pixel differs where its largest channel difference exceeds this.
  pub threshold: u8,
  /// The scale the per-variant table shows; the largest scale both sets drew where `None`.
  pub scale: Option<f32>,
  /// Where to write a difference image per variant at that scale.
  pub diff_dir: Option<PathBuf>,
  /// A line width or metric agrees within this many logical pixels.
  pub tolerance: f32,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      threshold: 64,
      scale: None,
      diff_dir: None,
      tolerance: 0.5,
    }
  }
}

/// Agreement on which font draws each character.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct FontAgreement {
  /// Visible characters both sets drew.
  pub characters: usize,
  /// Of those, the ones drawn in the same family.
  pub same_family: usize,
  /// Characters both sets named a PostScript name for.
  pub with_postscript: usize,
  /// Of those, the ones drawn in the same PostScript font.
  pub same_postscript: usize,
  /// Each distinct disagreement: the character, `a`'s family, `b`'s family.
  pub differing: Vec<(String, String, String)>,
}

/// Agreement on cluster advances, over clusters both sets drew with the same range.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct ClusterAgreement {
  /// Clusters matched.
  pub matched: usize,
  /// Clusters in `a`.
  pub of_a: usize,
  /// Clusters in `b`.
  pub of_b: usize,
  /// The mean absolute advance difference.
  pub mean_abs: f32,
  /// The largest absolute advance difference.
  pub max_abs: f32,
}

/// One variant compared.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct VariantComparison {
  /// The variant.
  pub variant: String,
  /// Its case.
  pub case: String,
  /// The case's tags.
  pub tags: Vec<String>,
  /// Line counts in `a` and `b`.
  pub lines: (usize, usize),
  /// Whether every line starts at the same byte (the line counts equal).
  pub breaks_identical: bool,
  /// The lines' texts where breaks differ, `a`'s then `b`'s.
  pub break_texts: Option<(Vec<String>, Vec<String>)>,
  /// Whether each set cut the text.
  pub truncated: (bool, bool),
  /// Per line, the difference of widths, over lines both have.
  pub width_deltas: Vec<f32>,
  /// The largest absolute width difference.
  pub max_width_delta: f32,
  /// The layout's width difference.
  pub layout_width_delta: f32,
  /// The layout's height difference.
  pub height_delta: f32,
  /// The first baseline's difference.
  pub first_baseline_delta: Option<f32>,
  /// The largest absolute baseline difference, over lines both have.
  pub max_baseline_delta: f32,
  /// The largest absolute line-height difference, over lines both have.
  pub max_line_height_delta: f32,
  /// The largest absolute ascent and descent differences, over lines both have.
  pub max_ascent_descent_delta: f32,
  /// Fonts per character.
  pub fonts: FontAgreement,
  /// Cluster advances, where both report clusters.
  pub clusters: Option<ClusterAgreement>,
  /// Min- and max-content differences, where both report them.
  pub intrinsic_delta: Option<(f32, f32)>,
  /// Pixel metrics per scale both drew.
  pub pixels: BTreeMap<String, PixelMetrics>,
  /// Each set's timings.
  pub timings: (Option<VariantTimings>, Option<VariantTimings>),
  /// Each set's notes.
  pub notes: (Vec<String>, Vec<String>),
}

/// Totals over a group of variants.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Totals {
  /// Variants compared.
  pub variants: usize,
  /// With identical breaks.
  pub breaks_identical: usize,
  /// With equal line counts.
  pub lines_equal: usize,
  /// With every line's width within the tolerance.
  pub widths_within: usize,
  /// With the height within the tolerance.
  pub heights_within: usize,
  /// The mean absolute height difference.
  pub mean_abs_height_delta: f64,
  /// Visible characters compared for fonts.
  pub characters: usize,
  /// Of those, drawn in the same family.
  pub same_family: usize,
  /// Variants with pixels at the table's scale.
  pub imaged: usize,
  /// The mean of their mean absolute differences.
  pub mean_abs_diff: f64,
  /// The mean of their shares of pixels over the threshold.
  pub mean_share_over: f64,
  /// The mean of their SSIMs.
  pub mean_ssim: f64,
  /// The mean of their ink IoUs.
  pub mean_ink_iou: f64,
  /// Cold layout in all, `a` then `b`, over variants both timed.
  pub cold_ms: (f64, f64),
  /// Warm layout in all, `a` then `b`, over variants both timed.
  pub warm_ms: (f64, f64),
}

/// Two record sets compared.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Comparison {
  /// `a`'s summary.
  pub a: String,
  /// `b`'s summary.
  pub b: String,
  /// The scale the table shows.
  pub scale: Option<String>,
  /// The threshold.
  pub threshold: u8,
  /// The tolerance.
  pub tolerance: f32,
  /// Every variant both have.
  pub variants: Vec<VariantComparison>,
  /// Variants of the corpus a set lacks: `a` or `b`, and the variant.
  pub missing: Vec<(String, String)>,
  /// Totals over every variant.
  pub totals: Totals,
  /// Totals per tag.
  pub by_tag: BTreeMap<String, Totals>,
  /// Run-wide timings, `a` then `b`, as JSON.
  pub set_timings: (serde_json::Value, serde_json::Value),
}

/// Two family names name one family: equal ignoring case, spaces and hyphens, or one is the
/// other with a face name after it (`Noto Sans CJK JP Regular`).
#[must_use]
pub fn same_family(a: &str, b: &str) -> bool {
  let squash = |name: &str| {
    name
      .chars()
      .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
      .flat_map(char::to_lowercase)
      .collect::<String>()
  };
  let (a, b) = (squash(a), squash(b));

  !a.is_empty() && (a == b || a.starts_with(&b) || b.starts_with(&a))
}

fn family_at(record: &Record, offsets: &Offsets, byte: usize) -> Option<(String, Option<String>)> {
  record.runs.iter().find_map(|run| {
    let [start, end] = run.range.bytes(offsets);

    (start <= byte && byte < end).then(|| {
      (
        run
          .family
          .clone()
          .or_else(|| run.postscript.clone())
          .or_else(|| run.path.clone())
          .unwrap_or_default(),
        run.postscript.clone(),
      )
    })
  })
}

fn fonts(text: &str, offsets: &Offsets, a: &Record, b: &Record) -> FontAgreement {
  let mut agreement = FontAgreement::default();

  for (byte, character) in text.char_indices() {
    if is_invisible(character) {
      continue;
    }

    let (Some((family_a, post_a)), Some((family_b, post_b))) =
      (family_at(a, offsets, byte), family_at(b, offsets, byte))
    else {
      continue;
    };

    agreement.characters += 1;

    if same_family(&family_a, &family_b) {
      agreement.same_family += 1;
    }
    else {
      let entry = (character.to_string(), family_a, family_b);

      if !agreement.differing.contains(&entry) {
        agreement.differing.push(entry);
      }
    }

    if let (Some(post_a), Some(post_b)) = (
      post_a.filter(|p| !p.is_empty()),
      post_b.filter(|p| !p.is_empty()),
    ) {
      agreement.with_postscript += 1;
      agreement.same_postscript += usize::from(post_a.eq_ignore_ascii_case(&post_b));
    }
  }

  agreement
}

fn clusters(offsets: &Offsets, a: &Record, b: &Record) -> Option<ClusterAgreement> {
  if a.clusters.is_empty() || b.clusters.is_empty() {
    return None;
  }

  let advances: BTreeMap<[usize; 2], f32> = b
    .clusters
    .iter()
    .map(|cluster| (cluster.range.bytes(offsets), cluster.advance))
    .collect();
  let deltas: Vec<f32> = a
    .clusters
    .iter()
    .filter_map(|cluster| {
      advances
        .get(&cluster.range.bytes(offsets))
        .map(|advance| (advance - cluster.advance).abs())
    })
    .collect();

  #[expect(clippy::cast_precision_loss, reason = "cluster counts")]
  Some(ClusterAgreement {
    matched: deltas.len(),
    of_a: a.clusters.len(),
    of_b: b.clusters.len(),
    mean_abs: deltas.iter().sum::<f32>() / deltas.len().max(1) as f32,
    max_abs: deltas.iter().copied().fold(0., f32::max),
  })
}

fn line_texts(text: &str, offsets: &Offsets, record: &Record) -> Vec<String> {
  record
    .lines
    .iter()
    .map(|line| {
      let [start, end] = line.range.bytes(offsets);

      text.get(start..end).unwrap_or_default().to_owned()
    })
    .collect()
}

fn max_abs(values: impl Iterator<Item = f32>) -> f32 {
  values.map(f32::abs).fold(0., f32::max)
}

/// The key of the largest scale both records drew.
fn common_scales(a: &Record, b: &Record) -> Vec<String> {
  let mut scales: Vec<String> = a
    .images
    .keys()
    .filter(|scale| b.images.contains_key(*scale))
    .cloned()
    .collect();

  scales.sort_by(|x, y| {
    x.parse::<f32>()
      .unwrap_or_default()
      .total_cmp(&y.parse::<f32>().unwrap_or_default())
  });

  scales
}

/// Compares `variant`'s records.
fn compare_variant(
  corpus: &Corpus,
  (a, root_a): (&Record, &Path),
  (b, root_b): (&Record, &Path),
  options: &Options,
  table_scale: Option<&str>,
) -> Option<VariantComparison> {
  let variant = corpus.variant(&a.variant)?;
  let text = &variant.case.text;
  let offsets = Offsets::new(text);
  let starts = |record: &Record| -> Vec<usize> {
    record
      .lines
      .iter()
      .map(|line| line.range.bytes(&offsets)[0])
      .collect()
  };
  let breaks_identical = starts(a) == starts(b);
  let common = a.lines.len().min(b.lines.len());
  let pairs = || a.lines.iter().zip(&b.lines).take(common);
  let width_deltas: Vec<f32> = pairs().map(|(a, b)| b.width - a.width).collect();
  let mut pixels = BTreeMap::new();

  for scale in common_scales(a, b) {
    let (Ok(image_a), Ok(image_b)) = (
      Rgba::read(&root_a.join(&a.images[&scale])),
      Rgba::read(&root_b.join(&b.images[&scale])),
    )
    else {
      continue;
    };

    if table_scale == Some(scale.as_str())
      && let Some(dir) = &options.diff_dir
    {
      let _ = std::fs::create_dir_all(dir);
      let _ = image::difference(&image_a, &image_b)
        .write(&dir.join(format!("{}@{scale}x.png", variant.file_stem())));
    }

    pixels.insert(scale, image::compare(&image_a, &image_b, options.threshold));
  }

  Some(VariantComparison {
    variant: a.variant.clone(),
    case: variant.case.id.clone(),
    tags: variant.case.tags.clone(),
    lines: (a.lines.len(), b.lines.len()),
    breaks_identical,
    break_texts: (!breaks_identical)
      .then(|| (line_texts(text, &offsets, a), line_texts(text, &offsets, b))),
    truncated: (a.truncated, b.truncated),
    max_width_delta: max_abs(width_deltas.iter().copied()),
    width_deltas,
    layout_width_delta: b.width - a.width,
    height_delta: b.height - a.height,
    first_baseline_delta: a
      .lines
      .first()
      .zip(b.lines.first())
      .map(|(a, b)| b.baseline - a.baseline),
    max_baseline_delta: max_abs(pairs().map(|(a, b)| b.baseline - a.baseline)),
    max_line_height_delta: max_abs(pairs().map(|(a, b)| b.height - a.height)),
    max_ascent_descent_delta: max_abs(
      pairs().flat_map(|(a, b)| [b.ascent - a.ascent, b.descent - a.descent]),
    ),
    fonts: fonts(text, &offsets, a, b),
    clusters: clusters(&offsets, a, b),
    intrinsic_delta: a
      .intrinsic
      .zip(b.intrinsic)
      .map(|(a, b)| (b.min - a.min, b.max - a.max)),
    pixels,
    timings: (a.timings, b.timings),
    notes: (a.notes.clone(), b.notes.clone()),
  })
}

impl Totals {
  fn add(&mut self, variant: &VariantComparison, scale: Option<&str>, tolerance: f32) {
    self.variants += 1;
    self.breaks_identical += usize::from(variant.breaks_identical);
    self.lines_equal += usize::from(variant.lines.0 == variant.lines.1);
    self.widths_within += usize::from(variant.max_width_delta <= tolerance);
    self.heights_within += usize::from(variant.height_delta.abs() <= tolerance);
    self.mean_abs_height_delta += f64::from(variant.height_delta.abs());
    self.characters += variant.fonts.characters;
    self.same_family += variant.fonts.same_family;

    if let Some(pixels) = scale.and_then(|scale| variant.pixels.get(scale)) {
      self.imaged += 1;
      self.mean_abs_diff += pixels.mean_abs_diff;
      self.mean_share_over += pixels.share_over;
      self.mean_ssim += pixels.ssim;
      self.mean_ink_iou += pixels.ink_iou;
    }

    if let (Some(a), Some(b)) = variant.timings {
      self.cold_ms.0 += a.cold_ms;
      self.cold_ms.1 += b.cold_ms;
      self.warm_ms.0 += a.warm_ms;
      self.warm_ms.1 += b.warm_ms;
    }
  }

  fn finish(&mut self) {
    #[expect(clippy::cast_precision_loss, reason = "variant counts")]
    let (variants, imaged) = (self.variants.max(1) as f64, self.imaged.max(1) as f64);

    self.mean_abs_height_delta /= variants;
    self.mean_abs_diff /= imaged;
    self.mean_share_over /= imaged;
    self.mean_ssim /= imaged;
    self.mean_ink_iou /= imaged;
  }
}

/// Compares set `a` (under `root_a`) with set `b` (under `root_b`) over `corpus`.
#[must_use]
pub fn compare(
  corpus: &Corpus,
  (a, root_a): (&RecordSet, &Path),
  (b, root_b): (&RecordSet, &Path),
  options: &Options,
) -> Comparison {
  let mut a = a.clone();
  let mut b = b.clone();

  a.normalize(corpus);
  b.normalize(corpus);

  let mut missing = Vec::new();
  let mut variants = Vec::new();
  let table_scale = options.scale.map(crate::record::scale_key).or_else(|| {
    a.scales
      .iter()
      .filter(|scale| b.scales.contains(scale))
      .copied()
      .fold(None, |best: Option<f32>, scale| {
        Some(best.map_or(scale, |best| best.max(scale)))
      })
      .map(crate::record::scale_key)
  });

  for variant in corpus.variants() {
    let id = variant.id();

    match (a.record(&id), b.record(&id)) {
      (Some(record_a), Some(record_b)) => variants.extend(compare_variant(
        corpus,
        (record_a, root_a),
        (record_b, root_b),
        options,
        table_scale.as_deref(),
      )),
      (None, _) => missing.push(("a".to_owned(), id)),
      (_, None) => missing.push(("b".to_owned(), id)),
    }
  }

  let mut totals = Totals::default();
  let mut by_tag: BTreeMap<String, Totals> = BTreeMap::new();

  for variant in &variants {
    totals.add(variant, table_scale.as_deref(), options.tolerance);

    for tag in &variant.tags {
      by_tag.entry(tag.clone()).or_default().add(
        variant,
        table_scale.as_deref(),
        options.tolerance,
      );
    }
  }

  totals.finish();
  by_tag.values_mut().for_each(Totals::finish);

  Comparison {
    a: a.summary(),
    b: b.summary(),
    scale: table_scale,
    threshold: options.threshold,
    tolerance: options.tolerance,
    variants,
    missing,
    totals,
    by_tag,
    set_timings: (
      serde_json::to_value(&a.timings).unwrap_or_default(),
      serde_json::to_value(&b.timings).unwrap_or_default(),
    ),
  }
}

/// A line's text as a table cell shows it: blanks and breaks visible, at most 40 characters.
fn shown(text: &str) -> String {
  let mut out = String::new();

  for (at, character) in text.chars().enumerate() {
    if at == 40 {
      out.push('…');

      break;
    }

    match character {
      '\n' => out.push('⏎'),
      '\t' => out.push('⇥'),
      '\u{AD}' => out.push_str("[shy]"),
      '|' => out.push_str("\\|"),
      '`' => out.push('\''),
      character => out.push(character),
    }
  }

  format!("`{out}`")
}

fn percent(part: usize, whole: usize) -> String {
  #[expect(clippy::cast_precision_loss, reason = "counts")]
  let share = 100. * part as f64 / whole.max(1) as f64;

  format!("{part}/{whole} ({share:.0}%)")
}

fn totals_row(out: &mut String, name: &str, totals: &Totals) {
  let pixels = if totals.imaged == 0 {
    "- | - | - | -".to_owned()
  }
  else {
    format!(
      "{:.2} | {:.2}% | {:.3} | {:.3}",
      totals.mean_abs_diff,
      totals.mean_share_over * 100.,
      totals.mean_ssim,
      totals.mean_ink_iou
    )
  };
  let _ = writeln!(
    out,
    "| {name} | {} | {} | {} | {} | {} | {:.2} | {} | {pixels} |",
    totals.variants,
    percent(totals.breaks_identical, totals.variants),
    percent(totals.lines_equal, totals.variants),
    percent(totals.widths_within, totals.variants),
    percent(totals.heights_within, totals.variants),
    totals.mean_abs_height_delta,
    percent(totals.same_family, totals.characters),
  );
}

impl Comparison {
  /// The comparison as markdown: totals, totals per tag, then a table per case.
  #[must_use]
  pub fn markdown(&self, corpus: &Corpus) -> String {
    let mut out = String::new();
    let scale = self.scale.clone().unwrap_or_else(|| "-".to_owned());

    let _ = writeln!(out, "# Text parity\n");
    let _ = writeln!(out, "- **a** (reference): {}", self.a);
    let _ = writeln!(out, "- **b** (candidate): {}", self.b);
    let _ = writeln!(
      out,
      "- deltas are b - a in logical pixels; a width or height agrees within {}; pixels at {scale}x, \
       a pixel differs beyond {} of 255 in a channel\n",
      self.tolerance, self.threshold
    );

    if !self.missing.is_empty() {
      let _ = writeln!(
        out,
        "Missing: {} variants ({})\n",
        self.missing.len(),
        self
          .missing
          .iter()
          .take(8)
          .map(|(set, id)| format!("{set}: {id}"))
          .collect::<Vec<_>>()
          .join(", ")
      );
    }

    let header = "| group | variants | breaks identical | line counts equal | widths within | heights within | mean abs Δheight | same family (chars) | mean MAD | mean >thr | mean SSIM | mean ink IoU |\n|---|---|---|---|---|---|---|---|---|---|---|---|";

    let _ = writeln!(out, "## Totals\n\n{header}");
    totals_row(&mut out, "**all**", &self.totals);

    for (tag, totals) in &self.by_tag {
      totals_row(&mut out, tag, totals);
    }

    let _ = writeln!(
      out,
      "\nLayout time over variants both timed: cold {:.1} ms (a) vs {:.1} ms (b), warm {:.1} ms vs {:.1} ms.\n",
      self.totals.cold_ms.0, self.totals.cold_ms.1, self.totals.warm_ms.0, self.totals.warm_ms.1
    );
    let _ = writeln!(
      out,
      "Run timings:\n\n- a: `{}`\n- b: `{}`\n",
      self.set_timings.0, self.set_timings.1
    );
    let _ = writeln!(out, "## Cases\n");

    for case in &corpus.cases {
      let rows: Vec<&VariantComparison> = self
        .variants
        .iter()
        .filter(|variant| variant.case == case.id)
        .collect();

      if rows.is_empty() {
        continue;
      }

      let _ = writeln!(out, "### {}\n\n{}\n", case.id, case.note);
      let _ = writeln!(
        out,
        "| variant | lines a/b | breaks | max abs Δwidth | Δheight | Δbaseline₁ | Δmin/Δmax content | same family | MAD | >thr | SSIM | ink IoU | Δink l,t,r,b (px) |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|"
      );

      for row in &rows {
        let pixels = self.scale.as_ref().and_then(|scale| row.pixels.get(scale));
        let cell = |value: Option<String>| value.unwrap_or_else(|| "-".to_owned());

        let _ = writeln!(
          out,
          "| {} | {}/{}{} | {} | {:.2} | {:+.2} | {} | {} | {} | {} | {} | {} | {} | {} |",
          row.variant,
          row.lines.0,
          row.lines.1,
          if row.truncated.0 || row.truncated.1 {
            format!(
              " (cut {}/{})",
              u8::from(row.truncated.0),
              u8::from(row.truncated.1)
            )
          }
          else {
            String::new()
          },
          if row.breaks_identical {
            "same"
          }
          else {
            "**differ**"
          },
          row.max_width_delta,
          row.height_delta,
          cell(row.first_baseline_delta.map(|delta| format!("{delta:+.2}"))),
          cell(
            row
              .intrinsic_delta
              .map(|(min, max)| format!("{min:+.1}/{max:+.1}"))
          ),
          percent(row.fonts.same_family, row.fonts.characters),
          cell(pixels.map(|p| format!("{:.2}", p.mean_abs_diff))),
          cell(pixels.map(|p| format!("{:.2}%", p.share_over * 100.))),
          cell(pixels.map(|p| format!("{:.3}", p.ssim))),
          cell(pixels.map(|p| format!("{:.3}", p.ink_iou))),
          cell(
            pixels
              .and_then(|p| p.ink_delta)
              .map(|d| format!("{},{},{},{}", d[0], d[1], d[2], d[3]))
          ),
        );
      }

      let _ = writeln!(out);

      for row in &rows {
        if let Some((lines_a, lines_b)) = &row.break_texts {
          let _ = writeln!(out, "- {} lines differ:", row.variant);
          let _ = writeln!(
            out,
            "  - a: {}",
            lines_a
              .iter()
              .map(|line| shown(line))
              .collect::<Vec<_>>()
              .join(" ")
          );
          let _ = writeln!(
            out,
            "  - b: {}",
            lines_b
              .iter()
              .map(|line| shown(line))
              .collect::<Vec<_>>()
              .join(" ")
          );
        }

        for (character, family_a, family_b) in row.fonts.differing.iter().take(6) {
          let _ = writeln!(
            out,
            "- {} `{character}`: a {family_a:?}, b {family_b:?}",
            row.variant
          );
        }

        let notes: Vec<&String> = row.notes.0.iter().chain(&row.notes.1).collect();

        if !notes.is_empty() {
          let _ = writeln!(
            out,
            "- {} notes: {}",
            row.variant,
            notes
              .iter()
              .map(|note| note.as_str())
              .collect::<Vec<_>>()
              .join("; ")
          );
        }
      }

      let _ = writeln!(out);
    }

    out
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn families() {
    assert!(same_family("Noto Sans CJK JP", "Noto Sans CJK JP Regular"));
    assert!(same_family("DejaVu Sans", "dejavu-sans"));
    assert!(!same_family("DejaVu Sans", "Inter"));
    assert!(!same_family("", ""));
  }
}
