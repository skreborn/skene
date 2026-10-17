//! B tuned for Apple: Parley as Skene would drive it there, the recoverable gaps closed.
//!
//! - **Fonts through Core Text.** No fontique system scan: the system UI font is the one
//!   `NSFont`/`UIFont` hands back, and every character it cannot draw falls back to the font
//!   `CTFontCreateForStringWithLanguage` picks from it in the text's language. Each font is
//!   registered with fontique as a single-face sfnt built from `CTFontCopyTable` (Skia's
//!   `onOpenStream`), so hidden system fonts and faces inside collections work alike.
//! - **Optical size at the size**: `opsz` is set to the point size, Core Text's automatic optical
//!   sizing (CSS `font-optical-sizing: auto`).
//! - **Hyphenation from Core Foundation**: where a case asks for automatic hyphenation,
//!   `CFStringGetHyphenationLocationBeforeIndex` gives the points, inserted as soft hyphens and
//!   mapped back to the text's offsets in the record.
//! - **Line metrics from the paragraph's font**: a fallback font is registered with the system
//!   font's ascent, descent and leading per em, since TextKit sizes a line by the fonts its
//!   attributes name and never by a substituted one.
//! - **Hangul keeps its words whole** in every language, TextKit's `standard` strategy.
//! - **No break inside a grapheme cluster**, through Parley's line-break override.
//! - **Tracking** of a fallback font with a `trak` table and no optical size axis (Apple Color
//!   Emoji), as letter spacing on its clusters, the glyph drawn centred in the extra advance as
//!   Core Text does. The amount is TextKit's own, read once per font and size from TextKit's line
//!   of one of its clusters against the font's advance.
//! - **A fallback font at its own instance**: its clusters take the variation Core Text gave the
//!   fallback font (PingFang UI at `wght` 400), every other axis at its default, rather than the
//!   system font's; where Core Text reports no optical size, the one whose advances match TextKit's
//!   line of a sample cluster.
//! - **Natural alignment is the app's**: `start` on a right-to-left paragraph is the left edge in a
//!   left-to-right app, as `NSTextAlignment.natural` is.
//!
//! The breaking, the soft hyphen's hyphen and the ellipsis are the harness's
//! (`text_parity::layout`), copied here because its shaping step has no hook for variations or
//! a family list.
#![allow(unused_unsafe)]

use std::{borrow::Cow, collections::HashMap, sync::Arc, time::Instant};

use objc2_core_foundation::{CFIndex, CFLocale, CFRange, CFRetained, CFString};
use objc2_core_text::{CTFont, CTFontTableOptions};
use parley::{
  Alignment as ParleyAlignment,
  AlignmentOptions,
  BaseDirection,
  BreakReason,
  FontData,
  FontFamily,
  FontFamilyName,
  FontFeature,
  FontFeatures,
  FontStyle,
  FontVariation,
  FontVariations,
  FontWeight,
  GenericFamily,
  Layout,
  LayoutContext,
  LineBreak as ParleyLineBreak,
  LineBreakContext,
  LineHeight,
  OverflowWrap,
  PositionedLayoutItem,
  StyleProperty,
  TextWrapMode,
  WordBreak as ParleyWordBreak,
  fontique::{Blob, FontInfoOverride, Language, Synthesis},
  setting::Tag,
};
use skrifa::{
  FontRef,
  MetadataProvider as _,
  instance::{LocationRef, NormalizedCoord, Size},
  raw::types::F2Dot14,
};
use text_parity::{
  corpus::{Alignment, Case, Direction, Hyphenation, LineBreak, Variant, WordBreak},
  fonts::{FontSource, Fonts},
  layout::{Laid, Mark},
  record::{Intrinsic, Range, Record},
  text::{Offsets, is_invisible},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{coretext, platform};

/// What the tuning turns on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tuning {
  /// Every axis of the system font at Core Text's instance for the size and weight
  /// (`CTFontCopyVariation`: `opsz` at the size, Apple's `wght` for each weight).
  pub instance: bool,
  /// Core Foundation's hyphenation points as soft hyphens where a case asks for `auto`.
  pub hyphenate: bool,
  /// Line metrics rounded to whole points (Parley's `quantize`), as TextKit 2 does on macOS.
  pub quantize: bool,
  /// Hangul keeps its words whole under the normal word break, in every language, as
  /// TextKit's `standard` strategy does.
  pub korean_keep_all: bool,
  /// A line height in points for every line (a text style's line height and leading).
  pub line_height: Option<f32>,
  /// A fallback font takes the system font's line metrics per em.
  pub primary_metrics: bool,
  /// No break inside an extended grapheme cluster (regional indicator pairs aside).
  pub whole_clusters: bool,
  /// OpenType features on for the whole text (`chws`, `halt`), by tag.
  pub features: &'static [&'static str],
  /// A fallback font's `trak` table applies, as letter spacing on its clusters.
  pub trak: bool,
  /// `start` is the app's left edge (a left-to-right app) on every paragraph.
  pub natural_from_app: bool,
  /// A fallback font's clusters take the fallback font's own instance.
  pub fallback_instance: bool,
}

/// What a fallback font's clusters take: its instance and its tracking, as Core Text lays the
/// font out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FallbackStyle {
  /// Every axis's value, where the font varies.
  pub instance: Option<Vec<(String, f64)>>,
  /// The tracking per cluster in points, where the font has a `trak` in force.
  pub tracking: Option<f32>,
}

/// The advances of `ids` in the single-face sfnt `bytes` at `size`, at the axis values
/// `settings`, summed.
fn advances(bytes: &[u8], ids: &[u16], size: f32, settings: &[(String, f64)]) -> Option<f64> {
  let font = FontRef::from_index(bytes, 0).ok()?;
  let location = font
    .axes()
    .location(settings.iter().filter_map(|(tag, value)| {
      Some((
        skrifa::Tag::new(tag.as_bytes().try_into().ok()?),
        *value as f32,
      ))
    }));
  let metrics = font.glyph_metrics(Size::new(size), &location);

  Some(
    ids
      .iter()
      .map(|id| {
        f64::from(
          metrics
            .advance_width(skrifa::GlyphId::new(u32::from(*id)))
            .unwrap_or(0.),
        )
      })
      .sum(),
  )
}

/// A font's normal track from its `trak` table: the tracking per size, in font units.
#[derive(Clone, Debug, PartialEq)]
pub struct Trak {
  sizes: Vec<f32>,
  values: Vec<f32>,
  units: f32,
}

