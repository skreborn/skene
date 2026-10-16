//! The probes beside the corpus comparison: what the system font is and whether its bytes can
//! be read, where Chinese goes (PingFang and `hvgl`), shaping parity in SF at each size,
//! line metrics, Korean strategies, hyphenation points, emoji presentation, fontique's
//! start-up, and the animation and intrinsic timings.
#![allow(unused_unsafe)]

use std::{borrow::Cow, sync::Arc, time::Instant};

use objc2_core_foundation::{CFDictionary, CFIndex, CFRange, CFRetained, CFString, CFType};
use objc2_core_graphics::CGPath;
use objc2_core_text::{CTFont, CTLine, kCTFontAttributeName, kCTLanguageAttributeName};
use parley::{
  FontFamily,
  FontFamilyName,
  FontVariation,
  FontVariations,
  FontWeight,
  LayoutContext,
  PositionedLayoutItem,
  StyleProperty,
  fontique::{Blob, FallbackKey, FontInfoOverride, GenericFamily, Language, Script},
  setting::Tag,
};
use serde_json::{Value, json};
use skrifa::{FontRef, raw::TableProvider as _};
use text_parity::{
  bench::plain_case,
  corpus::{Corpus, Variant, WordBreak},
  fonts::{FontSource, Fonts},
  layout::{Options as LayoutOptions, ParleyEngine},
  record::RecordSet,
};

use crate::{
  coretext::{self, tag_name},
  platform::{self, as_ct, system_font},
  textkit::{self, Setup},
  tuned::{TunedEngine, Tuning},
};

/// What the probes found: a text summary and the same as JSON.
#[derive(Default)]
pub struct Report {
  /// The summary, line by line.
  pub text: String,
  /// Everything, by probe.
  pub json: serde_json::Map<String, Value>,
}

impl Report {
  /// Adds a summary line (and prints it).
  pub fn line(&mut self, line: impl AsRef<str>) {
    println!("{}", line.as_ref());
    self.text.push_str(line.as_ref());
    self.text.push('\n');
  }

  /// Stores a probe's data.
  pub fn put(&mut self, key: &str, value: Value) {
    self.json.insert(key.to_owned(), value);
  }
}

/// The resident memory of this process, in bytes.
#[must_use]
pub fn resident() -> u64 {
  let mut info: libc::mach_task_basic_info = unsafe { std::mem::zeroed() };
  let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
  unsafe extern "C" {
    static mach_task_self_: libc::mach_port_t;
  }

  let task = unsafe { mach_task_self_ };
  let status = unsafe {
    libc::task_info(
      task,
      libc::MACH_TASK_BASIC_INFO,
      std::ptr::from_mut(&mut info).cast(),
      &mut count,
    )
  };

  if status == 0 { info.resident_size } else { 0 }
}

/// A Core Text line of `text` in `font`, in `locale`.
#[must_use]
pub fn ct_line(text: &str, font: &CTFont, locale: Option<&str>) -> CFRetained<CTLine> {
  let string = CFString::from_str(text);
  let language = CFString::from_str(locale.unwrap_or("en"));
  let keys: [&CFString; 2] = unsafe { [kCTFontAttributeName, kCTLanguageAttributeName] };
  let values: [&CFType; 2] = [font, &language];
  let attributes = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
  let attributed = unsafe {
    objc2_core_foundation::CFAttributedString::new(
      None,
      Some(&string),
      Some(attributes.as_opaque()),
    )
  }
  .expect("an attributed string");

  coretext::line_of(&attributed)
}

/// One shaped glyph as a probe reads it.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Shaped {
  /// The family its font is registered under or named.
  pub family: String,
  /// The glyph id.
  pub id: u32,
  /// Its advance.
  pub advance: f32,
}

/// `text` shaped by Parley in one line in `family` from `fonts`, at `size`, with `variations`.
pub fn parley_glyphs(
  fonts: &mut Fonts,
  layouts: &mut LayoutContext<()>,
  families: &[FontFamilyName<'static>],
  text: &str,
  size: f32,
  weight: f32,
  locale: &str,
  variations: &[(Tag, f32)],
) -> Vec<Shaped> {
  let mut builder = layouts.ranged_builder(&mut fonts.context, text, 1., false);

  builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Owned(
    families.to_vec(),
  ))));
  builder.push_default(StyleProperty::FontSize(size));
  builder.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));

  if let Ok(language) = Language::parse(locale) {
    builder.push_default(StyleProperty::Locale(Some(language)));
  }

  if !variations.is_empty() {
    let list: Vec<FontVariation> = variations
      .iter()
      .map(|(tag, value)| FontVariation::new(*tag, *value))
      .collect();

    builder.push_default(StyleProperty::FontVariations(FontVariations::List(
      Cow::Owned(list),
    )));
  }

  let mut layout = builder.build(text);

  layout.break_all_lines(None);

  let mut out = Vec::new();

  for line in layout.lines() {
    for item in line.items() {
      let PositionedLayoutItem::GlyphRun(run) = item
      else {
        continue;
      };
      let identity = fonts.identity(run.run().font());

      for glyph in run.glyphs() {
        out.push(Shaped {
          family: identity.family.clone(),
          id: glyph.id,
          advance: glyph.advance,
        });
      }
    }
  }

  out
}

