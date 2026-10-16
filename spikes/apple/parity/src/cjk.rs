//! What tuned Parley still lays out differently from TextKit, one cause at a time: CJK
//! punctuation spacing against OpenType's spacing features, each run's font instance in
//! TextKit's lines against the instance Parley is given, and each fallback script shaped by
//! Core Text and by Parley in the same font.

use std::collections::{BTreeMap, BTreeSet};

use objc2_core_foundation::{CFRange, CFString};
use objc2_core_text::{CTFont, CTFontTableOptions};
use parley::PositionedLayoutItem;
use serde_json::json;
use skrifa::raw::{
  FontData,
  FontRead,
  tables::{gpos::Gpos, gsub::Gsub},
};
use text_parity::{corpus::Corpus, record::Record};

use crate::{
  coretext,
  isolate,
  platform::{as_ct, system_font},
  probes::{Report, ct_line, tuned_engine},
  textkit::{self, Setup},
  tuned::{Trak, TunedEngine},
};

/// The GPOS and GSUB feature tags of `font`.
fn features(font: &CTFont) -> BTreeSet<String> {
  let mut out = BTreeSet::new();
  let table = |tag: &str| {
    unsafe { font.table(coretext::tag_of(tag), CTFontTableOptions::NoOptions) }
      .map(|data| data.to_vec())
  };

  if let Some(data) = table("GPOS")
    && let Ok(gpos) = Gpos::read(FontData::new(&data))
    && let Ok(list) = gpos.feature_list()
  {
    out.extend(
      list
        .feature_records()
        .iter()
        .map(|record| record.feature_tag().to_string()),
    );
  }

  if let Some(data) = table("GSUB")
    && let Ok(gsub) = Gsub::read(FontData::new(&data))
    && let Ok(list) = gsub.feature_list()
  {
    out.extend(
      list
        .feature_records()
        .iter()
        .map(|record| record.feature_tag().to_string()),
    );
  }

  out
}

/// Each cluster's advance by its UTF-8 range.
fn advances(record: &Record) -> BTreeMap<[usize; 2], f32> {
  record
    .clusters
    .iter()
    .filter_map(|cluster| Some((cluster.range.utf8?, cluster.advance)))
    .collect()
}

/// CJK punctuation: TextKit's cluster advances against tuned Parley's with each feature set.
pub fn spacing(report: &mut Report) {
  report.line("\n[CJK punctuation spacing: TextKit's cluster advances against tuned Parley's, by OpenType feature]");

  let corpus = Corpus::builtin();
  let ids = [
    "han-zh-hans/17/max",
    "han-zh-hant/17/max",
    "han-ja/17/max",
    "han-ko/17/max",
    "ja-kinsoku/17/200",
    "zh-hans-paragraph/17/200",
  ];
  let sets: [&'static [&'static str]; 5] =
    [&[], &["chws"], &["halt"], &["palt"], &["chws", "palt"]];
  let mut engines: Vec<TunedEngine> = sets
    .iter()
    .map(|features| {
      let mut engine = tuned_engine();

      engine.tuning.features = features;
      engine
    })
    .collect();
  let mut rows = Vec::new();
  let base = system_font(17., 400, false);
  let base = as_ct(&base);

  for id in ids {
    let Some(variant) = corpus.variant(id)
    else {
      continue;
    };
    let a = textkit::variant(&variant, &Setup::default(), 0).record;
    let reference = advances(&a);
    let string = CFString::from_str(&variant.case.text);
    let language = CFString::from_str(&variant.case.locale);
    let fallback = unsafe {
      CTFont::for_string_with_language(
        base,
        &string,
        CFRange {
          location: 0,
          length: string.length(),
        },
        Some(&language),
      )
    };
    let tags: Vec<String> = features(&fallback)
      .into_iter()
      .filter(|tag| {
        [
          "chws", "halt", "palt", "vchw", "vhal", "vpal", "kern", "locl",
        ]
        .contains(&tag.as_str())
      })
      .collect();

    report.line(format!(
      "  {id}: {} has {}",
      coretext::info(&fallback).postscript,
      tags.join(" ")
    ));

    for (features, engine) in sets.iter().zip(engines.iter_mut()) {
      let laid = engine.lay_out(&variant);
      let record = engine.record(&variant, &laid, None);
      let ours = advances(&record);
      let mut differing = 0;
      let mut sum = 0f32;

      for (range, advance) in &reference {
        if let Some(other) = ours.get(range) {
          let delta = (other - advance).abs();

          sum += delta;

          if delta > 0.05 {
            differing += 1;
          }
        }
      }

      let width = |record: &Record| {
        record
          .lines
          .iter()
          .map(|line| line.width)
          .fold(0f32, f32::max)
      };

      report.line(format!(
        "    {:<12} clusters differing {differing}/{}, sum |delta| {sum:.2}, widest line {:.2} vs TextKit {:.2}, breaks {}",
        if features.is_empty() { "none".to_owned() } else { features.join("+") },
        reference.len(),
        width(&record),
        width(&a),
        record.lines.len() == a.lines.len()
          && record.lines.iter().zip(&a.lines).all(|(ours, theirs)| ours.range.utf8 == theirs.range.utf8)
      ));
      rows.push(json!({"variant": id, "features": features, "differing": differing, "clusters": reference.len(), "sum": sum}));
    }
  }

  report.put("cjk_spacing", json!(rows));
}