impl Trak {
  /// The normal track of `font`, where it has one and no `opsz` axis (an optical size axis
  /// carries its tracking in its outlines).
  #[must_use]
  pub fn of(font: &CTFont) -> Option<Self> {
    if coretext::axes(font).iter().any(|axis| axis.tag == "opsz") {
      return None;
    }

    let data =
      unsafe { font.table(coretext::tag_of("trak"), CTFontTableOptions::NoOptions) }?.to_vec();
    let head =
      unsafe { font.table(coretext::tag_of("head"), CTFontTableOptions::NoOptions) }?.to_vec();
    let units = f32::from(u16::from_be_bytes(head.get(18..20)?.try_into().ok()?));
    let u16_at = |at: usize| Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?));
    let i16_at = |at: usize| Some(i16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?));
    let fixed_at =
      |at: usize| Some(i32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?) as f32 / 65536.);
    let horizontal = usize::from(u16_at(6)?);

    if horizontal == 0 {
      return None;
    }

    let tracks = usize::from(u16_at(horizontal)?);
    let count = usize::from(u16_at(horizontal + 2)?);
    let size_table = u32_at(horizontal + 4)? as usize;
    let sizes: Vec<f32> = (0..count)
      .map(|at| fixed_at(size_table + 4 * at))
      .collect::<Option<_>>()?;

    for track in 0..tracks {
      let entry = horizontal + 8 + 8 * track;

      if fixed_at(entry)? != 0. {
        continue;
      }

      let offset = usize::from(u16_at(entry + 6)?);
      let values: Vec<f32> = (0..count)
        .map(|at| i16_at(offset + 2 * at).map(f32::from))
        .collect::<Option<_>>()?;

      return Some(Self {
        sizes,
        values,
        units,
      });
    }

    None
  }

  /// The tracking at `size` points, in points: interpolated, and extrapolated, between the
  /// two nearest sizes in the table, as HarfBuzz does.
  #[must_use]
  pub fn at(&self, size: f32) -> f32 {
    let count = self.sizes.len();
    let value = match count {
      0 => 0.,
      1 => self.values[0],
      _ => {
        let mut index = 0;

        while index < count - 1 && self.sizes[index] < size {
          index += 1;
        }

        let low = index.saturating_sub(1);
        let (from, to) = (self.sizes[low], self.sizes[low + 1]);
        let t = if (to - from).abs() < f32::EPSILON {
          0.
        }
        else {
          (size - from) / (to - from)
        };

        (t * self.values[low + 1] + (1. - t) * self.values[low]).round()
      }
    };

    value * size / self.units
  }
}

/// A registered Core Text font.
#[derive(Clone)]
struct Registered {
  family: String,
}

/// How a font's bytes reach fontique.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Storage {
  /// Every table but `hvgl` and `hvpm` copied from `CTFontCopyTable` into one sfnt.
  #[default]
  Copy,
  /// Only the tables layout reads ([`coretext::LAYOUT_TABLES`]) copied: enough to shape and
  /// measure, nothing to draw (C draws through Core Text).
  Layout,
  /// The system font mapped from its file; a fallback (its line metrics rewritten, `hvgl`
  /// stripped) as [`Storage::Layout`].
  Map,
  /// Every font with a file mapped from it as it is (a collection's face selected in a private
  /// copy of its header), its line metrics its own; a font fontique refuses (`hvgl`) or without
  /// a file as [`Storage::Layout`].
  MapAll,
}

impl Storage {
  /// The storage named `name` (`copy`, `layout`, `map`, `map-all`).
  #[must_use]
  pub fn parse(name: &str) -> Option<Self> {
    match name {
      "copy" => Some(Self::Copy),
      "layout" => Some(Self::Layout),
      "map" => Some(Self::Map),
      "map-all" => Some(Self::MapAll),
      _ => None,
    }
  }

  /// The storage's name.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::Copy => "copy",
      Self::Layout => "layout",
      Self::Map => "map",
      Self::MapAll => "map-all",
    }
  }
}

/// How one font was registered.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Stored {
  /// The family it is registered under.
  pub family: String,
  /// Its PostScript name.
  pub postscript: String,
  /// Its file, where Core Text reports one.
  pub path: Option<String>,
  /// `mapped` or `copied`.
  pub how: &'static str,
  /// Bytes copied into memory.
  pub copied: usize,
  /// Bytes mapped from the file.
  pub mapped: usize,
  /// The face of a collection selected, and how many faces the file holds.
  pub face: Option<(u32, u32)>,
  /// What opening and mapping the file answered, where it was tried.
  pub access: Option<crate::mapped::Access>,
  /// Whether its line metrics were rewritten to the paragraph font's.
  pub metrics_rewritten: bool,
  /// Whether it carries `hvgl` outlines.
  pub hvgl: bool,
  /// Milliseconds its registration took.
  pub ms: f64,
}

/// Parley over Core Text's fonts.
pub struct TunedEngine {
  // The collection, holding only what Core Text resolved.
  pub fonts: Fonts,
  layouts: LayoutContext<()>,
  // What is on.
  pub tuning: Tuning,
  registered: HashMap<String, Registered>,
  refused: std::collections::HashSet<String>,
  instances: HashMap<(u32, u16, bool), Vec<(String, f64)>>,
  fallback_cache: HashMap<(String, String, String), Option<String>>,
  // The Core Text font each registered blob came from, by blob id.
  pub sources: HashMap<u64, CFRetained<CTFont>>,
  // A cluster and language each fallback family was found for, to ask Core Text again at
  // another size.
  samples: HashMap<String, (String, String)>,
  // Each fallback family's registered font data (face 0), for its advances.
  bytes_of: HashMap<String, Arc<dyn AsRef<[u8]> + Send + Sync>>,
  // Each fallback family's blob id.
  blob_of: HashMap<String, u64>,
  // Each fallback family's style by size, weight and slant.
  fallback_styles_cache: HashMap<(String, u32, u16, bool), FallbackStyle>,
  // The tracking in points of each tracked blob at each size (`f32` bits).
  tracking_by_blob: HashMap<(u64, u32), f32>,
  // How each fallback family's optical size was settled.
  pub optical_notes: Vec<String>,
  // Time spent building sfnts from tables and registering them.
  pub register_ms: f64,
  // Bytes of sfnt registered (copied).
  pub registered_bytes: usize,
  // Bytes of font files registered by mapping them.
  pub mapped_bytes: usize,
  // How fonts reach fontique.
  pub storage: Storage,
  // Each font registered, as it was stored.
  pub stored: Vec<Stored>,
  // Fonts fontique refused (an `hvgl` font, say).
  pub rejected: Vec<String>,
  // The families registered, in order: family, PostScript name, file.
  pub log: Vec<(String, String, Option<String>)>,
  // What registration changed: `hvgl` stripped or substituted.
  pub notes: Vec<String>,
}