/// The glyphs Core Text's line of `text` draws: family, id, advance.
#[must_use]
pub fn ct_glyphs(text: &str, font: &CTFont, locale: Option<&str>) -> Vec<Shaped> {
  let line = ct_line(text, font, locale);
  let mut out = Vec::new();

  for run in coretext::runs(&line) {
    let family = unsafe { run.font.family_name() }.to_string();

    for glyph in run.glyphs {
      out.push(Shaped {
        family: family.clone(),
        id: u32::from(glyph.id),
        advance: glyph.advance as f32,
      });
    }
  }

  out
}

fn total(glyphs: &[Shaped]) -> f32 {
  glyphs.iter().map(|glyph| glyph.advance).sum()
}

fn ids(glyphs: &[Shaped]) -> Vec<u32> {
  glyphs.iter().map(|glyph| glyph.id).collect()
}

/// A fresh collection holding only `font` (as an sfnt from its tables) under `family`.
fn single(font: &CTFont, family: &str) -> (Fonts, Result<usize, String>) {
  single_except(font, family, &[])
}

/// [`single`] without the tables named in `skip`.
fn single_except(font: &CTFont, family: &str, skip: &[&str]) -> (Fonts, Result<usize, String>) {
  let mut fonts = Fonts::new(&FontSource {
    system: false,
    files: Vec::new(),
    generics: Vec::new(),
    fallbacks: Vec::new(),
  })
  .expect("empty");
  let Some(bytes) = coretext::sfnt_except(font, skip)
  else {
    return (fonts, Err("no tables".to_owned()));
  };
  let added = fonts.context.collection.register_fonts(
    Blob::new(Arc::new(bytes)),
    Some(FontInfoOverride {
      family_name: Some(family),
      ..FontInfoOverride::default()
    }),
  );

  if added.is_empty() {
    (fonts, Err("fontique registered no face".to_owned()))
  }
  else {
    (fonts, Ok(added.len()))
  }
}

fn readable(path: &str) -> String {
  use std::io::Read as _;

  match std::fs::File::open(path) {
    Ok(mut file) => {
      let mut head = [0u8; 4];

      match file.read_exact(&mut head) {
        Ok(()) => format!(
          "readable ({} bytes)",
          std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
        ),
        Err(error) => format!("opens, read fails: {error}"),
      }
    }
    Err(error) => format!("not readable: {error}"),
  }
}

/// The system font at each size and weight, the text styles, and whether their files can be
/// read and what tables they hold.
pub fn system_fonts(report: &mut Report) {
  report.line("\n[system font: names, files, tables, axes, instances]");

  let mut rows = Vec::new();
  let mut paths = std::collections::BTreeMap::new();

  for size in [11., 13., 17., 20., 28., 34., 64.] {
    let mut cells = Vec::new();

    for weight in [100u16, 200, 300, 400, 500, 600, 700, 800, 900] {
      let font = system_font(size, weight, false);
      let ct = as_ct(&font);
      let info = coretext::info(ct);

      if let Some(path) = &info.path {
        paths
          .entry(path.clone())
          .or_insert_with(|| (coretext::tables(ct), coretext::axes(ct)));
      }

      cells.push(format!(
        "{weight}:{} {}",
        info.postscript,
        info
          .variation
          .iter()
          .map(|(tag, value)| format!("{tag}={value:.1}"))
          .collect::<Vec<_>>()
          .join(",")
      ));
      rows.push(json!({"size": size, "weight": weight, "font": info}));
    }

    report.line(format!("  {size}pt  {}", cells.join(" | ")));
  }

  let italic = coretext::info(as_ct(&system_font(17., 400, true)));

  report.line(format!(
    "  italic 17pt: {} {} {:?}",
    italic.family, italic.postscript, italic.path
  ));

  if let Some(path) = &italic.path {
    paths.entry(path.clone()).or_insert_with(|| {
      let font = system_font(17., 400, true);

      (coretext::tables(as_ct(&font)), coretext::axes(as_ct(&font)))
    });
  }

  let mut files = Vec::new();

  for (path, (tables, axes)) in &paths {
    let names: Vec<String> = tables.iter().map(|tag| tag_name(*tag)).collect();
    let interesting: Vec<&str> = [
      "trak", "STAT", "fvar", "avar", "morx", "kerx", "GSUB", "GPOS", "glyf", "CFF ", "CFF2",
      "sbix", "COLR", "hvgl", "opsz",
    ]
    .into_iter()
    .filter(|tag| names.iter().any(|name| name == tag))
    .collect();

    report.line(format!("  file {path}: {}", readable(path)));
    report.line(format!(
      "    tables ({}): has {}",
      names.len(),
      interesting.join(" ")
    ));
    report.line(format!(
      "    axes: {}",
      axes
        .iter()
        .map(|axis| format!(
          "{} {}..{} default {}",
          axis.tag, axis.min, axis.max, axis.default
        ))
        .collect::<Vec<_>>()
        .join("; ")
    ));
    files.push(json!({"path": path, "readable": readable(path), "tables": names, "axes": axes}));
  }

  let mut styles = Vec::new();

  for (name, font) in platform::text_styles() {
    let info = coretext::info(as_ct(&font));

    report.line(format!(
      "  text style {name}: {} {}pt {} {}",
      info.postscript,
      info.size,
      info
        .variation
        .iter()
        .map(|(tag, value)| format!("{tag}={value:.1}"))
        .collect::<Vec<_>>()
        .join(","),
      info
        .path
        .as_deref()
        .map(readable)
        .unwrap_or_else(|| "no URL".to_owned())
    ));
    styles.push(json!({"style": name, "font": info}));
  }

  let sfnt_started = Instant::now();
  let regular = system_font(17., 400, false);
  let bytes = coretext::sfnt(as_ct(&regular)).map_or(0, |bytes| bytes.len());
  let sfnt_ms = sfnt_started.elapsed().as_secs_f64() * 1e3;

  report.line(format!(
    "  sfnt from CTFontCopyTable for the system font: {bytes} bytes in {sfnt_ms:.1} ms"
  ));
  report.put("system_fonts", json!({"rows": rows, "files": files, "styles": styles, "italic": italic, "sfnt_bytes": bytes, "sfnt_ms": sfnt_ms}));
}

