//! Core Text, read: a font's names, file, tables, axes and instance; a line's runs, glyphs and
//! clusters; a single-face sfnt assembled from a font's tables (Skia's `onOpenStream`); and a
//! line layout by `CTFramesetter` beside TextKit's.

use std::{collections::BTreeMap, ffi::c_void, ptr::NonNull};

use objc2_core_foundation::{
  CFArray,
  CFAttributedString,
  CFDictionary,
  CFIndex,
  CFNumber,
  CFRange,
  CFRetained,
  CFString,
  CFType,
  CFURL,
  CGPoint,
  CGSize,
};
use objc2_core_graphics::CGGlyph;
use objc2_core_text::{
  CTFont,
  CTFontTableOptions,
  CTLine,
  CTRun,
  CTRunStatus,
  kCTFontAttributeName,
  kCTFontURLAttribute,
  kCTFontVariationAxisDefaultValueKey,
  kCTFontVariationAxisIdentifierKey,
  kCTFontVariationAxisMaximumValueKey,
  kCTFontVariationAxisMinimumValueKey,
  kCTFontWeightTrait,
};
use serde::Serialize;
use text_parity::{
  record::{Cluster, Range, Run},
  text::Offsets,
};

/// A four-character tag as text.
#[must_use]
pub fn tag_name(tag: u32) -> String {
  tag
    .to_be_bytes()
    .iter()
    .map(|&byte| char::from(byte))
    .collect()
}

/// A tag from its four characters.
#[must_use]
pub fn tag_of(name: &str) -> u32 {
  let mut bytes = [b' '; 4];

  for (at, byte) in name.bytes().take(4).enumerate() {
    bytes[at] = byte;
  }

  u32::from_be_bytes(bytes)
}

/// A font as Core Text describes it.
#[derive(Clone, PartialEq, Debug, Default, Serialize)]
pub struct FontInfo {
  /// `CTFontCopyFamilyName`.
  pub family: String,
  /// `CTFontCopyPostScriptName`.
  pub postscript: String,
  /// The file, from `kCTFontURLAttribute`.
  pub path: Option<String>,
  /// The point size.
  pub size: f64,
  /// The weight trait (-1 to 1).
  pub weight_trait: f64,
  /// Whether its traits say italic.
  pub italic: bool,
  /// The instance, `CTFontCopyVariation`: tag and value.
  pub variation: Vec<(String, f64)>,
}

/// One variation axis.
#[derive(Clone, PartialEq, Debug, Default, Serialize)]
pub struct Axis {
  /// The tag.
  pub tag: String,
  /// The least value.
  pub min: f64,
  /// The default.
  pub default: f64,
  /// The greatest value.
  pub max: f64,
}

fn number(value: &CFType) -> Option<f64> {
  value.downcast_ref::<CFNumber>().and_then(CFNumber::as_f64)
}

/// The file `font` was loaded from.
#[must_use]
pub fn path(font: &CTFont) -> Option<String> {
  let url = unsafe { font.attribute(kCTFontURLAttribute) }?;
  let url = url.downcast::<CFURL>().ok()?;

  url.to_file_path().map(|path| path.display().to_string())
}

/// `font`'s instance: each axis's tag and value.
#[must_use]
pub fn variation(font: &CTFont) -> Vec<(String, f64)> {
  let Some(dictionary) = (unsafe { font.variation() })
  else {
    return Vec::new();
  };
  let dictionary: &CFDictionary<CFNumber, CFNumber> = unsafe { dictionary.cast_unchecked() };
  let (keys, values) = dictionary.to_vecs();
  let mut out: Vec<(String, f64)> = keys
    .iter()
    .zip(values.iter())
    .filter_map(|(key, value)| {
      Some((
        tag_name(u32::try_from(key.as_i64()?).ok()?),
        value.as_f64()?,
      ))
    })
    .collect();

  out.sort_by(|a, b| a.0.cmp(&b.0));

  out
}