/// The family name the system UI font is registered under.
pub const SYSTEM: &str = "ct system";

fn retain(font: &CTFont) -> CFRetained<CTFont> {
  unsafe { CFRetained::retain(std::ptr::NonNull::from(font)) }
}

impl TunedEngine {
  /// An engine with an empty collection.
  ///
  /// # Panics
  ///
  /// Never: an empty source cannot fail.
  #[must_use]
  pub fn new(tuning: Tuning) -> Self {
    let fonts = Fonts::new(&FontSource {
      system: false,
      files: Vec::new(),
      generics: Vec::new(),
      fallbacks: Vec::new(),
    })
    .expect("an empty collection");
    let engine = Self {
      fonts,
      layouts: LayoutContext::new(),
      tuning,
      registered: HashMap::new(),
      refused: std::collections::HashSet::new(),
      instances: HashMap::new(),
      fallback_cache: HashMap::new(),
      sources: HashMap::new(),
      samples: HashMap::new(),
      bytes_of: HashMap::new(),
      blob_of: HashMap::new(),
      fallback_styles_cache: HashMap::new(),
      tracking_by_blob: HashMap::new(),
      optical_notes: Vec::new(),
      register_ms: 0.,
      registered_bytes: 0,
      mapped_bytes: 0,
      storage: Storage::Copy,
      stored: Vec::new(),
      rejected: Vec::new(),
      log: Vec::new(),
      notes: Vec::new(),
    };
    engine
  }

  /// Registers `font` (once per file, face and metrics) under `family` (its own PostScript
  /// name where `None`): the family name Parley asks for. With `metrics` (ascent, descent and
  /// leading per em), the font's line metrics are replaced by them.
  pub fn register(
    &mut self,
    font: &CTFont,
    family: Option<&str>,
    italic: Option<bool>,
    metrics: Option<[f64; 3]>,
  ) -> Option<String> {
    let info = coretext::info(font);
    let key = format!(
      "{}#{}",
      info.path.clone().unwrap_or_default(),
      info.postscript
    );
    let key = if info.path.is_some() && family == Some(SYSTEM) {
      format!(
        "{}#{}",
        info.path.clone().unwrap_or_default(),
        italic.unwrap_or(false)
      )
    }
    else {
      key
    };
    let tag =
      metrics.map(|metrics| format!("{:.4}/{:.4}/{:.4}", metrics[0], metrics[1], metrics[2]));
    let key = match &tag {
      Some(tag) => format!("{key}@{tag}"),
      None => key,
    };

    if let Some(found) = self.registered.get(&key) {
      return Some(found.family.clone());
    }

    if self.refused.contains(&key) {
      return None;
    }

    let started = Instant::now();
    let family = family.map_or_else(
      || match &tag {
        Some(tag) => format!("ct {} {tag}", info.postscript),
        None => format!("ct {}", info.postscript),
      },
      str::to_owned,
    );
    let has_hvgl = coretext::tables(font).contains(&coretext::tag_of("hvgl"));
    let mut skip = Vec::new();

    if has_hvgl {
      self.notes.push(format!(
        "{}: hvgl and hvpm stripped; shaped from its other tables, drawn from Core Text's outlines",
        info.postscript
      ));
      skip.extend(["hvgl", "hvpm"]);
    }

    let mappable = match self.storage {
      Storage::Copy | Storage::Layout => false,
      Storage::Map => metrics.is_none() && !has_hvgl,
      Storage::MapAll => !has_hvgl,
    };
    let mut access = None;
    let mut face = None;
    let mut mapped: Option<crate::mapped::Mapped> = None;

    if mappable && let Some(path) = &info.path {
      match crate::mapped::Mapped::open(path, None) {
        Ok((whole, opened)) => {
          access = Some(opened);
          face = crate::mapped::face_of(whole.as_ref(), &info.postscript);
          mapped = match face {
            Some((index, count)) if count > 1 => {
              drop(whole);

              crate::mapped::Mapped::open(path, Some(index))
                .ok()
                .map(|(selected, _)| selected)
            }
            Some(_) => Some(whole),
            None => None,
          };
        }
        Err(failed) => access = Some(failed),
      }
    }

    let (bytes, copied, metrics_rewritten): (Arc<dyn AsRef<[u8]> + Send + Sync>, bool, bool) =
      if let Some(mapped) = mapped {
        self.mapped_bytes += mapped.len();

        (Arc::new(mapped), false, false)
      }
      else {
        if metrics.is_some() {
          skip.push("MVAR");
        }

        let layout: Vec<&str> = coretext::LAYOUT_TABLES
          .iter()
          .copied()
          .filter(|tag| !skip.contains(tag))
          .collect();
        let bytes = if self.storage == Storage::Copy {
          coretext::sfnt_except(font, &skip)
        }
        else {
          coretext::sfnt_only(font, &layout)
        };
        let bytes = bytes.map(|mut bytes| {
          if let Some(metrics) = metrics {
            coretext::with_line_metrics(&mut bytes, metrics);
          }

          bytes
        });
        let Some(bytes) = bytes
        else {
          self.refused.insert(key.clone());
          self
            .rejected
            .push(format!("{}: no tables", info.postscript));

          return None;
        };

        self.registered_bytes += bytes.len();

        (Arc::new(bytes), true, metrics.is_some())
      };
    let size = (*bytes).as_ref().len();
    let blob = Blob::new(bytes.clone());
    let id = blob.id();
    let added = self.fonts.context.collection.register_fonts(
      blob,
      Some(FontInfoOverride {
        family_name: Some(&family),
        style: italic.map(|italic| {
          if italic {
            parley::fontique::FontStyle::Italic
          }
          else {
            parley::fontique::FontStyle::Normal
          }
        }),
        ..FontInfoOverride::default()
      }),
    );

    self.register_ms += started.elapsed().as_secs_f64() * 1e3;

    if added.is_empty() {
      let tables: Vec<String> = coretext::tables(font)
        .into_iter()
        .map(coretext::tag_name)
        .collect();

      self.rejected.push(format!(
        "{} ({}): fontique registered no face; tables {}",
        info.postscript,
        info.family,
        tables.join(" ")
      ));
      self.refused.insert(key);

      return None;
    }

    self.sources.insert(id, retain(font));
    self.bytes_of.insert(family.clone(), bytes);
    self.blob_of.insert(family.clone(), id);
    self.stored.push(Stored {
      family: family.clone(),
      postscript: info.postscript.clone(),
      path: info.path.clone(),
      how: if copied { "copied" } else { "mapped" },
      copied: if copied { size } else { 0 },
      mapped: if copied { 0 } else { size },
      face,
      access,
      metrics_rewritten,
      hvgl: has_hvgl,
      ms: started.elapsed().as_secs_f64() * 1e3,
    });

    self.log.push((family.clone(), info.postscript, info.path));
    self.registered.insert(
      key,
      Registered {
        family: family.clone(),
      },
    );

    Some(family)
  }

