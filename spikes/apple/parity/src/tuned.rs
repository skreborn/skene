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
//!
//! The breaking, the soft hyphen's hyphen and the ellipsis are the harness's
//! (`text_parity::layout`), copied here because its shaping step has no hook for variations or
//! a family list.
#![allow(unused_unsafe)]

use std::{borrow::Cow, collections::HashMap, sync::Arc, time::Instant};

use objc2_core_foundation::{CFIndex, CFLocale, CFRange, CFRetained, CFString};
use objc2_core_text::CTFont;
use parley::{
  Alignment as ParleyAlignment,
  AlignmentOptions,
  BaseDirection,
  BreakReason,
  FontData,
  FontFamily,
  FontFamilyName,
  FontStyle,
  FontVariation,
  FontVariations,
  FontWeight,
  GenericFamily,
  Layout,
  LayoutContext,
  LineBreak as ParleyLineBreak,
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

/// What a font with Apple's `hvgl` outlines (PingFang UI on iOS 18 and macOS 15) becomes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hvgl {
  /// The font is registered without its `hvgl` and `hvpm` tables: Parley shapes it from its
  /// other tables and only Core Text can draw it (C).
  Strip,
  /// The font's public sibling is registered in its place (`PingFangSC-Regular` for
  /// `.PingFangUITextSC-Regular`), whose outlines vello draws (B).
  Substitute,
}

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
  /// Korean keeps its words whole under the normal word break, as TextKit's `standard`
  /// strategy does.
  pub korean_keep_all: bool,
  /// What an `hvgl` font becomes.
  pub hvgl: Hvgl,
  /// A line height in points for every line (a text style's line height and leading).
  pub line_height: Option<f32>,
}

/// A registered Core Text font.
#[derive(Clone)]
struct Registered {
  family: String,
}

/// Parley over Core Text's fonts.
pub struct TunedEngine {
  /// The collection, holding only what Core Text resolved.
  pub fonts: Fonts,
  layouts: LayoutContext<()>,
  /// What is on.
  pub tuning: Tuning,
  registered: HashMap<String, Registered>,
  refused: std::collections::HashSet<String>,
  instances: HashMap<(u32, u16, bool), Vec<(String, f64)>>,
  fallback_cache: HashMap<(String, String, String), Option<String>>,
  /// The Core Text font each registered blob came from, by blob id.
  pub sources: HashMap<u64, CFRetained<CTFont>>,
  /// Time spent building sfnts from tables and registering them.
  pub register_ms: f64,
  /// Bytes of sfnt registered.
  pub registered_bytes: usize,
  /// Fonts fontique refused (an `hvgl` font, say).
  pub rejected: Vec<String>,
  /// The families registered, in order: family, PostScript name, file.
  pub log: Vec<(String, String, Option<String>)>,
  /// What registration changed: `hvgl` stripped or substituted.
  pub notes: Vec<String>,
}

/// The family name the system UI font is registered under.
pub const SYSTEM: &str = "ct system";

/// The family name the emoji font is registered under.
pub const EMOJI: &str = "ct emoji";

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
    let mut engine = Self {
      fonts,
      layouts: LayoutContext::new(),
      tuning,
      registered: HashMap::new(),
      refused: std::collections::HashSet::new(),
      instances: HashMap::new(),
      fallback_cache: HashMap::new(),
      sources: HashMap::new(),
      register_ms: 0.,
      registered_bytes: 0,
      rejected: Vec::new(),
      log: Vec::new(),
      notes: Vec::new(),
    };
    let emoji = unsafe {
      CTFont::with_name(
        &CFString::from_str("AppleColorEmoji"),
        17.,
        std::ptr::null(),
      )
    };

    if engine.register(&emoji, Some(EMOJI), None).is_some()
      && let Some(id) = engine.fonts.context.collection.family_id(EMOJI)
    {
      engine
        .fonts
        .context
        .collection
        .set_generic_families(GenericFamily::Emoji, [id].into_iter());
    }

    engine
  }

  /// Registers `font` (once per file and face) under `family` (its own PostScript name where
  /// `None`): the family name Parley asks for.
  pub fn register(
    &mut self,
    font: &CTFont,
    family: Option<&str>,
    italic: Option<bool>,
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

    if let Some(found) = self.registered.get(&key) {
      return Some(found.family.clone());
    }

    if self.refused.contains(&key) {
      return None;
    }

    let started = Instant::now();
    let family = family.map_or_else(|| format!("ct {}", info.postscript), str::to_owned);
    let has_hvgl = coretext::tables(font).contains(&coretext::tag_of("hvgl"));

    if has_hvgl && self.tuning.hvgl == Hvgl::Substitute {
      let public = info
        .postscript
        .trim_start_matches('.')
        .replace("UIText", "")
        .replace("UIDisplay", "")
        .replace("UI", "");
      let candidate =
        unsafe { CTFont::with_name(&CFString::from_str(&public), info.size, std::ptr::null()) };
      let found = coretext::info(&candidate);
      let usable = found.postscript == public
        && !coretext::tables(&candidate).contains(&coretext::tag_of("hvgl"));

      self.refused.insert(key.clone());

      if !usable {
        self.notes.push(format!(
          "{}: hvgl, and no public sibling {public} (Core Text gave {})",
          info.postscript, found.postscript
        ));

        return None;
      }

      self.notes.push(format!(
        "{}: hvgl, substituted by {} at {:?}",
        info.postscript, found.postscript, found.path
      ));

      return self.register(&candidate, Some(&family), italic);
    }

    let bytes = if has_hvgl {
      self.notes.push(format!(
        "{}: hvgl and hvpm stripped; shaped from its other tables, drawn only by Core Text",
        info.postscript
      ));

      coretext::sfnt_except(font, &["hvgl", "hvpm"])
    }
    else {
      coretext::sfnt(font)
    };
    let Some(bytes) = bytes
    else {
      self.refused.insert(key.clone());
      self
        .rejected
        .push(format!("{}: no tables", info.postscript));

      return None;
    };

    self.registered_bytes += bytes.len();

    let blob = Blob::new(Arc::new(bytes));
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

    if let Some(system) = self.register(base, Some(SYSTEM), Some(case.italic)) {
      families.push(system);
    }

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
        let found = self.register(&font, None, None);

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

    if self.tuning.korean_keep_all
      && case.locale.starts_with("ko")
      && case.word_break == WordBreak::Normal
    {
      let mut korean = case.clone();

      korean.word_break = WordBreak::KeepAll;
      notes.push("Korean keeps its words whole, as TextKit's standard strategy does".to_owned());

      return (korean, None, notes);
    }

    if case.hyphenation != Hyphenation::Auto {
      return (case.clone(), None, notes);
    }

    if !self.tuning.hyphenate {
      notes.push("automatic hyphenation unsupported: soft hyphens only".to_owned());

      return (case.clone(), None, notes);
    }

    let points = hyphenation_points(&case.text, &case.locale);
    let (text, back) = with_soft_hyphens(&case.text, &points);
    let mut tuned = case.clone();

    notes.push(format!("{} hyphenation points from CFStringGetHyphenationLocationBeforeIndex, inserted as soft hyphens", points.len()));
    tuned.text = text;
    tuned.hyphenation = Hyphenation::Manual;

    (tuned, Some(back), notes)
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