/// fontique's system collection: its start-up time and memory, its generic families and its
/// fallbacks for the scripts the corpus needs.
pub fn fontique(report: &mut Report) -> Fonts {
  report.line("\n[fontique system collection]");

  let before = resident();
  let started = Instant::now();
  let mut fonts = Fonts::system();
  let ms = started.elapsed().as_secs_f64() * 1e3;
  let after = resident();
  let collection = &mut fonts.context.collection;
  let families = collection.family_names().count();
  let mut generics = serde_json::Map::new();

  for (name, generic) in [
    ("system-ui", GenericFamily::SystemUi),
    ("sans-serif", GenericFamily::SansSerif),
    ("serif", GenericFamily::Serif),
    ("monospace", GenericFamily::Monospace),
    ("emoji", GenericFamily::Emoji),
  ] {
    let ids: Vec<_> = collection.generic_families(generic).collect();
    let names: Vec<String> = ids
      .iter()
      .filter_map(|id| collection.family_name(*id).map(str::to_owned))
      .collect();

    report.line(format!("  generic {name}: {}", names.join(", ")));
    generics.insert(name.to_owned(), json!(names));
  }

  let mut fallbacks = serde_json::Map::new();

  for (script, locale) in [
    ("Hani", Some("zh-Hans")),
    ("Hani", Some("zh-Hant")),
    ("Hani", Some("zh-HK")),
    ("Hani", Some("ja")),
    ("Hani", Some("ko")),
    ("Hani", None),
    ("Hira", Some("ja")),
    ("Hang", Some("ko")),
    ("Thai", None),
    ("Khmr", None),
    ("Laoo", None),
    ("Mymr", None),
    ("Arab", None),
    ("Hebr", None),
    ("Deva", None),
    ("Beng", None),
    ("Taml", None),
  ] {
    let language = locale.and_then(|locale| Language::parse(locale).ok());
    let Ok(parsed) = Script::parse(script)
    else {
      continue;
    };
    let ids: Vec<_> = collection
      .fallback_families(FallbackKey::new(parsed, language.as_ref()))
      .collect();
    let names: Vec<String> = ids
      .iter()
      .filter_map(|id| collection.family_name(*id).map(str::to_owned))
      .collect();
    let key = format!("{script}/{}", locale.unwrap_or("-"));

    report.line(format!("  fallback {key}: {}", names.join(", ")));
    fallbacks.insert(key, json!(names));
  }

  report.line(format!(
    "  start-up {ms:.1} ms, {families} families, resident {:.1} MB -> {:.1} MB (+{:.1} MB)",
    before as f64 / 1e6,
    after as f64 / 1e6,
    (after as f64 - before as f64) / 1e6
  ));
  report.put("fontique", json!({"startup_ms": ms, "families": families, "resident_before": before, "resident_after": after, "generics": generics, "fallbacks": fallbacks}));

  fonts
}