  /// The families a variant asks for: the system font, then each font Core Text falls back
  /// to for a cluster it cannot draw, in order of first use.
  pub fn families(&mut self, case: &Case, size: f32) -> Vec<String> {
    let base = platform::system_font(f64::from(size), case.weight, case.italic);
    let base = platform::as_ct(&base);
    let mut families = Vec::new();

    if let Some(system) = self.register(base, Some(SYSTEM), Some(case.italic), None) {
      families.push(system);
    }

    let metrics = self.tuning.primary_metrics.then(|| {
      let size = unsafe { base.size() };

      unsafe {
        [
          base.ascent() / size,
          base.descent() / size,
          base.leading() / size,
        ]
      }
    });

    let language = CFString::from_str(&case.locale);
    let base_name = unsafe { base.post_script_name() }.to_string();

    for cluster in case.text.graphemes(true) {
      if cluster.chars().all(is_invisible) {
        continue;
      }

      let mut units: Vec<u16> = cluster.encode_utf16().collect();
      let mut glyphs = vec![0u16; units.len()];
      let covered = unsafe {
        base.glyphs_for_characters(
          std::ptr::NonNull::new_unchecked(units.as_mut_ptr()),
          std::ptr::NonNull::new_unchecked(glyphs.as_mut_ptr()),
          units.len() as CFIndex,
        )
      };

      if covered && crate::tuned::not_emoji(cluster) {
        continue;
      }

      let key = (base_name.clone(), case.locale.clone(), cluster.to_owned());

      let family = if let Some(found) = self.fallback_cache.get(&key) {
        found.clone()
      }
      else {
        let string = CFString::from_str(cluster);
        let range = CFRange {
          location: 0,
          length: string.length(),
        };
        let font =
          unsafe { CTFont::for_string_with_language(base, &string, range, Some(&language)) };
        let found = self.register(&font, None, None, metrics);

        if let Some(family) = &found {
          self
            .samples
            .entry(family.clone())
            .or_insert_with(|| (cluster.to_owned(), case.locale.clone()));
        }

        if let Some(family) = &found
          && coretext::info(&font).postscript.contains("AppleColorEmoji")
          && let Some(id) = self.fonts.context.collection.family_id(family)
        {
          self
            .fonts
            .context
            .collection
            .set_generic_families(GenericFamily::Emoji, [id].into_iter());
        }

        self.fallback_cache.insert(key, found.clone());

        found
      };

      if let Some(family) = family
        && !families.contains(&family)
      {
        families.push(family);
      }
    }

    families
  }
}

/// Whether a cluster is not drawn in emoji presentation (emoji go to the emoji family first).
#[must_use]
pub fn not_emoji(cluster: &str) -> bool {
  !text_parity::emoji::is_emoji_presentation(cluster)
}

/// Whether `character` is Hangul: jamo, compatibility jamo or a syllable.
const fn is_hangul(character: char) -> bool {
  matches!(
    character,
    '\u{1100}'..='\u{11FF}'
      | '\u{3130}'..='\u{318F}'
      | '\u{A960}'..='\u{A97F}'
      | '\u{AC00}'..='\u{D7AF}'
      | '\u{D7B0}'..='\u{D7FF}'
  )
}

/// The byte ranges of `text`'s Hangul runs.
#[must_use]
pub fn hangul_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
  let mut out: Vec<std::ops::Range<usize>> = Vec::new();

  for (at, character) in text.char_indices() {
    if !is_hangul(character) {
      continue;
    }

    let end = at + character.len_utf8();

    match out.last_mut() {
      Some(last) if last.end == at => last.end = end,
      _ => out.push(at..end),
    }
  }

  out
}

/// Suppresses a break between two code points of one extended grapheme cluster, as Core Text
/// never breaks inside one; a pair of regional indicators is left to the default.
fn whole_clusters(context: LineBreakContext) -> Option<bool> {
  let regional = |character: char| ('\u{1F1E6}'..='\u{1F1FF}').contains(&character);

  if regional(context.before) && regional(context.after) {
    return None;
  }

  let mut buffer = [0u8; 8];
  let first = context.before.encode_utf8(&mut buffer).len();
  let second = context.after.encode_utf8(&mut buffer[first..]).len();
  let pair = std::str::from_utf8(&buffer[..first + second]).ok()?;

  (pair.graphemes(true).count() == 1).then_some(false)
}

/// The UTF-16 hyphenation points of `text` in `locale`, from Core Foundation.
#[must_use]
pub fn hyphenation_points(text: &str, locale: &str) -> Vec<usize> {
  let string = CFString::from_str(text);
  let identifier = CFString::from_str(locale);
  let Some(locale) = CFLocale::new(None, Some(&identifier))
  else {
    return Vec::new();
  };

  if !CFString::is_hyphenation_available_for_locale(Some(&locale)) {
    return Vec::new();
  }

  let units: Vec<u16> = text.encode_utf16().collect();
  let mut points = Vec::new();
  let mut start = 0usize;

  while start < units.len() {
    while start < units.len()
      && !char::from_u32(u32::from(units[start])).is_some_and(char::is_alphabetic)
    {
      start += 1;
    }

    let mut end = start;

    while end < units.len()
      && char::from_u32(u32::from(units[end])).is_some_and(char::is_alphabetic)
    {
      end += 1;
    }

    let mut at = end;
    let limit = CFRange {
      location: start as CFIndex,
      length: (end - start) as CFIndex,
    };

    while at > start {
      let found = unsafe {
        string.hyphenation_location_before_index(
          at as CFIndex,
          limit,
          0,
          Some(&locale),
          std::ptr::null_mut(),
        )
      };

      if found < 0 || found as usize <= start || found as usize >= at {
        break;
      }

      points.push(found as usize);
      at = found as usize;
    }

    start = end.max(start + 1);
  }

  points.sort_unstable();
  points.dedup();
  points
}