/// `font`'s variation axes.
#[must_use]
pub fn axes(font: &CTFont) -> Vec<Axis> {
  let Some(array) = (unsafe { font.variation_axes() })
  else {
    return Vec::new();
  };
  let array: &CFArray<CFDictionary<CFString, CFType>> = unsafe { array.cast_unchecked() };

  array
    .iter()
    .map(|axis| {
      let get = |key: &CFString| axis.get(key).and_then(|value| number(&value));

      Axis {
        tag: get(unsafe { kCTFontVariationAxisIdentifierKey })
          .map(|tag| tag_name(tag as u32))
          .unwrap_or_default(),
        min: get(unsafe { kCTFontVariationAxisMinimumValueKey }).unwrap_or_default(),
        default: get(unsafe { kCTFontVariationAxisDefaultValueKey }).unwrap_or_default(),
        max: get(unsafe { kCTFontVariationAxisMaximumValueKey }).unwrap_or_default(),
      }
    })
    .collect()
}

/// The tables `font` has, as tags.
#[must_use]
pub fn tables(font: &CTFont) -> Vec<u32> {
  let Some(array) = (unsafe { font.available_tables(CTFontTableOptions::NoOptions) })
  else {
    return Vec::new();
  };

  (0..array.count())
    .map(|index| unsafe { array.value_at_index(index) } as usize as u32)
    .collect()
}

/// Everything [`FontInfo`] holds.
#[must_use]
pub fn info(font: &CTFont) -> FontInfo {
  let traits = unsafe { font.traits() };
  let traits: &CFDictionary<CFString, CFType> = unsafe { traits.cast_unchecked() };
  let weight_trait = traits
    .get(unsafe { kCTFontWeightTrait })
    .and_then(|value| number(&value))
    .unwrap_or_default();

  FontInfo {
    family: unsafe { font.family_name() }.to_string(),
    postscript: unsafe { font.post_script_name() }.to_string(),
    path: path(font),
    size: unsafe { font.size() },
    weight_trait,
    italic: unsafe { font.symbolic_traits() }
      .contains(objc2_core_text::CTFontSymbolicTraits::TraitItalic),
    variation: variation(font),
  }
}

/// A CSS weight for a font: its `wght` instance where it has one, else its weight trait mapped
/// back through the system font's weights.
#[must_use]
pub fn css_weight(info: &FontInfo) -> f32 {
  if let Some((_, value)) = info.variation.iter().find(|(tag, _)| tag == "wght") {
    return *value as f32;
  }

  let points = [
    (-0.8, 100.),
    (-0.6, 200.),
    (-0.4, 300.),
    (0., 400.),
    (0.23, 500.),
    (0.3, 600.),
    (0.4, 700.),
    (0.56, 800.),
    (0.62, 900.),
  ];

  points
    .iter()
    .min_by(|a, b| {
      (a.0 - info.weight_trait)
        .abs()
        .total_cmp(&(b.0 - info.weight_trait).abs())
    })
    .map_or(400., |point| point.1)
}

/// A single-face sfnt assembled from `font`'s tables, as Skia's `SkTypeface_Mac::onOpenStream`
/// does: what fontique can register when the file cannot be read, or holds many faces.
#[must_use]
pub fn sfnt(font: &CTFont) -> Option<Vec<u8>> {
  sfnt_except(font, &[])
}