/// Where Chinese goes: Core Text's pick per language, whether it is an `hvgl` font, whether its
/// outlines and tables reach Parley, and what stock fontique drew instead.
pub fn pingfang(report: &mut Report, b: Option<&RecordSet>) {
  report.line("\n[Chinese fonts: PingFang, hvgl, Heiti]");

  let text = "中文字体";
  let base = system_font(17., 400, false);
  let base = as_ct(&base);
  let string = CFString::from_str(text);
  let mut rows = Vec::new();
  let mut layouts = LayoutContext::new();

  for language in ["zh-Hans", "zh-Hant", "zh-HK", "ja", "ko"] {
    let font = unsafe {
      CTFont::for_string_with_language(
        base,
        &string,
        CFRange {
          location: 0,
          length: string.length(),
        },
        Some(&CFString::from_str(language)),
      )
    };
    let info = coretext::info(&font);
    let tables: Vec<String> = coretext::tables(&font).into_iter().map(tag_name).collect();
    let hvgl = tables.iter().any(|tag| tag == "hvgl");
    let mut units: Vec<u16> = "中".encode_utf16().collect();
    let mut glyph = [0u16; 1];
    let _ = unsafe {
      font.glyphs_for_characters(
        std::ptr::NonNull::new_unchecked(units.as_mut_ptr()),
        std::ptr::NonNull::new_unchecked(glyph.as_mut_ptr()),
        1,
      )
    };
    let path = unsafe { font.path_for_glyph(glyph[0], std::ptr::null()) };
    let bounds = path.as_deref().map(|path| CGPath::bounding_box(Some(path)));
    let outline = bounds.is_some_and(|bounds| bounds.size.width > 0. && bounds.size.height > 0.);
    let ct = ct_glyphs(text, &font, Some(language));
    let (mut fonts, registered) = single(&font, "probe");
    let parley = if registered.is_ok() {
      parley_glyphs(
        &mut fonts,
        &mut layouts,
        &[FontFamilyName::Named(Cow::Borrowed("probe"))],
        text,
        17.,
        400.,
        language,
        &[],
      )
    }
    else {
      Vec::new()
    };
    let same_ids = !parley.is_empty() && ids(&parley) == ids(&ct);
    let (stripped_ids, stripped_total) = if hvgl {
      let (mut fonts, registered) = single_except(&font, "probe", &["hvgl", "hvpm"]);
      let glyphs = if registered.is_ok() {
        parley_glyphs(
          &mut fonts,
          &mut layouts,
          &[FontFamilyName::Named(Cow::Borrowed("probe"))],
          text,
          17.,
          400.,
          language,
          &[],
        )
      }
      else {
        Vec::new()
      };

      (
        Some(!glyphs.is_empty() && ids(&glyphs) == ids(&ct)),
        Some(total(&glyphs)),
      )
    }
    else {
      (None, None)
    };
    let public = info
      .postscript
      .trim_start_matches('.')
      .replace("UIText", "")
      .replace("UIDisplay", "")
      .replace("UI", "");
    let sibling = unsafe { CTFont::with_name(&CFString::from_str(&public), 17., std::ptr::null()) };
    let sibling_info = coretext::info(&sibling);
    let sibling_tables: Vec<String> = coretext::tables(&sibling)
      .into_iter()
      .map(tag_name)
      .collect();

    report.line(format!(
      "  {language}: {} / {} hvgl={hvgl} file {} outline-from-CTFontCreatePathForGlyph={outline} fontique={} parley-ids-equal-ct={same_ids} advances ct {:.2} parley {:.2}",
      info.family,
      info.postscript,
      info.path.as_deref().map(|path| format!("{path} ({})", readable(path))).unwrap_or_else(|| "none".to_owned()),
      match &registered {
        Ok(count) => format!("registered {count}"),
        Err(error) => error.clone(),
      },
      total(&ct),
      total(&parley)
    ));

    if let Some(equal) = stripped_ids {
      report.line(format!(
        "    hvgl and hvpm stripped: fontique registers it, Parley's glyph ids equal Core Text's: {equal}, advances {:.2}",
        stripped_total.unwrap_or_default()
      ));
    }

    report.line(format!(
      "    public sibling {public}: Core Text gives {} at {:?} hvgl={} outlines={}",
      sibling_info.postscript,
      sibling_info.path,
      sibling_tables.iter().any(|tag| tag == "hvgl"),
      sibling_tables
        .iter()
        .filter(|tag| ["glyf", "CFF ", "CFF2"].contains(&tag.as_str()))
        .cloned()
        .collect::<Vec<_>>()
        .join(",")
    ));
    rows.push(json!({"stripped_ids_equal": stripped_ids, "stripped_total": stripped_total, "sibling": sibling_info, "sibling_tables": sibling_tables}));
    rows.push(json!({"language": language, "font": info, "tables": tables, "hvgl": hvgl, "outline": outline, "bounds": bounds.map(|b| [b.origin.x, b.origin.y, b.size.width, b.size.height]), "fontique": registered.map_err(|e| e), "ct": ct, "parley": parley}));
  }

  let names = unsafe { objc2_core_text::CTFontManagerCopyAvailableFontFamilyNames() };
  let names: &objc2_core_foundation::CFArray<CFString> = unsafe { names.cast_unchecked() };
  let all: Vec<String> = names.iter().map(|name| name.to_string()).collect();
  let heiti: Vec<&String> = all
    .iter()
    .filter(|name| name.contains("Heiti") || name.contains("PingFang") || name.contains("Hiragino"))
    .collect();

  report.line(format!(
    "  families available: {} in all; Heiti/PingFang/Hiragino: {:?}",
    all.len(),
    heiti
  ));

  let mut stock = Vec::new();

  if let Some(b) = b {
    for id in [
      "han-zh-hans/17/max",
      "han-zh-hant/17/max",
      "han-ja/17/max",
      "han-ko/17/max",
      "zh-hans-paragraph/17/120",
    ] {
      let Some(record) = b.record(id)
      else {
        continue;
      };
      let families: Vec<String> = record
        .runs
        .iter()
        .filter_map(|run| run.family.clone())
        .collect();
      let notdef = record
        .glyph_runs
        .iter()
        .flat_map(|run| run.glyphs.iter())
        .filter(|glyph| glyph.id == 0)
        .count();

      report.line(format!(
        "  stock fontique {id}: families {:?}, .notdef glyphs {notdef}",
        families
      ));
      stock.push(json!({"variant": id, "families": families, "notdef": notdef}));
    }
  }

  report.put(
    "pingfang",
    json!({"rows": rows, "families_available": all.len(), "cjk_families": heiti, "stock": stock}),
  );
}