/// `text` with a soft hyphen at each UTF-16 point, and the map from each byte of the result
/// back to the byte of `text` it came from.
#[must_use]
pub fn with_soft_hyphens(text: &str, points: &[usize]) -> (String, Vec<usize>) {
  let offsets = Offsets::new(text);
  let mut bytes: Vec<usize> = points.iter().map(|point| offsets.utf8(*point)).collect();

  bytes.sort_unstable();

  let mut out = String::with_capacity(text.len() + points.len() * 2);
  let mut back = Vec::with_capacity(text.len() + points.len() * 2 + 1);
  let mut next = bytes.iter().peekable();

  for (at, character) in text.char_indices() {
    while next.peek().is_some_and(|point| **point == at) {
      next.next();
      out.push('\u{AD}');
      back.extend([at, at]);
    }

    out.push(character);
    back.extend(std::iter::repeat_n(at, character.len_utf8()));
  }

  back.push(text.len());

  (out, back)
}

/// Moves every range of `record` from the hyphenated text back to `original`.
pub fn map_back(record: &mut Record, original: &str, back: &[usize]) {
  let offsets = Offsets::new(original);
  let map = |range: &mut Range| {
    if let Some([start, end]) = range.utf8 {
      let start = back.get(start).copied().unwrap_or(original.len());
      let end = back.get(end).copied().unwrap_or(original.len());

      range.utf8 = Some([start, end]);
      range.utf16 = Some(offsets.range_utf16([start, end]));
    }
  };

  for line in &mut record.lines {
    map(&mut line.range);
  }

  for run in &mut record.runs {
    map(&mut run.range);
  }

  record.clusters.retain(|cluster| {
    cluster
      .range
      .utf8
      .is_none_or(|[start, end]| back.get(start).copied() != back.get(end).copied() || start == end)
  });

  for cluster in &mut record.clusters {
    map(&mut cluster.range);
  }

  for run in &mut record.glyph_runs {
    for glyph in &mut run.glyphs {
      glyph.cluster = back.get(glyph.cluster).copied().unwrap_or(original.len());
    }
  }
}

/// A variant shaped and not yet broken.
pub struct Shaped {
  layout: Layout<()>,
  text: String,
  max_lines: Option<usize>,
  ellipsis: bool,
  alignment: Alignment,
  notes: Vec<String>,
}

fn glyph_of(font: &FontData, coords: &[i16], size: f32, character: char) -> Option<(u32, f32)> {
  let font = FontRef::from_index(font.data.data(), font.index).ok()?;
  let coords: Vec<NormalizedCoord> = coords
    .iter()
    .map(|bits| F2Dot14::from_bits(*bits))
    .collect();
  let glyph = font.charmap().map(character)?;
  let advance = font
    .glyph_metrics(Size::new(size), LocationRef::new(&coords))
    .advance_width(glyph)?;

  Some((glyph.to_u32(), advance))
}

fn trailing_blank(text: &str, line: &parley::Line<'_, ()>) -> f32 {
  let mut clusters: Vec<(std::ops::Range<usize>, f32)> = line
    .runs()
    .flat_map(|run| {
      run
        .clusters()
        .map(|cluster| (cluster.text_range(), cluster.advance()))
        .collect::<Vec<_>>()
    })
    .collect();
  let mut blank = 0.;

  clusters.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));

  for (range, advance) in clusters {
    let end = range.end.min(text.len());

    if !text[range.start.min(end)..end].chars().all(is_invisible) {
      break;
    }

    blank += advance;
  }

  blank
}

/// A tuned layout of a variant, with what its record needs.
pub struct TunedLaid {
  /// The layout, in the harness's type.
  pub laid: Laid,
  /// The case it was laid out as (its text hyphenated where tuned).
  pub case: Case,
  /// The map back to the original text's bytes, where the text was hyphenated.
  pub back: Option<Vec<usize>>,
  /// The families asked for.
  pub families: Vec<String>,
}