/// [`sfnt`] without the tables named in `skip` (`hvgl` and `hvpm`, which fontique refuses).
#[must_use]
pub fn sfnt_except(font: &CTFont, skip: &[&str]) -> Option<Vec<u8>> {
  let skip: Vec<u32> = skip.iter().map(|tag| tag_of(tag)).collect();
  let mut tags = tables(font);

  tags.retain(|tag| !skip.contains(tag));
  tags.sort_unstable();
  tags.dedup();

  let mut tables = Vec::with_capacity(tags.len());

  for tag in tags {
    let data = unsafe { font.table(tag, CTFontTableOptions::NoOptions) }?;

    tables.push((tag, data.to_vec()));
  }

  if tables.is_empty() {
    return None;
  }

  let count = tables.len() as u16;
  let mut search = 1u16;
  let mut selector = 0u16;

  while search * 2 <= count {
    search *= 2;
    selector += 1;
  }

  let has_cff = tables
    .iter()
    .any(|(tag, _)| *tag == tag_of("CFF ") || *tag == tag_of("CFF2"));
  let mut out = Vec::new();

  out.extend_from_slice(
    &(if has_cff {
      0x4F54_544F_u32
    }
    else {
      0x0001_0000
    })
    .to_be_bytes(),
  );
  out.extend_from_slice(&count.to_be_bytes());
  out.extend_from_slice(&(search * 16).to_be_bytes());
  out.extend_from_slice(&selector.to_be_bytes());
  out.extend_from_slice(&(count * 16 - search * 16).to_be_bytes());

  let mut offset = 12 + 16 * tables.len();
  let mut records = Vec::new();
  let mut body = Vec::new();

  for (tag, data) in &tables {
    let mut sum = 0u32;

    for chunk in data.chunks(4) {
      let mut word = [0u8; 4];

      word[..chunk.len()].copy_from_slice(chunk);
      sum = sum.wrapping_add(u32::from_be_bytes(word));
    }

    records.extend_from_slice(&tag.to_be_bytes());
    records.extend_from_slice(&sum.to_be_bytes());
    records.extend_from_slice(&(offset as u32).to_be_bytes());
    records.extend_from_slice(&(data.len() as u32).to_be_bytes());
    body.extend_from_slice(data);

    while body.len() % 4 != 0 {
      body.push(0);
    }

    offset = 12 + 16 * tables.len() + body.len();
  }

  out.extend_from_slice(&records);
  out.extend_from_slice(&body);

  Some(out)
}

/// The byte range of the table `tag` in the single-face sfnt `bytes`.
fn table_range(bytes: &[u8], tag: &str) -> Option<std::ops::Range<usize>> {
  let count = usize::from(u16::from_be_bytes(bytes.get(4..6)?.try_into().ok()?));
  let wanted = tag_of(tag).to_be_bytes();

  (0..count).find_map(|at| {
    let record = bytes.get(12 + at * 16..12 + at * 16 + 16)?;

    if record[..4] != wanted {
      return None;
    }

    let offset = u32::from_be_bytes(record[8..12].try_into().ok()?) as usize;
    let length = u32::from_be_bytes(record[12..16].try_into().ok()?) as usize;

    (offset + length <= bytes.len()).then_some(offset..offset + length)
  })
}

/// Rewrites the line metrics of the single-face sfnt `bytes` to `metrics` (ascent, descent
/// and leading per em): `hhea`'s, and `OS/2`'s typographic and Windows ones, with
/// `USE_TYPO_METRICS` set, so every reader of either table finds the same.
pub fn with_line_metrics(bytes: &mut [u8], metrics: [f64; 3]) {
  let Some(head) = table_range(bytes, "head")
  else {
    return;
  };
  let units = f64::from(u16::from_be_bytes([
    bytes[head.start + 18],
    bytes[head.start + 19],
  ]));
  let ascent = (metrics[0] * units).round() as i16;
  let descent = (metrics[1] * units).round() as i16;
  let gap = (metrics[2] * units).round() as i16;
  let hhea = table_range(bytes, "hhea");
  let os2 = table_range(bytes, "OS/2").filter(|range| range.len() >= 78);
  let put = |bytes: &mut [u8], at: usize, value: [u8; 2]| {
    if let Some(slot) = bytes.get_mut(at..at + 2) {
      slot.copy_from_slice(&value);
    }
  };

  if let Some(hhea) = hhea {
    put(bytes, hhea.start + 4, ascent.to_be_bytes());
    put(bytes, hhea.start + 6, (-descent).to_be_bytes());
    put(bytes, hhea.start + 8, gap.to_be_bytes());
  }

  if let Some(os2) = os2 {
    let selection = u16::from_be_bytes([bytes[os2.start + 62], bytes[os2.start + 63]]) | 0x80;

    put(bytes, os2.start + 62, selection.to_be_bytes());
    put(bytes, os2.start + 68, ascent.to_be_bytes());
    put(bytes, os2.start + 70, (-descent).to_be_bytes());
    put(bytes, os2.start + 72, gap.to_be_bytes());
    put(bytes, os2.start + 74, (ascent.max(0) as u16).to_be_bytes());
    put(bytes, os2.start + 76, (descent.max(0) as u16).to_be_bytes());
  }
}