/// Shaping parity in the system font: Parley's glyph ids and advance totals against Core
/// Text's line in the same font, with `opsz` unset, at the size, and every axis at Core Text's
/// instance.
pub fn shaping(report: &mut Report) {
  report.line("\n[shaping parity in the system font: advance totals, Parley - Core Text]");
  report.line(
    "  case size: CT total | none: delta ids | opsz=size: delta ids | CT instance: delta ids",
  );

  let corpus = Corpus::builtin();
  let mut layouts = LayoutContext::new();
  let mut rows = Vec::new();
  let mut sums = [0f64; 3];
  let mut same = [0usize; 3];
  let mut count = 0usize;

  for id in [
    "latin-kerning",
    "latin-digits",
    "latin-punctuation",
    "sizes-latin",
    "weight-bold",
    "weight-light",
  ] {
    let Some(case) = corpus.case(id)
    else {
      continue;
    };

    for size in [11f32, 13., 17., 20., 28., 34.] {
      let font = system_font(f64::from(size), case.weight, case.italic);
      let ct_font = as_ct(&font);
      let ct = ct_glyphs(&case.text, ct_font, Some(&case.locale));
      let (mut fonts, registered) = single(ct_font, "probe");

      if registered.is_err() {
        report.line(format!("  {id} {size}: fontique refused the system font"));

        continue;
      }

      let family = [FontFamilyName::Named(Cow::Borrowed("probe"))];
      let instance: Vec<(Tag, f32)> = coretext::variation(ct_font)
        .iter()
        .map(|(tag, value)| {
          (
            Tag::new(&tag.as_bytes().try_into().unwrap_or(*b"    ")),
            *value as f32,
          )
        })
        .collect();
      let tries = [
        parley_glyphs(
          &mut fonts,
          &mut layouts,
          &family,
          &case.text,
          size,
          f32::from(case.weight),
          &case.locale,
          &[],
        ),
        parley_glyphs(
          &mut fonts,
          &mut layouts,
          &family,
          &case.text,
          size,
          f32::from(case.weight),
          &case.locale,
          &[(Tag::new(b"opsz"), size)],
        ),
        parley_glyphs(
          &mut fonts,
          &mut layouts,
          &family,
          &case.text,
          size,
          f32::from(case.weight),
          &case.locale,
          &instance,
        ),
      ];
      let ct_total = total(&ct);
      let mut cells = Vec::new();

      for (at, glyphs) in tries.iter().enumerate() {
        let delta = total(glyphs) - ct_total;
        let equal = ids(glyphs) == ids(&ct);

        sums[at] += f64::from(delta.abs());
        same[at] += usize::from(equal);
        cells.push(format!(
          "{delta:+.2} {}",
          if equal { "=" } else { "differ" }
        ));
      }

      count += 1;
      report.line(format!(
        "  {id} {size}: {ct_total:.2} | {}",
        cells.join(" | ")
      ));
      rows.push(json!({
        "case": id, "size": size, "ct_total": ct_total, "ct_instance": coretext::variation(ct_font),
        "ct_ids": ids(&ct), "none": {"total": total(&tries[0]), "ids": ids(&tries[0])},
        "opsz": {"total": total(&tries[1]), "ids": ids(&tries[1])},
        "instance": {"total": total(&tries[2]), "ids": ids(&tries[2])},
      }));
    }
  }

  report.line(format!(
    "  mean |delta| over {count}: none {:.3}, opsz {:.3}, CT instance {:.3}; ids equal: {}/{}/{}",
    sums[0] / count.max(1) as f64,
    sums[1] / count.max(1) as f64,
    sums[2] / count.max(1) as f64,
    same[0],
    same[1],
    same[2]
  ));
  report.put("shaping", json!({"rows": rows, "mean_abs_delta": sums.map(|sum| sum / count.max(1) as f64), "ids_equal": same, "count": count}));
}