impl TunedEngine {
  fn shape_with(
    &mut self,
    case: &Case,
    size: f32,
    families: &[String],
    anywhere_from: Option<usize>,
  ) -> Shaped {
    let mut notes = Vec::new();
    let names: Vec<FontFamilyName<'static>> = families
      .iter()
      .map(|family| FontFamilyName::Named(Cow::Owned(family.clone())))
      .chain([FontFamilyName::Generic(GenericFamily::SystemUi)])
      .collect();
    let instance = if self.tuning.instance {
      self.instance(case, size)
    }
    else {
      Vec::new()
    };
    let fallback = self.fallback_styles(case, size);
    let mut builder = self.layouts.ranged_builder(
      &mut self.fonts.context,
      &case.text,
      1.,
      self.tuning.quantize,
    );

    builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Owned(
      names.clone(),
    ))));
    builder.push_default(StyleProperty::FontSize(size));
    builder.push_default(StyleProperty::FontWeight(FontWeight::new(f32::from(
      case.weight,
    ))));
    builder.push_default(StyleProperty::FontStyle(if case.italic {
      FontStyle::Italic
    }
    else {
      FontStyle::Normal
    }));
    builder.push_default(StyleProperty::LineHeight(match self.tuning.line_height {
      Some(height) => LineHeight::Absolute(height),
      None => LineHeight::MetricsRelative(1.),
    }));
    builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::BreakWord));
    builder.push_default(StyleProperty::TextWrapMode(TextWrapMode::Wrap));

    if !instance.is_empty() {
      let list: Vec<FontVariation> = instance
        .iter()
        .filter_map(|(tag, value)| {
          Some(FontVariation::new(
            Tag::new(tag.as_bytes().try_into().ok()?),
            *value as f32,
          ))
        })
        .collect();

      builder.push_default(StyleProperty::FontVariations(FontVariations::List(
        Cow::Owned(list),
      )));
    }

    match Language::parse(&case.locale) {
      Ok(language) => builder.push_default(StyleProperty::Locale(Some(language))),
      Err(_) => notes.push(format!("locale {:?} does not parse", case.locale)),
    }

    builder.push_default(StyleProperty::LineBreak(match case.line_break {
      LineBreak::Normal => ParleyLineBreak::Normal,
      LineBreak::Strict => ParleyLineBreak::Strict,
      LineBreak::Loose => ParleyLineBreak::Loose,
      LineBreak::Anywhere => ParleyLineBreak::Anywhere,
    }));
    builder.push_default(StyleProperty::WordBreak(match case.word_break {
      WordBreak::Normal => ParleyWordBreak::Normal,
      WordBreak::BreakAll => ParleyWordBreak::BreakAll,
      WordBreak::KeepAll => ParleyWordBreak::KeepAll,
      WordBreak::Phrase => {
        notes.push("word_break phrase unsupported: broken as normal".to_owned());

        ParleyWordBreak::Normal
      }
    }));

    if self.tuning.korean_keep_all && case.word_break == WordBreak::Normal {
      for range in hangul_ranges(&case.text) {
        builder.push(StyleProperty::WordBreak(ParleyWordBreak::KeepAll), range);
      }
    }

    if !self.tuning.features.is_empty() {
      let features: Vec<FontFeature> = self
        .tuning
        .features
        .iter()
        .filter_map(|tag| {
          Some(FontFeature {
            tag: Tag::new(tag.as_bytes().try_into().ok()?),
            value: 1,
          })
        })
        .collect();

      builder.push_default(StyleProperty::FontFeatures(FontFeatures::List(Cow::Owned(
        features,
      ))));
    }

    if self.tuning.whole_clusters {
      builder.set_line_break_override(Some(&whole_clusters));
    }

    for (range, spacing, variations) in fallback {
      if let Some(spacing) = spacing {
        builder.push(StyleProperty::LetterSpacing(spacing), range.clone());
      }

      if let Some(variations) = variations {
        builder.push(
          StyleProperty::FontVariations(FontVariations::List(Cow::Owned(variations))),
          range,
        );
      }
    }

    for range in text_parity::emoji::emoji_ranges(&case.text) {
      let mut emoji_first = vec![FontFamilyName::Generic(GenericFamily::Emoji)];

      emoji_first.extend(names.iter().cloned());
      builder.push(
        StyleProperty::FontFamily(FontFamily::List(Cow::Owned(emoji_first))),
        range,
      );
    }

    if let Some(from) = anywhere_from {
      builder.push(
        StyleProperty::LineBreak(ParleyLineBreak::Anywhere),
        from..case.text.len(),
      );
    }

    builder.set_base_direction(match case.direction {
      Direction::Auto => BaseDirection::Auto,
      Direction::Ltr => BaseDirection::Ltr,
      Direction::Rtl => BaseDirection::Rtl,
    });

    Shaped {
      layout: builder.build(&case.text),
      text: case.text.clone(),
      max_lines: case.max_lines.map(|lines| lines.max(1) as usize),
      ellipsis: case.ellipsis,
      alignment: case.alignment,
      notes,
    }
  }

  /// The case as tuned: its text with soft hyphens where hyphenation is asked and tuned.
  #[must_use]
  pub fn tuned_case(&self, case: &Case) -> (Case, Option<Vec<usize>>, Vec<String>) {
    let mut notes = Vec::new();
    let mut tuned = case.clone();

    if self.tuning.natural_from_app
      && case.alignment == Alignment::Start
      && crate::textkit::is_rtl(&case.text, case.direction)
    {
      tuned.alignment = Alignment::End;
      notes.push("start is the app's left edge, as NSTextAlignment.natural is".to_owned());
    }

    if case.hyphenation != Hyphenation::Auto {
      return (tuned, None, notes);
    }

    if !self.tuning.hyphenate {
      notes.push("automatic hyphenation unsupported: soft hyphens only".to_owned());

      return (tuned, None, notes);
    }

    let points = hyphenation_points(&case.text, &case.locale);
    let (text, back) = with_soft_hyphens(&case.text, &points);

    notes.push(format!("{} hyphenation points from CFStringGetHyphenationLocationBeforeIndex, inserted as soft hyphens", points.len()));
    tuned.text = text;
    tuned.hyphenation = Hyphenation::Manual;

    (tuned, Some(back), notes)
  }

  /// The clusters of `case` that fall back to another font, with that font's tracking at
  /// `size` where it has a `trak` in force and its own instance where it varies.
  #[expect(clippy::type_complexity, reason = "a spike's private triple")]
  fn fallback_styles(
    &mut self,
    case: &Case,
    size: f32,
  ) -> Vec<(
    std::ops::Range<usize>,
    Option<f32>,
    Option<Vec<FontVariation>>,
  )> {
    if !self.tuning.trak && !self.tuning.fallback_instance {
      return Vec::new();
    }

    let base = platform::system_font(f64::from(size), case.weight, case.italic);
    let base_name = unsafe { platform::as_ct(&base).post_script_name() }.to_string();
    let mut out = Vec::new();

    for (at, cluster) in case.text.grapheme_indices(true) {
      let key = (base_name.clone(), case.locale.clone(), cluster.to_owned());
      let Some(Some(family)) = self.fallback_cache.get(&key).cloned()
      else {
        continue;
      };
      let style = self.fallback_style(&family, case, size);
      let spacing = style.tracking;
      let variations = style.instance.map(|instance| {
        instance
          .iter()
          .filter_map(|(tag, value)| {
            Some(FontVariation::new(
              Tag::new(tag.as_bytes().try_into().ok()?),
              *value as f32,
            ))
          })
          .collect()
      });

      if spacing.is_some() || variations.is_some() {
        out.push((at..at + cluster.len(), spacing, variations));
      }
    }

    out
  }

  /// The style `family`'s clusters take when Core Text falls back to its font from the system
  /// font at `size` in `case`'s weight and slant.
  fn fallback_style(&mut self, family: &str, case: &Case, size: f32) -> FallbackStyle {
    let key = (family.to_owned(), size.to_bits(), case.weight, case.italic);

    if let Some(found) = self.fallback_styles_cache.get(&key) {
      return found.clone();
    }

    let mut style = FallbackStyle::default();

    if let Some((cluster, locale)) = self.samples.get(family).cloned()
      && let Some(bytes) = self.bytes_of.get(family).cloned()
    {
      let base = platform::system_font(f64::from(size), case.weight, case.italic);
      let string = CFString::from_str(&cluster);
      let font = unsafe {
        CTFont::for_string_with_language(
          platform::as_ct(&base),
          &string,
          CFRange {
            location: 0,
            length: string.length(),
          },
          Some(&CFString::from_str(&locale)),
        )
      };
      let axes = coretext::axes(&font);
      let variation = coretext::variation(&font);
      let mut instance: Vec<(String, f64)> = axes
        .iter()
        .map(|axis| {
          let value = variation
            .iter()
            .find(|(tag, _)| *tag == axis.tag)
            .map_or(axis.default, |(_, value)| *value);

          (axis.tag.clone(), value)
        })
        .collect();

      // TextKit's own line of the cluster in the system font: its width, and the glyphs it
      // drew in the fallback font.
      let mut sample = case.clone();

      sample.text.clone_from(&cluster);
      sample.locale.clone_from(&locale);
      sample.max_lines = None;
      sample.ellipsis = false;

      let variant = Variant {
        case: &sample,
        size,
        width: None,
      };
      let laid = crate::textkit::variant(&variant, &crate::textkit::Setup::default(), 0);
      let name = coretext::info(&font).postscript;
      let ids: Vec<u16> = crate::isolate::placed(&laid.laid, &sample, false)
        .iter()
        .filter(|run| coretext::info(&run.font).postscript == name)
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.0))
        .collect();
      let theirs: f64 = laid
        .record
        .clusters
        .iter()
        .map(|cluster| f64::from(cluster.advance))
        .sum();
      let ours = |settings: &[(String, f64)]| advances((*bytes).as_ref(), &ids, size, settings);

      if self.tuning.fallback_instance
        && let Some(opsz) = axes.iter().find(|axis| axis.tag == "opsz")
        && !variation.iter().any(|(tag, _)| tag == "opsz")
      {
        let sized = f64::from(size).clamp(opsz.min, opsz.max);
        let mut at_size = instance.clone();

        for (tag, value) in &mut at_size {
          if tag == "opsz" {
            *value = sized;
          }
        }

        if let (Some(default), Some(optical)) = (ours(&instance), ours(&at_size)) {
          let chosen = (optical - theirs).abs() < (default - theirs).abs();

          self.optical_notes.push(format!(
            "{family} at {size}: Core Text {theirs:.2}, opsz {sized} {optical:.2}, default {default:.2}: {}",
            if chosen { "sized" } else { "default" }
          ));

          if chosen {
            instance = at_size;
          }
        }
      }

      if self.tuning.trak
        && Trak::of(&font).is_some()
        && let Some(our) = ours(&instance)
      {
        let tracking = (theirs - our) as f32;

        if tracking.abs() > 0.001 {
          style.tracking = Some(tracking);

          if let Some(blob) = self.blob_of.get(family) {
            self
              .tracking_by_blob
              .insert((*blob, size.to_bits()), tracking);
          }
        }
      }

      if self.tuning.fallback_instance && !instance.is_empty() {
        style.instance = Some(instance);
      }
    }

    self.fallback_styles_cache.insert(key, style.clone());

    style
  }

  /// The tracking in points of the glyphs of blob `blob` at `size`, where its font is tracked.
  #[must_use]
  pub fn tracking(&self, blob: u64, size: f32) -> Option<f32> {
    self.tracking_by_blob.get(&(blob, size.to_bits())).copied()
  }

  /// Core Text's instance of the system font at `size` for `case`'s weight and slant: each
  /// axis's tag and value.
  pub fn instance(&mut self, case: &Case, size: f32) -> Vec<(String, f64)> {
    self
      .instances
      .entry((size.to_bits(), case.weight, case.italic))
      .or_insert_with(|| {
        let font = platform::system_font(f64::from(size), case.weight, case.italic);

        coretext::variation(platform::as_ct(&font))
      })
      .clone()
  }

  /// Shapes the tuned case at `size` with its families.
  pub fn shape(&mut self, case: &Case, size: f32) -> (Shaped, Vec<String>) {
    let families = self.families(case, size);
    let shaped = self.shape_with(case, size, &families, None);

    (shaped, families)
  }

  /// Lays `variant` out tuned.
  pub fn lay_out(&mut self, variant: &Variant<'_>) -> TunedLaid {
    let (case, back, notes) = self.tuned_case(variant.case);
    let (shaped, families) = self.shape(&case, variant.size);
    let mut laid = shaped.break_at(variant.width);

    if laid.truncated && case.ellipsis {
      let last = laid.layout.len().saturating_sub(1);

      if let Some(tail) = laid.layout.get(last).map(|line| line.text_range().start) {
        let caps: Vec<f32> = laid
          .layout
          .lines()
          .take(last)
          .map(|line| line.metrics().advance - line.metrics().hanging_advance + 0.01)
          .collect();
        let again = self
          .shape_with(&case, variant.size, &families, Some(tail))
          .break_capped(variant.width, &caps);
        let starts = |laid: &Laid| -> Vec<usize> {
          laid
            .layout
            .lines()
            .map(|line| line.text_range().start)
            .collect()
        };

        if starts(&again) == starts(&laid) {
          laid = again;
        }
        else {
          laid
            .notes
            .push("ellipsis at a word: the grapheme cut moved an earlier break".to_owned());
        }
      }
    }

    laid.notes.extend(notes);

    TunedLaid {
      laid,
      case,
      back,
      families,
    }
  }

  /// The record of a tuned layout, its ranges in the original text.
  pub fn record(
    &mut self,
    variant: &Variant<'_>,
    tuned: &TunedLaid,
    intrinsic: Option<Intrinsic>,
  ) -> Record {
    let as_tuned = Variant {
      case: &tuned.case,
      size: variant.size,
      width: variant.width,
    };
    let mut record = tuned
      .laid
      .record(&as_tuned, &mut self.fonts, intrinsic, true);

    if let Some(back) = &tuned.back {
      map_back(&mut record, &variant.case.text, back);
    }

    record
  }

  /// Min- and max-content of the tuned variant.
  pub fn intrinsic(&mut self, variant: &Variant<'_>) -> Intrinsic {
    let (case, _, _) = self.tuned_case(variant.case);
    let (shaped, _) = self.shape(&case, variant.size);

    shaped.intrinsic()
  }
}