/// One glyph of a Core Text run.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize)]
pub struct CtGlyph {
  /// The glyph id.
  pub id: u16,
  /// Its x in the line.
  pub x: f64,
  /// Its advance.
  pub advance: f64,
  /// Its UTF-16 index in the string.
  pub index: usize,
}

/// A Core Text run as read.
#[derive(Clone, Debug)]
pub struct CtRun {
  /// The font.
  pub font: CFRetained<CTFont>,
  /// Its UTF-16 range in the string.
  pub range: [usize; 2],
  /// Right to left.
  pub rtl: bool,
  /// Whether its text matrix is not the identity (a synthetic slant).
  pub skewed: bool,
  /// Its glyphs, in visual order.
  pub glyphs: Vec<CtGlyph>,
}

/// The runs of `line`.
#[must_use]
pub fn runs(line: &CTLine) -> Vec<CtRun> {
  let array = unsafe { line.glyph_runs() };
  let array: &CFArray<CTRun> = unsafe { array.cast_unchecked() };
  let mut out = Vec::new();

  for run in array.iter() {
    let count = unsafe { run.glyph_count() }.max(0) as usize;
    let all = CFRange {
      location: 0,
      length: 0,
    };
    let mut ids = vec![0 as CGGlyph; count];
    let mut positions = vec![CGPoint::new(0., 0.); count];
    let mut advances = vec![CGSize::new(0., 0.); count];
    let mut indices = vec![0 as CFIndex; count];

    if count > 0 {
      unsafe {
        run.glyphs(all, NonNull::new_unchecked(ids.as_mut_ptr()));
        run.positions(all, NonNull::new_unchecked(positions.as_mut_ptr()));
        run.advances(all, NonNull::new_unchecked(advances.as_mut_ptr()));
        run.string_indices(all, NonNull::new_unchecked(indices.as_mut_ptr()));
      }
    }

    let attributes = unsafe { run.attributes() };
    let attributes: &CFDictionary<CFString, CFType> = unsafe { attributes.cast_unchecked() };
    let Some(font) = attributes
      .get(unsafe { kCTFontAttributeName })
      .and_then(|font| font.downcast::<CTFont>().ok())
    else {
      continue;
    };
    let range = unsafe { run.string_range() };
    let matrix = unsafe { run.text_matrix() };

    out.push(CtRun {
      font,
      range: [
        range.location as usize,
        (range.location + range.length) as usize,
      ],
      rtl: unsafe { run.status() }.contains(CTRunStatus::RightToLeft),
      skewed: matrix.c.abs() > 1e-6 || matrix.b.abs() > 1e-6,
      glyphs: (0..count)
        .map(|at| CtGlyph {
          id: ids[at],
          x: positions[at].x,
          advance: advances[at].width,
          index: indices[at].max(0) as usize,
        })
        .collect(),
    });
  }

  out
}

/// A line of an attributed string laid out by Core Text alone.
pub fn line_of(string: &CFAttributedString) -> CFRetained<CTLine> {
  unsafe { CTLine::with_attributed_string(string) }
}