/// Line metrics of the system font: Core Text's, the platform font's, the font file's own
/// tables, and the line TextKit, stock Parley and tuned Parley give one line.
pub fn metrics(report: &mut Report, tuned: &mut TunedEngine) {
  report.line("\n[line metrics: one line of 'Handgloves']");
  report.line("  size: CT asc/desc/lead | TextKit height baseline | Parley stock height baseline | Parley tuned height baseline");

  let mut stock = ParleyEngine::new(Fonts::system(), LayoutOptions::default());
  let mut rows = Vec::new();

  for size in [11f32, 13., 17., 20., 28., 34., 64.] {
    let font = system_font(f64::from(size), 400, false);
    let ct = as_ct(&font);
    let (ascent, descent, leading) = unsafe { (ct.ascent(), ct.descent(), ct.leading()) };
    let case = plain_case("metrics", "Handgloves");
    let variant = Variant {
      case: &case,
      size,
      width: None,
    };
    let tk = textkit::variant(&variant, &Setup::default(), 0);
    let tk_line = tk.record.lines.first().cloned().unwrap_or_default();
    let laid = stock.lay_out(&variant);
    let offsets = text_parity::text::Offsets::new(&case.text);
    let stock_line = laid.lines(&offsets).first().cloned().unwrap_or_default();
    let tuned_laid = tuned.lay_out(&variant);
    let tuned_line = tuned_laid
      .laid
      .lines(&offsets)
      .first()
      .cloned()
      .unwrap_or_default();

    report.line(format!(
      "  {size}: {ascent:.2}/{descent:.2}/{leading:.2} | {:.2} {:.2} | {:.2} {:.2} | {:.2} {:.2}",
      tk_line.height,
      tk_line.baseline,
      stock_line.height,
      stock_line.baseline,
      tuned_line.height,
      tuned_line.baseline
    ));
    rows.push(json!({"size": size, "ct": [ascent, descent, leading], "textkit": tk_line, "stock": stock_line, "tuned": tuned_line}));
  }

  let font = system_font(17., 400, false);

  if let Some(bytes) = coretext::sfnt(as_ct(&font))
    && let Ok(file) = FontRef::new(&bytes)
  {
    let upem = file.head().map(|head| head.units_per_em()).unwrap_or(1000);
    let hhea = file.hhea().ok();
    let os2 = file.os2().ok();
    let use_typo = os2
      .as_ref()
      .is_some_and(|os2| os2.fs_selection().bits() & (1 << 7) != 0);
    let line = format!(
      "  file tables: upem {upem}; hhea asc {} desc {} gap {}; OS/2 typo asc {} desc {} gap {}, win asc {} desc {}; USE_TYPO_METRICS {use_typo}",
      hhea.as_ref().map_or(0, |hhea| hhea.ascender().to_i16()),
      hhea.as_ref().map_or(0, |hhea| hhea.descender().to_i16()),
      hhea.as_ref().map_or(0, |hhea| hhea.line_gap().to_i16()),
      os2.as_ref().map_or(0, |os2| os2.s_typo_ascender()),
      os2.as_ref().map_or(0, |os2| os2.s_typo_descender()),
      os2.as_ref().map_or(0, |os2| os2.s_typo_line_gap()),
      os2.as_ref().map_or(0, |os2| os2.us_win_ascent()),
      os2.as_ref().map_or(0, |os2| os2.us_win_descent()),
    );

    report.line(line.clone());
    rows.push(json!({"file_tables": line}));
  }

  for (name, font) in platform::text_styles()
    .into_iter()
    .filter(|(name, _)| name == "body" || name == "headline" || name == "largeTitle")
  {
    let ct = as_ct(&font);
    let size = unsafe { ct.size() } as f32;
    let case = plain_case("metrics", "Handgloves");
    let variant = Variant {
      case: &case,
      size,
      width: None,
    };
    let setup = Setup {
      font: Some(font.clone()),
      ..Setup::default()
    };
    let tk = textkit::variant(&variant, &setup, 0);
    let line = tk.record.lines.first().cloned().unwrap_or_default();
    let usage = textkit::usage(&tk.laid);

    report.line(format!(
      "  text style {name} {size}pt: TextKit line height {:.2} baseline {:.2}, usage height {:.2}; CT asc/desc/lead {:.2}/{:.2}/{:.2}{}",
      line.height,
      line.baseline,
      usage.1,
      unsafe { ct.ascent() },
      unsafe { ct.descent() },
      unsafe { ct.leading() },
      platform::font_line_metrics(&font)
    ));
    rows.push(json!({"style": name, "size": size, "textkit": line, "usage_height": usage.1}));
  }

  report.put("metrics", json!(rows));
}