impl Shaped {
  /// Min- and max-content widths.
  #[must_use]
  pub fn intrinsic(&self) -> Intrinsic {
    let widths = self.layout.calculate_content_widths();

    Intrinsic {
      min: widths.min,
      max: widths.max,
    }
  }

  fn run_font(
    layout: &Layout<()>,
    line: usize,
    at: usize,
  ) -> Option<(FontData, f32, Vec<i16>, Synthesis)> {
    let line = layout.get(line)?;
    let mut found = None;

    for item in line.items() {
      if let PositionedLayoutItem::GlyphRun(glyphs) = item {
        let run = glyphs.run();
        let fits = run.text_range().contains(&at) || run.text_range().end == at;

        if fits || found.is_none() {
          found = Some((
            run.font().clone(),
            run.font_size(),
            run
              .normalized_coords()
              .iter()
              .map(|coord| coord.to_bits())
              .collect(),
            run.synthesis(),
          ));
        }
      }
    }

    found
  }

  fn content_edges(text: &str, layout: &Layout<()>, line: usize) -> Option<(f32, f32)> {
    let line = layout.get(line)?;
    let metrics = *line.metrics();
    let blank = trailing_blank(text, &line);
    let left = metrics.offset + if layout.is_rtl() { blank } else { 0. };

    Some((left, left + metrics.advance - blank))
  }