/// The family and PostScript names in the font file's own name table, and the face's index:
/// the names Parley's records carry (Core Text names the system font `.AppleSystemUIFont`, its
/// file `System Font`). The face is found by PostScript name, or is the file's only one.
#[must_use]
pub fn file_names(info: &FontInfo) -> Option<(String, String, u32)> {
  type Cache = std::sync::Mutex<std::collections::HashMap<String, Option<(String, String, u32)>>>;

  static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();

  let path = info.path.as_ref()?;
  let key = format!("{path}#{}", info.postscript);
  let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));

  if let Some(found) = cache.lock().ok()?.get(&key) {
    return found.clone();
  }

  let found = crate::sets::read_file(path).and_then(|data| {
    let count = match skrifa::raw::FileRef::new(&data) {
      Ok(skrifa::raw::FileRef::Collection(collection)) => collection.len(),
      _ => 1,
    };
    let faces: Vec<(u32, String, String)> = (0..count)
      .filter_map(|index| {
        text_parity::fonts::names_of(&data, index)
          .map(|(family, postscript)| (index, family, postscript))
      })
      .collect();

    faces
      .iter()
      .find(|face| face.2 == info.postscript)
      .or_else(|| (faces.len() == 1).then(|| &faces[0]))
      .map(|face| (face.1.clone(), face.2.clone(), face.0))
  });

  cache.lock().ok()?.insert(key, found.clone());

  found
}

/// The record's runs and clusters of one line: `runs` from a `CTLine` of the line's text, whose
/// UTF-16 indices start at `base` in the whole text and whose x starts at `x0`.
#[must_use]
pub fn record_parts(
  runs: &[CtRun],
  base: usize,
  line: usize,
  x0: f64,
  offsets: &Offsets,
) -> (Vec<Run>, Vec<Cluster>) {
  let mut out_runs = Vec::new();
  let mut clusters = Vec::new();

  for run in runs {
    let info = info(&run.font);
    let range = [base + run.range[0], base + run.range[1]];
    let file = file_names(&info);

    out_runs.push(Run {
      range: Range {
        utf8: Some(offsets.range_utf8(range)),
        utf16: Some(range),
      },
      line,
      family: Some(
        file
          .as_ref()
          .map_or_else(|| info.family.clone(), |file| file.0.clone()),
      ),
      postscript: Some(
        file
          .as_ref()
          .map_or_else(|| info.postscript.clone(), |file| file.1.clone()),
      ),
      path: info.path.clone(),
      index: file.as_ref().map(|file| file.2),
      size: info.size as f32,
      weight: Some(css_weight(&info)),
      italic: Some(info.italic),
      synthetic_bold: false,
      synthetic_italic: run.skewed,
      rtl: run.rtl,
    });

    let mut by_index: BTreeMap<usize, (f64, f64)> = BTreeMap::new();

    for glyph in &run.glyphs {
      let entry = by_index.entry(glyph.index).or_insert((f64::MAX, 0.));

      entry.0 = entry.0.min(glyph.x);
      entry.1 += glyph.advance;
    }

    let starts: Vec<usize> = by_index.keys().copied().collect();

    for (at, start) in starts.iter().enumerate() {
      let end = starts.get(at + 1).copied().unwrap_or(run.range[1]);
      let (x, advance) = by_index[start];
      let range = [base + start, base + end];

      clusters.push(Cluster {
        range: Range {
          utf8: Some(offsets.range_utf8(range)),
          utf16: Some(range),
        },
        line,
        x: (x0 + x) as f32,
        advance: advance as f32,
      });
    }
  }

  (out_runs, clusters)
}

/// A line's ascent, descent and leading as Core Text measures it.
#[must_use]
pub fn line_metrics(line: &CTLine) -> (f64, f64, f64, f64) {
  let (mut ascent, mut descent, mut leading) = (0., 0., 0.);
  let width = unsafe { line.typographic_bounds(&mut ascent, &mut descent, &mut leading) };

  (width, ascent, descent, leading)
}

/// A raw pointer for a Core Foundation dictionary key.
#[must_use]
pub fn key(string: &CFString) -> *const c_void {
  std::ptr::from_ref(string).cast()
}