/// Korean under each TextKit strategy against Parley's normal and keep-all breaking.
pub fn korean(report: &mut Report) {
  use crate::platform::NSLineBreakStrategy;

  report.line("\n[Korean: TextKit strategies against Parley's word-break]");

  let corpus = Corpus::builtin();
  let Some(case) = corpus.case("ko-paragraph")
  else {
    return;
  };
  let mut engine = ParleyEngine::new(Fonts::system(), LayoutOptions::default());
  let mut rows = Vec::new();

  for width in [120f32, 200.] {
    let variant = Variant {
      case,
      size: 17.,
      width: Some(width),
    };
    let starts = |strategy: NSLineBreakStrategy| -> Vec<usize> {
      let setup = Setup {
        strategy: Some(strategy),
        ..Setup::default()
      };
      let tk = textkit::variant(&variant, &setup, 0);

      tk.record
        .lines
        .iter()
        .filter_map(|line| line.range.utf16.map(|range| range[0]))
        .collect()
    };
    let none = starts(NSLineBreakStrategy::empty());
    let standard = starts(NSLineBreakStrategy::Standard);
    let hangul = starts(NSLineBreakStrategy::HangulWordPriority);
    let both = starts(NSLineBreakStrategy::Standard | NSLineBreakStrategy::HangulWordPriority);
    let mut parley_starts = |keep: bool| -> Vec<usize> {
      let mut case = case.clone();

      if keep {
        case.word_break = WordBreak::KeepAll;
      }

      let variant = Variant {
        case: &case,
        size: 17.,
        width: Some(width),
      };
      let laid = engine.lay_out(&variant);
      let offsets = text_parity::text::Offsets::new(&case.text);

      laid
        .lines(&offsets)
        .iter()
        .filter_map(|line| line.range.utf16.map(|range| range[0]))
        .collect()
    };
    let normal = parley_starts(false);
    let keep = parley_starts(true);

    report.line(format!("  width {width}: TextKit none {none:?} standard {standard:?} hangulWordPriority {hangul:?} both {both:?}"));
    report.line(format!(
      "               Parley normal {normal:?} keep-all {keep:?}"
    ));
    rows.push(json!({"width": width, "none": none, "standard": standard, "hangul": hangul, "both": both, "parley_normal": normal, "parley_keep_all": keep}));
  }

  report.put("korean", json!(rows));
}

/// Core Foundation's hyphenation points for the hyphenation cases.
pub fn hyphenation(report: &mut Report) {
  report.line("\n[hyphenation points from CFStringGetHyphenationLocationBeforeIndex]");

  let corpus = Corpus::builtin();
  let mut rows = Vec::new();

  for id in ["hyphenation-en", "hyphenation-de"] {
    let Some(case) = corpus.case(id)
    else {
      continue;
    };
    let points = crate::tuned::hyphenation_points(&case.text, &case.locale);
    let (text, _) = crate::tuned::with_soft_hyphens(&case.text, &points);

    report.line(format!(
      "  {id} ({}): {} points: {}",
      case.locale,
      points.len(),
      text.replace('\u{AD}', "-")
    ));
    rows.push(json!({"case": id, "points": points, "hyphenated": text.replace('\u{AD}', "-")}));
  }

  report.put("hyphenation", json!(rows));
}

/// Which font and glyph each engine picks for keycaps and for text- and emoji-presentation
/// sequences.
pub fn emoji(report: &mut Report, tuned: &mut TunedEngine) {
  report.line("\n[emoji presentation: font and glyph per engine]");

  let strings = [
    "#\u{FE0F}\u{20E3}",
    "*\u{FE0F}\u{20E3}",
    "1\u{FE0F}\u{20E3}",
    "\u{1F51F}",
    "\u{203C}",
    "\u{203C}\u{FE0E}",
    "\u{203C}\u{FE0F}",
    "\u{2764}",
    "\u{2764}\u{FE0F}",
    "\u{263A}",
    "\u{263A}\u{FE0F}",
    "\u{A9}",
    "\u{A9}\u{FE0F}",
  ];
  let mut plain = ParleyEngine::new(
    Fonts::system(),
    LayoutOptions {
      plain_emoji: true,
      ..LayoutOptions::default()
    },
  );
  let mut first = ParleyEngine::new(Fonts::system(), LayoutOptions::default());
  let base = system_font(17., 400, false);
  let mut rows = Vec::new();
  let describe = |glyphs: &[(String, u32)]| {
    glyphs
      .iter()
      .map(|(family, id)| format!("{family}#{id}"))
      .collect::<Vec<_>>()
      .join(" ")
  };

  for string in strings {
    let case = plain_case("emoji", string);
    let variant = Variant {
      case: &case,
      size: 17.,
      width: None,
    };
    let ct: Vec<(String, u32)> = ct_glyphs(string, as_ct(&base), Some("en"))
      .into_iter()
      .map(|glyph| (glyph.family, glyph.id))
      .collect();
    let parley = |engine: &mut ParleyEngine| -> Vec<(String, u32)> {
      let laid = engine.lay_out(&variant);
      let mut out = Vec::new();

      for run in laid.glyph_runs() {
        let family = engine.fonts.identity(&run.font).family;

        out.extend(run.glyphs.iter().map(|glyph| (family.clone(), glyph.id)));
      }

      out
    };
    let plain_glyphs = parley(&mut plain);
    let first_glyphs = parley(&mut first);
    let tuned_laid = tuned.lay_out(&variant);
    let mut tuned_glyphs = Vec::new();

    for run in tuned_laid.laid.glyph_runs() {
      let family = tuned.fonts.identity(&run.font).family;

      tuned_glyphs.extend(run.glyphs.iter().map(|glyph| (family.clone(), glyph.id)));
    }

    let codes: Vec<String> = string
      .chars()
      .map(|c| format!("U+{:04X}", u32::from(c)))
      .collect();

    report.line(format!(
      "  {}: CT {} | parley plain {} | parley emoji-first {} | tuned {}",
      codes.join(" "),
      describe(&ct),
      describe(&plain_glyphs),
      describe(&first_glyphs),
      describe(&tuned_glyphs)
    ));
    rows.push(json!({"text": codes, "ct": ct, "parley_plain": plain_glyphs, "parley_emoji_first": first_glyphs, "tuned": tuned_glyphs}));
  }

  report.put("emoji", json!(rows));
}