  fn mark(
    text: &str,
    layout: &Layout<()>,
    line: usize,
    at: usize,
    ellipsis: bool,
    pick: impl Fn(&FontData, &[i16], f32) -> Option<Vec<(u32, f32)>>,
  ) -> Option<Mark> {
    let (font, size, coords, synthesis) = Self::run_font(layout, line, at)?;
    let glyphs = pick(&font, &coords, size)?;
    let advance: f32 = glyphs.iter().map(|(_, advance)| advance).sum();
    let (left, right) = Self::content_edges(text, layout, line)?;
    let mut x = if layout.is_rtl() {
      left - advance
    }
    else {
      right
    };
    let placed = glyphs
      .into_iter()
      .map(|(glyph, advance)| {
        let at = x;

        x += advance;

        (glyph, at)
      })
      .collect();

    Some(Mark {
      font,
      size,
      coords,
      synthesis,
      glyphs: placed,
      advance,
      baseline: layout.get(line)?.metrics().baseline,
      at,
      line,
      ellipsis,
    })
  }

  fn hyphen(&self, layout: &Layout<()>, line: usize) -> Option<Mark> {
    let info = layout.get(line)?;
    let range = info.text_range();

    if info.break_reason() != BreakReason::Regular || !self.text[range.clone()].ends_with('\u{AD}')
    {
      return None;
    }

    Self::mark(
      &self.text,
      layout,
      line,
      range.end - 2,
      false,
      |font, coords, size| {
        glyph_of(font, coords, size, '\u{2010}')
          .or_else(|| glyph_of(font, coords, size, '-'))
          .map(|glyph| vec![glyph])
      },
    )
  }

  fn ellipsis_mark(&self, layout: &Layout<()>, line: usize) -> Option<Mark> {
    let info = layout.get(line)?;
    let end = info.text_range().end;
    let at = self.text[..end]
      .char_indices()
      .next_back()
      .map_or(0, |(at, _)| at);

    Self::mark(&self.text, layout, line, at, true, |font, coords, size| {
      glyph_of(font, coords, size, '\u{2026}')
        .map(|glyph| vec![glyph])
        .or_else(|| glyph_of(font, coords, size, '.').map(|dot| vec![dot; 3]))
    })
  }

  fn break_into(
    layout: &mut Layout<()>,
    limit: f32,
    (reserve, caps): (&[(usize, f32)], &[f32]),
    max: Option<usize>,
  ) -> bool {
    let mut breaker = layout.break_lines();
    let mut lines = 0;

    breaker.state_mut().set_layout_max_advance(limit);

    loop {
      if max.is_some_and(|max| lines == max) {
        break;
      }

      let room = reserve
        .iter()
        .find(|(line, _)| *line == lines)
        .map_or(limit, |(_, amount)| (limit - amount).max(0.))
        .min(caps.get(lines).copied().unwrap_or(f32::MAX));

      breaker.state_mut().set_line_max_advance(room);

      if breaker.break_next().is_none() {
        break;
      }

      lines += 1;
    }

    let left = !breaker.is_done();

    breaker.finish();

    left
  }

  /// The layout at `width` (`None` is unconstrained), marks included.
  #[must_use]
  pub fn break_at(&self, width: Option<f32>) -> Laid {
    self.break_capped(width, &[])
  }

  fn break_capped(&self, width: Option<f32>, caps: &[f32]) -> Laid {
    let limit = width.unwrap_or(f32::MAX);
    let mut layout = self.layout.clone();
    let mut reserve: Vec<(usize, f32)> = Vec::new();
    let mut left = false;

    for _ in 0..4 {
      left = Self::break_into(&mut layout, limit, (&reserve, caps), self.max_lines);

      let mut asked = false;

      for line in 0..layout.len() {
        if reserve.iter().any(|(at, _)| *at == line) {
          continue;
        }

        let mark = if left && self.ellipsis && Some(line + 1) == self.max_lines {
          self.ellipsis_mark(&layout, line)
        }
        else {
          self.hyphen(&layout, line)
        };
        let Some(mark) = mark
        else {
          continue;
        };
        let used = layout.get(line).map_or(0., |line| {
          line.metrics().advance - trailing_blank(&self.text, &line)
        });

        if used + mark.advance > limit {
          reserve.push((line, mark.advance));
          asked = true;

          break;
        }
      }

      if !asked {
        break;
      }
    }

    let truncated = left
      && self.max_lines.is_some_and(|max| layout.len() >= max)
      && layout
        .get(layout.len().saturating_sub(1))
        .is_some_and(|line| line.text_range().end < self.text.len());

    layout.align(
      match self.alignment {
        Alignment::Start => ParleyAlignment::Start,
        Alignment::Center => ParleyAlignment::Center,
        Alignment::End => ParleyAlignment::End,
        Alignment::Justify => ParleyAlignment::Justify,
      },
      AlignmentOptions::default(),
    );

    let mut marks = Vec::new();

    for line in 0..layout.len() {
      let mark = if truncated && self.ellipsis && line + 1 == layout.len() {
        self.ellipsis_mark(&layout, line)
      }
      else {
        self.hyphen(&layout, line)
      };

      marks.extend(mark);
    }

    Laid {
      layout,
      text: self.text.clone(),
      marks,
      truncated,
      notes: self.notes.clone(),
    }
  }
}