/// Each run's font and instance in TextKit's lines, against what Parley is given: the system
/// font's instance for every font with the axis.
pub fn instances(report: &mut Report) {
  report.line("\n[run instances: each font TextKit drew a variant with, its CTFontCopyVariation, and the instance Parley applies]");

  let corpus = Corpus::builtin();
  let mut engine = tuned_engine();
  let mut rows = Vec::new();

  for id in [
    "bidi-numbers/17/max",
    "arabic/17/max",
    "mixed-scripts/17/max",
    "devanagari/17/max",
    "sizes-latin/11/max",
    "sizes-arabic/34/max",
  ] {
    let Some(variant) = corpus.variant(id)
    else {
      continue;
    };
    let tk = textkit::variant(&variant, &Setup::default(), 0);
    let placed = isolate::placed(&tk.laid, variant.case, tk.record.truncated);
    let ours = engine.instance(variant.case, variant.size);
    let mut seen = BTreeSet::new();

    report.line(format!("  {id}: Parley's instance {ours:?}"));

    for run in placed {
      let info = coretext::info(&run.font);

      if !seen.insert(info.postscript.clone()) {
        continue;
      }

      let variation = coretext::variation(&run.font);
      let axes = coretext::axes(&run.font);

      report.line(format!(
        "    {} at {:.1}: variation {variation:?}; axes {axes:?}",
        info.postscript, info.size
      ));
      rows.push(
        json!({"variant": id, "font": info.postscript, "variation": variation, "parley": ours}),
      );
    }
  }

  report.put("run_instances", json!(rows));
}

/// Each fallback script in one line, shaped by Core Text (`CTLine` in the system font, the
/// fallback its own) and by tuned Parley: glyph ids as a multiset and in visual order, advance
/// totals, and the shaping tables of each font Core Text used.
pub fn scripts(report: &mut Report) {
  report
    .line("\n[shaping in the fallback fonts: Core Text's line against tuned Parley's, same fonts]");
  report.line("  variant: glyphs ct/parley | ids as multiset | ids in order | advance ct/parley | fonts: tables, trak at 17");

  let corpus = Corpus::builtin();
  let mut engine = tuned_engine();
  let base = system_font(17., 400, false);
  let base = as_ct(&base);
  let mut rows = Vec::new();

  for id in [
    "arabic/17/max",
    "arabic-diacritics/17/max",
    "bidi-numbers/17/max",
    "devanagari/17/max",
    "bengali/17/max",
    "tamil/17/max",
    "thai/17/max",
    "khmer/17/max",
    "lao/17/max",
    "burmese/17/max",
    "emoji-zwj/17/max",
    "emoji-flags/17/max",
    "emoji-keycaps/17/max",
    "emoji-skin-tones/17/max",
  ] {
    let Some(variant) = corpus.variant(id)
    else {
      continue;
    };
    let line = ct_line(&variant.case.text, base, Some(&variant.case.locale));
    let runs = coretext::runs(&line);
    let ct: Vec<(u32, f64)> = runs
      .iter()
      .flat_map(|run| {
        run
          .glyphs
          .iter()
          .map(|glyph| (u32::from(glyph.id), glyph.advance))
      })
      .collect();
    let laid = engine.lay_out(&variant);
    let mut ours: Vec<(u32, f32)> = Vec::new();

    for line in laid.laid.layout.lines() {
      for item in line.items() {
        let PositionedLayoutItem::GlyphRun(run) = item
        else {
          continue;
        };

        ours.extend(run.glyphs().map(|glyph| (glyph.id, glyph.advance)));
      }
    }

    let differing: Vec<String> = ct
      .iter()
      .zip(&ours)
      .filter(|(theirs, ours)| theirs.0 == ours.0 && (theirs.1 - f64::from(ours.1)).abs() > 0.05)
      .take(4)
      .map(|(theirs, ours)| format!("#{} {:.2}/{:.2}", theirs.0, theirs.1, ours.1))
      .collect();
    let mut ct_ids: Vec<u32> = ct.iter().map(|glyph| glyph.0).collect();
    let mut our_ids: Vec<u32> = ours.iter().map(|glyph| glyph.0).collect();
    let in_order = ct_ids == our_ids;

    ct_ids.sort_unstable();
    our_ids.sort_unstable();

    let mut fonts = Vec::new();
    let mut seen = BTreeSet::new();

    for run in &runs {
      let info = coretext::info(&run.font);

      if !seen.insert(info.postscript.clone()) {
        continue;
      }

      let tables: Vec<String> = coretext::tables(&run.font)
        .into_iter()
        .map(coretext::tag_name)
        .filter(|tag| {
          ["morx", "GSUB", "GPOS", "kerx", "kern", "trak", "ankr"].contains(&tag.as_str())
        })
        .collect();
      let trak = Trak::of(&run.font).map(|trak| trak.at(17.));

      fonts.push(format!(
        "{} [{}] trak {}",
        info.postscript,
        tables.join(" "),
        trak.map_or_else(|| "-".to_owned(), |value| format!("{value:.2}"))
      ));
    }

    let ct_total: f64 = ct.iter().map(|glyph| glyph.1).sum();
    let our_total: f32 = ours.iter().map(|glyph| glyph.1).sum();

    report.line(format!(
      "  {id}: {}/{} | {} | {} | {ct_total:.2}/{our_total:.2} | {}",
      ct.len(),
      ours.len(),
      ct_ids == our_ids,
      in_order,
      fonts.join("; ")
    ));

    if !differing.is_empty() {
      report.line(format!(
        "    advances differing (glyph ct/parley): {}",
        differing.join(", ")
      ));
    }
    rows.push(json!({
      "variant": id, "ct_glyphs": ct.len(), "parley_glyphs": ours.len(), "ids_multiset": ct_ids == our_ids,
      "ids_in_order": in_order, "ct_advance": ct_total, "parley_advance": our_total, "fonts": fonts,
    }));
  }

  report.put("script_shaping", json!(rows));
}