/// The timings beside the label sets: one label's font size animated over 60 frames, and
/// min/max-content for 1000 labels, by TextKit, stock Parley and tuned Parley.
pub fn timings(report: &mut Report, tuned: &mut TunedEngine) {
  report.line("\n[timings: font-size animation and intrinsic widths]");

  let case = plain_case(
    "animate",
    "The quick brown fox jumps over the lazy dog, then naps.",
  );
  let frames: Vec<f32> = (0..60)
    .map(|frame| 17. + 17. * frame as f32 / 59.)
    .collect();
  let mut stock = ParleyEngine::new(Fonts::system(), LayoutOptions::default());
  let stats = |mut values: Vec<f64>| -> (f64, f64) {
    values.sort_by(f64::total_cmp);

    (
      values.iter().sum::<f64>() / values.len().max(1) as f64,
      values.last().copied().unwrap_or_default(),
    )
  };
  let times = |mut lay: Box<dyn FnMut(&Variant<'_>) + '_>| -> (f64, f64) {
    let values: Vec<f64> = frames
      .iter()
      .map(|size| {
        let variant = Variant {
          case: &case,
          size: *size,
          width: Some(300.),
        };
        let started = Instant::now();

        lay(&variant);

        started.elapsed().as_secs_f64() * 1e3
      })
      .collect();

    stats(values)
  };
  let textkit_frames = times(Box::new(|variant| {
    let attributed = textkit::attributed(variant, &Setup::default());
    let laid = textkit::lay_out(&attributed, 300., 0, false);

    std::hint::black_box(laid.lines.len());
  }));
  let stock_frames = times(Box::new(|variant| {
    std::hint::black_box(stock.lay_out(variant).layout.len());
  }));
  let tuned_frames = times(Box::new(|variant| {
    std::hint::black_box(tuned.lay_out(variant).laid.layout.len());
  }));

  report.line(format!(
    "  font size 17->34 over 60 frames, layout per frame mean/max ms: TextKit {:.3}/{:.3}, Parley stock {:.3}/{:.3}, Parley tuned {:.3}/{:.3}",
    textkit_frames.0, textkit_frames.1, stock_frames.0, stock_frames.1, tuned_frames.0, tuned_frames.1
  ));

  let corpus = Corpus::builtin();
  let cases: Vec<_> = corpus
    .cases
    .iter()
    .filter(|case| !case.text.is_empty())
    .collect();
  let variants: Vec<Variant<'_>> = (0..1000)
    .map(|at| Variant {
      case: cases[at % cases.len()],
      size: 13.,
      width: Some(150.),
    })
    .collect();
  let started = Instant::now();

  for variant in &variants {
    std::hint::black_box(textkit::intrinsic(variant, &Setup::default()));
  }

  let textkit_intrinsic = started.elapsed().as_secs_f64() * 1e3;
  let started = Instant::now();

  for variant in &variants {
    std::hint::black_box(stock.shape(variant).intrinsic());
  }

  let stock_intrinsic = started.elapsed().as_secs_f64() * 1e3;
  let started = Instant::now();

  for variant in &variants {
    std::hint::black_box(tuned.intrinsic(variant));
  }

  let tuned_intrinsic = started.elapsed().as_secs_f64() * 1e3;

  report.line(format!(
    "  min/max-content for 1000 fresh labels (shaping included): TextKit {textkit_intrinsic:.1} ms, Parley stock {stock_intrinsic:.1} ms, Parley tuned {tuned_intrinsic:.1} ms"
  ));
  report.put("timings", json!({
    "animation_mean_max_ms": {"textkit": textkit_frames, "stock": stock_frames, "tuned": tuned_frames},
    "intrinsic_1000_ms": {"textkit": textkit_intrinsic, "stock": stock_intrinsic, "tuned": tuned_intrinsic},
  }));
}

/// A tuned engine with everything on.
#[must_use]
pub fn tuned_engine(hvgl: crate::tuned::Hvgl) -> TunedEngine {
  TunedEngine::new(Tuning {
    instance: true,
    hyphenate: true,
    quantize: cfg!(target_os = "macos"),
    korean_keep_all: true,
    hvgl,
    line_height: None,
  })
}

/// A count of characters as CF needs it.
#[must_use]
pub fn cf_length(text: &str) -> CFIndex {
  text.encode_utf16().count() as CFIndex
}
