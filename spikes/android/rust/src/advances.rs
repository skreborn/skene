//! Hinted advances: how Minikin advances a glyph, the rules that might reproduce it measured
//! against the platform, and the chosen rule set as Parley's advances before line breaking,
//! through the spike's patched `parley_engine` (`vendor/parley_engine`, `AdvanceRule`).
//!
//! A rule replaces a glyph's base advance (its `hmtx` advance at the run's variation location)
//! and keeps whatever positioning moved it by (kerning, `chws`), scaled at the pixel size, or at
//! the floored ppem where the rule floors it.

use std::{
  collections::{BTreeMap, HashMap},
  sync::{Arc, Mutex},
};

use jni::{
  objects::{JClass, JString},
  sys::{jfloat, jstring},
  JNIEnv,
};
use parley_engine::{AdvanceRule, FontData, NormalizedCoord, RunAdvances};
use serde_json::{json, Value};
use skrifa::{
  instance::{LocationRef, Size},
  metrics::GlyphMetrics,
  outline::{
    DrawSettings,
    Engine,
    GlyphStyles,
    HintingInstance,
    HintingOptions,
    OutlinePen,
    SmoothMode,
    Target,
  },
  raw::{types::F2Dot14, TableProvider as _},
  FontRef,
  GlyphId,
  MetadataProvider as _,
};

use crate::ndk::{Matcher, VARIANT_COMPACT};

/// A rule for a glyph's base advance in device pixels, from its linear advance there.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Rule {
  /// The linear advance: Parley's own.
  Linear,
  /// Rounded to the nearest pixel.
  Round,
  /// Rounded down.
  Floor,
  /// Rounded up.
  Ceil,
  /// The linear advance at the pixel size rounded to a whole ppem, rounded.
  RoundPpem,
  /// The linear advance at the pixel size rounded down to a whole ppem, rounded.
  FloorPpem,
  /// FreeType's 26.6 arithmetic (`FT_MulFix` by the size's scale), rounded as `FT_PIX_ROUND`.
  FtRound,
  /// FreeType's 26.6 arithmetic at the pixel size rounded down to a whole ppem, rounded as
  /// `FT_PIX_ROUND`, with positioning (kerning, `chws`) scaled at that ppem: what Minikin's layout
  /// measured from API 31.
  FreeTypeFloor,
  /// [`Rule::FreeTypeFloor`] with positioning rounded to whole pixels: what Minikin's layout
  /// measured on API 29.
  FreeTypeFloorWhole,
  /// FreeType's size request and grid-fitted advance: a TrueType font whose `head` forces an
  /// integer ppem (bit 3) is scaled at the rounded ppem (`tt_size_reset`), any other at the pixel
  /// size itself, and the advance is `FT_PIX_ROUND` of `FT_MulFix` by that scale: what
  /// `Font.getGlyphBounds` measured.
  FreeType,
  /// The hinted advance, as FreeType's default load (`FT_LOAD_TARGET_NORMAL`) reports it:
  /// the font's own hinting where it has instructions, the automatic hinter where not.
  Hinted,
  /// The automatic hinter's advance in its normal mode, for every font.
  HintedAuto,
  /// The hinted advance in the light mode (`FT_LOAD_TARGET_LIGHT`).
  HintedLight,
}

impl Rule {
  /// Every rule, in the order reports list them; a tie in matches goes to the earlier.
  pub const ALL: [Self; 13] = [
    Self::FreeTypeFloor,
    Self::FreeTypeFloorWhole,
    Self::Linear,
    Self::Round,
    Self::Floor,
    Self::Ceil,
    Self::RoundPpem,
    Self::FloorPpem,
    Self::FtRound,
    Self::FreeType,
    Self::Hinted,
    Self::HintedAuto,
    Self::HintedLight,
  ];

  /// The rule's name in reports and arguments.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::Linear => "linear",
      Self::Round => "round",
      Self::Floor => "floor",
      Self::Ceil => "ceil",
      Self::RoundPpem => "round_ppem",
      Self::FloorPpem => "floor_ppem",
      Self::FtRound => "ft_round",
      Self::FreeType => "freetype",
      Self::FreeTypeFloor => "freetype_floor",
      Self::FreeTypeFloorWhole => "freetype_floor_whole",
      Self::Hinted => "hinted",
      Self::HintedAuto => "hinted_auto",
      Self::HintedLight => "hinted_light",
    }
  }

  /// The rule named `name`.
  #[must_use]
  pub fn parse(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|rule| rule.name() == name)
  }

  const fn hinting(self) -> Option<(bool, SmoothMode)> {
    match self {
      Self::Hinted => Some((false, SmoothMode::Normal)),
      Self::HintedAuto => Some((true, SmoothMode::Normal)),
      Self::HintedLight => Some((false, SmoothMode::Light)),
      _ => None,
    }
  }
}

struct NullPen;

impl OutlinePen for NullPen {
  fn move_to(&mut self, _x: f32, _y: f32) {}

  fn line_to(&mut self, _x: f32, _y: f32) {}

  fn quad_to(&mut self, _cx0: f32, _cy0: f32, _x: f32, _y: f32) {}

  fn curve_to(&mut self, _cx0: f32, _cy0: f32, _cx1: f32, _cy1: f32, _x: f32, _y: f32) {}

  fn close(&mut self) {}
}

/// FreeType's `FT_MulFix`: `a * b / 65536`, rounded half away from zero.
fn mul_fix(a: i64, b: i64) -> i64 {
  let product = a * b;
  let rounded = (product.abs() + 0x8000) >> 16;

  if product < 0 {
    -rounded
  }
  else {
    rounded
  }
}

/// FreeType's `FT_DivFix`: `a * 65536 / b`, rounded.
fn div_fix(a: i64, b: i64) -> i64 {
  ((a << 16) + b / 2) / b
}

/// One font at one pixel size and location: the rule's advance of each glyph, cached.
pub struct Sizing {
  data: Arc<dyn AsRef<[u8]> + Send + Sync>,
  index: u32,
  coords: Vec<F2Dot14>,
  /// The device pixels per em.
  pub px: f32,
  /// The font's units per em.
  pub upem: f32,
  /// Whether FreeType scales the font at a whole ppem: TrueType outlines and `head` bit 3.
  pub integer_ppem: bool,
  rule: Rule,
  hinting: Option<HintingInstance>,
  cache: Mutex<HashMap<u32, (f32, f32)>>,
}

impl Sizing {
  /// The font in `data` (face `index`) at `px` pixels per em and `coords`, under `rule`; the
  /// automatic hinter's glyph styles come from `styles` where given.
  pub fn new(
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    index: u32,
    coords: Vec<F2Dot14>,
    px: f32,
    rule: Rule,
    styles: Option<GlyphStyles>,
  ) -> Option<Self> {
    let font = FontRef::from_index((*data).as_ref(), index).ok()?;
    let head = font.head().ok()?;
    let upem = f32::from(head.units_per_em());
    let integer_ppem = head
      .flags()
      .contains(skrifa::raw::tables::head::Flags::FORCE_INTEGER_PPEM)
      && font.table_data(skrifa::Tag::new(b"glyf")).is_some();
    let hinting = match rule.hinting() {
      Some((auto, mode)) => {
        let outlines = font.outline_glyphs();
        let engine = if auto {
          Engine::Auto(styles)
        }
        else if outlines.prefer_interpreter() {
          Engine::Interpreter
        }
        else {
          Engine::Auto(styles)
        };
        let options = HintingOptions {
          engine,
          target: Target::Smooth {
            mode,
            symmetric_rendering: true,
            preserve_linear_metrics: false,
          },
        };

        HintingInstance::new(&outlines, Size::new(px), LocationRef::new(&coords), options).ok()
      }
      None => None,
    };

    Some(Self {
      data,
      index,
      coords,
      px,
      upem,
      integer_ppem,
      rule,
      hinting,
      cache: Mutex::new(HashMap::new()),
    })
  }

  /// The pixels per em positioning (kerning, `chws`) is scaled at under the rule: the floored
  /// ppem where the rule floors it, else the pixel size.
  #[must_use]
  pub fn positioning_px(&self) -> f32 {
    match self.rule {
      Rule::FreeTypeFloor | Rule::FreeTypeFloorWhole => self.px.floor(),
      _ => self.px,
    }
  }

  /// The glyph's linear base advance in font units and its advance under the rule in pixels.
  pub fn advance(&self, glyph: u32) -> (f32, f32) {
    if let Some(found) = self.cache.lock().ok().and_then(|cache| cache.get(&glyph).copied()) {
      return found;
    }

    let found = self.compute(glyph).unwrap_or((0., 0.));

    if let Ok(mut cache) = self.cache.lock() {
      cache.insert(glyph, found);
    }

    found
  }

  fn compute(&self, glyph: u32) -> Option<(f32, f32)> {
    let font = FontRef::from_index((*self.data).as_ref(), self.index).ok()?;
    let location = LocationRef::new(&self.coords);
    let id = GlyphId::new(glyph);
    let units = GlyphMetrics::new(&font, Size::unscaled(), location).advance_width(id)?;
    let linear = units * self.px / self.upem;
    let ppem = |whole: f32| (units * whole / self.upem).round();
    let rule = match self.rule {
      Rule::Linear => linear,
      Rule::Round => linear.round(),
      Rule::Floor => linear.floor(),
      Rule::Ceil => linear.ceil(),
      Rule::RoundPpem => ppem(self.px.round()),
      Rule::FloorPpem => ppem(self.px.floor()),
      Rule::FtRound | Rule::FreeType | Rule::FreeTypeFloor | Rule::FreeTypeFloorWhole => {
        let requested = (f64::from(self.px) * 64.).round() as i64;
        let scaled = if matches!(self.rule, Rule::FreeTypeFloor | Rule::FreeTypeFloorWhole) {
          (requested >> 6) << 6
        }
        else if self.rule == Rule::FreeType && self.integer_ppem {
          ((requested + 32) >> 6) << 6
        }
        else {
          requested
        };
        let scale = div_fix(scaled, self.upem as i64);
        let advance = mul_fix(units.round() as i64, scale);

        ((advance + 32) & !63) as f32 / 64.
      }
      Rule::Hinted | Rule::HintedAuto | Rule::HintedLight => {
        let hinted = self.hinting.as_ref().and_then(|instance| {
          let outlines = font.outline_glyphs();
          let outline = outlines.get(id)?;

          outline
            .draw(DrawSettings::hinted(instance, false), &mut NullPen)
            .ok()?
            .advance_width
        });

        hinted.unwrap_or_else(|| linear.round())
      }
    };

    Some((units, rule))
  }
}

type SizingKey = (u64, u32, Vec<i16>, u32);

/// The rule as Parley's advances: device pixels are `density` per layout unit.
pub struct HintedAdvances {
  rule: Rule,
  density: f32,
  sizings: Mutex<HashMap<SizingKey, Arc<Sizing>>>,
  styles: Mutex<HashMap<(u64, u32), Option<GlyphStyles>>>,
}

impl HintedAdvances {
  /// The rule at `density`.
  #[must_use]
  pub fn new(rule: Rule, density: f32) -> Self {
    Self {
      rule,
      density,
      sizings: Mutex::new(HashMap::new()),
      styles: Mutex::new(HashMap::new()),
    }
  }

  fn sizing(&self, font: &FontData, coords: &[NormalizedCoord], font_size: f32) -> Option<Arc<Sizing>> {
    let bits: Vec<i16> = coords.iter().map(|coord| coord.to_bits()).collect();
    let key = (font.data.id(), font.index, bits.clone(), font_size.to_bits());

    if let Some(found) = self.sizings.lock().ok()?.get(&key) {
      return Some(found.clone());
    }

    let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(font.data.clone());
    let styles = self.rule.hinting().and_then(|_| {
      let mut styles = self.styles.lock().ok()?;

      styles
        .entry((font.data.id(), font.index))
        .or_insert_with(|| {
          FontRef::from_index(font.data.data(), font.index)
            .ok()
            .map(|font| GlyphStyles::new(&font.outline_glyphs()))
        })
        .clone()
    });
    let sizing = Arc::new(Sizing::new(
      data,
      font.index,
      bits.iter().map(|bits| F2Dot14::from_bits(*bits)).collect(),
      font_size * self.density,
      self.rule,
      styles,
    )?);

    self.sizings.lock().ok()?.insert(key, sizing.clone());

    Some(sizing)
  }
}

impl AdvanceRule for HintedAdvances {
  fn run<'a>(
    &'a self,
    font: &'a FontData,
    coords: &'a [NormalizedCoord],
    font_size: f32,
  ) -> Option<RunAdvances<'a>> {
    let sizing = self.sizing(font, coords, font_size)?;
    let density = self.density;

    Some(Box::new(move |glyph, shaped| {
      if shaped == 0 {
        return None;
      }

      let (units, rule) = sizing.advance(glyph);
      let positioning = (shaped as f32 - units.round()) * sizing.positioning_px() / sizing.upem;
      let positioning = if sizing.rule == Rule::FreeTypeFloorWhole {
        positioning.round()
      }
      else {
        positioning
      };

      Some((rule + positioning) / density)
    }))
  }
}

fn string(env: &mut JNIEnv, value: &JString) -> String {
  env.get_string(value).map(Into::into).unwrap_or_default()
}

fn answer(env: &mut JNIEnv, text: &str) -> jstring {
  env
    .new_string(text)
    .map(|value| value.into_raw())
    .unwrap_or(std::ptr::null_mut())
}

/// Sets Parley's advances to the rule named `rule` at `density` (`linear` restores Parley's
/// own): the rule's name where set, or `error: ` and why.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_setAdvanceRule<'local>(
  mut env: JNIEnv<'local>,
  _class: JClass<'local>,
  rule: JString<'local>,
  density: jfloat,
) -> jstring {
  let name = string(&mut env, &rule);
  let result = match Rule::parse(&name) {
    Some(Rule::Linear) => {
      parley_engine::set_advance_rule(None);

      "linear".to_owned()
    }
    Some(rule) => {
      parley_engine::set_advance_rule(Some(Arc::new(HintedAdvances::new(rule, density))));

      rule.name().to_owned()
    }
    None => format!("error: no rule {name:?}"),
  };

  answer(&mut env, &result)
}

/// The fonts the advance probe measures: a name, the family and sample the matcher is asked
/// with, its locale list, the weight, the characters measured one by one, the pairs measured
/// for kerning, and whether a lone character's glyph is the one its cmap names (so a whole-string
/// measurement measures that glyph). The CJK font's punctuation pairs ([`CHWS`]) are measured
/// with `chws` on and off.
const PROBE: &[(&str, &str, &str, &str, u16, &str, &[&str], bool)] = &[
  ("sans", "sans-serif", "a", "en-US", 400, LATIN, KERNING, true),
  ("sans-bold", "sans-serif", "a", "en-US", 700, LATIN, KERNING, true),
  ("serif", "serif", "a", "en-US", 400, LATIN, KERNING, true),
  ("mono", "monospace", "a", "en-US", 400, LATIN, &[], true),
  ("cjk", "sans-serif", "今", "ja-JP", 400, CJK, &[], true),
  ("thai", "sans-serif", "ก", "th-TH", 400, THAI, &[], true),
  ("arabic", "sans-serif", "ب", "ar", 400, ARABIC, &[], false),
  ("devanagari", "sans-serif", "क", "hi-IN", 400, DEVANAGARI, &[], false),
];

const LATIN: &str = "!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~éßñ";
const CJK: &str = "今日は天気永東国語あいうアイウ。、「」（）！？ＡＢａｂ０１AaBbgfi1ｱｲ";
const THAI: &str = "กขคฆงจฉชซฌญฎฏฐฑฒณดตถทธนบปผฝพฟภมยรลวศษสหฬอฮ";
const ARABIC: &str = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي";
const DEVANAGARI: &str = "कखगघङचछजझञटठडढणतथदधनपफबभमयरलवशषसह";
const KERNING: &[&str] = &[
  "AV", "AW", "AY", "To", "Ta", "Te", "Ty", "Vo", "Wa", "Yo", "LT", "LV", "P.", "F,", "r.", "y.",
  "Av", "ko", "\"A", "11",
];
const CHWS: &[&str] = &["。「", "」（", "、「", "）。", "」」", "「「", "（「"];

/// The dp sizes the probe measures at.
pub const SIZES: [f32; 20] = [
  8., 9., 10., 11., 12., 13., 14., 15., 16., 17., 18., 20., 22., 24., 28., 32., 34., 40., 48., 64.,
];

/// What the advance probe measures: per font, its file, face and axes (the matcher's pick),
/// and each character's glyph by the font's cmap.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_advanceProbe<'local>(
  mut env: JNIEnv<'local>,
  _class: JClass<'local>,
) -> jstring {
  let mut fonts = Vec::new();

  for (name, family, sample, locales, weight, chars, pairs, simple) in PROBE {
    let matcher = Matcher::new(*weight, false, locales, VARIANT_COMPACT);
    let units: Vec<u16> = sample.encode_utf16().collect();
    let Some((font, _)) = matcher.first(family, &units)
    else {
      continue;
    };
    let Ok(data) = std::fs::read(&font.path)
    else {
      continue;
    };
    let Ok(face) = FontRef::from_index(&data, font.index)
    else {
      continue;
    };
    let charmap = face.charmap();
    let glyphs: Vec<Value> = chars
      .chars()
      .filter_map(|character| {
        let glyph = charmap.map(character)?;

        (glyph.to_u32() != 0).then(|| json!({"char": character.to_string(), "glyph": glyph.to_u32()}))
      })
      .collect();
    let axes = font
      .axes
      .iter()
      .map(|(tag, value)| format!("'{tag}' {value}"))
      .collect::<Vec<_>>()
      .join(",");

    fonts.push(json!({
      "name": name, "path": font.path, "index": font.index, "axes": axes, "weight": weight,
      "locale": locales, "simple": simple, "glyphs": glyphs, "pairs": pairs,
      "chws": if *name == "cjk" { CHWS } else { &[] },
    }));
  }

  answer(&mut env, &json!({"sizes_dp": SIZES, "fonts": fonts}).to_string())
}

fn numbers(value: &Value) -> Vec<Option<f64>> {
  value
    .as_array()
    .map(|array| array.iter().map(Value::as_f64).collect())
    .unwrap_or_default()
}

/// The coordinates of `axes` ("'wght' 700,...") in `font`.
fn coords_of(font: &FontRef<'_>, axes: &str) -> Vec<F2Dot14> {
  let settings: Vec<(skrifa::Tag, f32)> = axes
    .split(',')
    .filter_map(|setting| {
      let setting = setting.trim();
      let tag = setting.get(1..5)?;
      let value: f32 = setting.get(6..)?.trim().parse().ok()?;

      Some((skrifa::Tag::new_checked(tag.as_bytes()).ok()?, value))
    })
    .collect();

  font
    .axes()
    .location(settings.iter().copied())
    .coords()
    .to_vec()
}

/// The advance in dp of `text` laid out by Parley in face `index` of `data` at `size` dp and
/// `axes`, under the process's advance rule.
fn parley_advance(data: &Arc<dyn AsRef<[u8]> + Send + Sync>, index: u32, axes: &str, text: &str, size: f32) -> Option<f32> {
  use std::borrow::Cow;

  use parley::{
    fontique::Blob,
    FontContext,
    FontFamily,
    FontVariation,
    FontVariations,
    LayoutContext,
    StyleProperty,
  };

  let mut fonts = FontContext::default();
  let added = fonts.collection.register_fonts(Blob::new(data.clone()), None);
  let family = added
    .iter()
    .find(|(_, infos)| infos.iter().any(|info| info.index() == index))
    .map(|(family, _)| *family)?;
  let name = fonts.collection.family_name(family)?.to_owned();
  let variations: Vec<FontVariation> = axes
    .split(',')
    .filter_map(|setting| {
      let setting = setting.trim();
      let tag = setting.get(1..5)?;
      let value: f32 = setting.get(6..)?.trim().parse().ok()?;

      Some(FontVariation::new(parley::setting::Tag::new(tag.as_bytes().try_into().ok()?), value))
    })
    .collect();
  let mut layouts: LayoutContext<()> = LayoutContext::new();
  let mut builder = layouts.ranged_builder(&mut fonts, text, 1., false);

  builder.push_default(StyleProperty::FontFamily(FontFamily::named(&name)));
  builder.push_default(StyleProperty::FontSize(size));
  builder.push_default(StyleProperty::FontVariations(FontVariations::List(Cow::Owned(variations))));

  let mut layout = builder.build(text);

  layout.break_all_lines(None);

  let advance = layout.lines().next().map(|line| line.metrics().advance);

  advance
}

/// Each kerning pair laid out by Parley under `rule` at every size against the platform's
/// measurement of the pair: how many agree within 0.02 px, and the largest difference.
fn pairs_under(rule: Rule, density: f32, sizes: &[f32], font: &Value, data: &Arc<dyn AsRef<[u8]> + Send + Sync>, index: u32) -> Value {
  parley_engine::set_advance_rule(match rule {
    Rule::Linear => None,
    rule => Some(Arc::new(HintedAdvances::new(rule, density))),
  });

  let axes = font["axes"].as_str().unwrap_or_default();
  let mut agree = 0usize;
  let mut count = 0usize;
  let mut worst = 0f64;
  let mut misses = Vec::new();

  for pair in font["pairs"].as_array().cloned().unwrap_or_default() {
    let text = pair["text"].as_str().unwrap_or_default();
    let measured = numbers(&pair["pair_hinted"]);

    for (at, size) in sizes.iter().enumerate() {
      let (Some(Some(measured)), Some(ours)) = (measured.get(at), parley_advance(data, index, axes, text, *size))
      else {
        continue;
      };
      let delta = (f64::from(ours * density) - measured).abs();

      count += 1;
      worst = worst.max(delta);

      if delta < 0.02 {
        agree += 1;
      }
      else if misses.len() < 8 {
        misses.push(json!({"text": text, "px": size * density, "measured": measured, "parley": ours * density}));
      }
    }
  }

  parley_engine::set_advance_rule(None);

  json!({"matched": agree, "of": count, "max_delta_px": worst, "misses": misses})
}

/// Which rules predict the measured advances: per font and rule, how many (glyph, size) the
/// rule matches within 0.01 px, with the misses of the best; kerning and `chws` against the
/// linear scale; and the rule that matches most overall.
fn analyze(measured: &Value) -> Value {
  let density = measured["density"].as_f64().unwrap_or(1.) as f32;
  let sizes: Vec<f32> = numbers(&measured["sizes_dp"])
    .into_iter()
    .map(|size| size.unwrap_or(0.) as f32)
    .collect();
  let mut totals: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
  let mut pair_totals: BTreeMap<String, (u64, u64)> = BTreeMap::new();
  let mut fonts_out = Vec::new();

  for font in measured["fonts"].as_array().cloned().unwrap_or_default() {
    let path = font["path"].as_str().unwrap_or_default();
    let index = font["index"].as_u64().unwrap_or(0) as u32;
    let Ok(data) = std::fs::read(path)
    else {
      continue;
    };
    let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(data);
    let Ok(face) = FontRef::from_index((*data).as_ref(), index)
    else {
      continue;
    };
    let coords = coords_of(&face, font["axes"].as_str().unwrap_or_default());
    let styles = GlyphStyles::new(&face.outline_glyphs());
    let mut per_rule: BTreeMap<&str, (usize, usize, Vec<Value>)> = BTreeMap::new();
    let mut per_rule_bounds: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut linear_agree = (0usize, 0usize, 0f64);

    for (at, size) in sizes.iter().enumerate() {
      let px = size * density;
      let sizings: Vec<(Rule, Sizing)> = Rule::ALL
        .into_iter()
        .filter_map(|rule| {
          Sizing::new(data.clone(), index, coords.clone(), px, rule, Some(styles.clone()))
            .map(|sizing| (rule, sizing))
        })
        .collect();

      for glyph in font["glyphs"].as_array().cloned().unwrap_or_default() {
        let id = glyph["glyph"].as_u64().unwrap_or(0) as u32;
        let measured_hinted = numbers(&glyph["hinted"]).get(at).copied().flatten();
        let measured_bounds = numbers(&glyph["bounds"]).get(at).copied().flatten();
        let measured_linear = numbers(&glyph["linear"])
          .get(at)
          .copied()
          .flatten()
          .or_else(|| numbers(&glyph["bounds_linear"]).get(at).copied().flatten());

        for (rule, sizing) in &sizings {
          let (_, predicted) = sizing.advance(id);

          if let Some(bounds) = measured_bounds {
            let entry = per_rule_bounds.entry(rule.name()).or_default();

            entry.1 += 1;

            if (f64::from(predicted) - bounds).abs() < 0.01 {
              entry.0 += 1;
            }
          }

          let Some(hinted) = measured_hinted
          else {
            continue;
          };
          let entry = per_rule.entry(rule.name()).or_default();
          let hit = (f64::from(predicted) - hinted).abs() < 0.01;

          entry.1 += 1;

          if hit {
            entry.0 += 1;
          }
          else if entry.2.len() < 12 {
            entry.2.push(json!({
              "char": glyph["char"], "glyph": id, "px": px, "measured": hinted,
              "predicted": predicted, "linear": measured_linear,
            }));
          }

          if *rule == Rule::Linear {
            if let Some(linear) = measured_linear {
              let delta = (f64::from(predicted) - linear).abs();

              linear_agree.1 += 1;
              linear_agree.2 = linear_agree.2.max(delta);

              if delta < 0.01 {
                linear_agree.0 += 1;
              }
            }
          }
        }
      }
    }

    let mut rules = serde_json::Map::new();

    for (rule, (hits, count, misses)) in &per_rule {
      let total = totals.entry(rule).or_default();

      total.0 += hits;
      total.1 += count;
      rules.insert(
        (*rule).to_owned(),
        json!({"matched": hits, "of": count, "misses": misses}),
      );
    }

    let best = Rule::ALL
      .iter()
      .rev()
      .filter_map(|rule| per_rule.get(rule.name()).map(|(hits, _, _)| (*hits, rule.name())))
      .max_by_key(|(hits, _)| *hits)
      .map(|(_, rule)| rule);
    let bounds: BTreeMap<&str, Value> = per_rule_bounds
      .iter()
      .map(|(rule, (hits, count))| (*rule, json!({"matched": hits, "of": count})))
      .collect();
    let mut pairs = Vec::new();
    let mut kerning_linear = (0usize, 0usize, 0f64);

    for pair in font["pairs"].as_array().cloned().unwrap_or_default() {
      let hinted = numbers(&pair["kern_hinted"]);
      let linear = numbers(&pair["kern_linear"]);

      for (at, (h, l)) in hinted.iter().zip(&linear).enumerate() {
        if let (Some(h), Some(l)) = (h, l) {
          let delta = (h - l).abs();

          kerning_linear.1 += 1;
          kerning_linear.2 = kerning_linear.2.max(delta);

          if delta < 0.01 {
            kerning_linear.0 += 1;
          }

          if at == 9 {
            pairs.push(json!({"text": pair["text"], "kern_hinted": h, "kern_linear": l}));
          }
        }
      }
    }

    let pairs_parley = json!({
      "freetype_floor": pairs_under(Rule::FreeTypeFloor, density, &sizes, &font, &data, index),
      "freetype_floor_whole": pairs_under(Rule::FreeTypeFloorWhole, density, &sizes, &font, &data, index),
      "linear": pairs_under(Rule::Linear, density, &sizes, &font, &data, index),
    });

    for (rule, value) in pairs_parley.as_object().into_iter().flatten() {
      let entry = pair_totals.entry(rule.clone()).or_insert((0, 0));

      entry.0 += value["matched"].as_u64().unwrap_or(0);
      entry.1 += value["of"].as_u64().unwrap_or(0);
    }

    fonts_out.push(json!({
      "name": font["name"], "path": path, "index": index, "axes": font["axes"],
      "pairs_parley": pairs_parley,
      "upem": face.head().map(|head| head.units_per_em()).unwrap_or(0),
      "head_flags": face.head().map(|head| head.flags().bits()).unwrap_or(0),
      "glyf": face.table_data(skrifa::Tag::new(b"glyf")).is_some(),
      "has_instructions": face.outline_glyphs().prefer_interpreter(),
      "linear_agrees": {"matched": linear_agree.0, "of": linear_agree.1, "max_delta": linear_agree.2},
      "rules": rules, "best": best, "rules_against_glyph_bounds": bounds,
      "kerning_linear": {"matched": kerning_linear.0, "of": kerning_linear.1, "max_delta": kerning_linear.2},
      "pairs_at_17dp": pairs, "chws": font["chws"],
    }));
  }

  // Ties go to the earliest rule of `Rule::ALL`.
  let best = Rule::ALL
    .iter()
    .rev()
    .filter_map(|rule| totals.get(rule.name()).map(|(hits, _)| (*hits, rule.name())))
    .max_by_key(|(hits, _)| *hits)
    .map(|(_, rule)| rule);
  let totals: BTreeMap<&str, Value> = totals
    .iter()
    .map(|(rule, (hits, count))| (*rule, json!({"matched": hits, "of": count})))
    .collect();

  // The glyph rule's positioning: whole pixels where Parley's pairs match the platform's better so.
  let positioning = match (pair_totals.get("freetype_floor"), pair_totals.get("freetype_floor_whole")) {
    (Some(fractional), Some(whole)) if whole.1 > 0 && whole.0 > fractional.0 => "freetype_floor_whole",
    _ => "freetype_floor",
  };
  let best_with_kerning = (best == Some("freetype_floor")).then_some(positioning).or(best);
  let pair_totals: BTreeMap<String, Value> = pair_totals
    .into_iter()
    .map(|(rule, (hits, count))| (rule, json!({"matched": hits, "of": count})))
    .collect();

  json!({
    "density": density, "best": best, "best_with_kerning": best_with_kerning, "totals": totals,
    "pairs_parley": pair_totals, "fonts": fonts_out,
  })
}

/// Which advance rule the measurements (`advanceProbe`'s fonts, measured by the platform at
/// each size) follow, as JSON.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_advanceAnalysis<'local>(
  mut env: JNIEnv<'local>,
  _class: JClass<'local>,
  measured: JString<'local>,
) -> jstring {
  let measured = string(&mut env, &measured);
  let result = serde_json::from_str::<Value>(&measured)
    .map(|measured| analyze(&measured).to_string())
    .unwrap_or_else(|error| json!({"error": error.to_string()}).to_string());

  answer(&mut env, &result)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fixed_point() {
    assert_eq!(mul_fix(1000, 65536), 1000);
    assert_eq!(div_fix(64, 1), 64 << 16);
  }

  /// Runs where `SKENE_TEST_FONT` names a font file: every rule answers for glyph 1 at 17 px.
  #[test]
  fn rules_answer() {
    let Some(data) = std::env::var_os("SKENE_TEST_FONT").and_then(|path| std::fs::read(path).ok())
    else {
      return;
    };
    let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(data);

    for rule in Rule::ALL {
      let sizing = Sizing::new(data.clone(), 0, Vec::new(), 44.625, rule, None).expect("sizing");
      let (units, px) = sizing.advance(36);

      eprintln!("{} {units} {px}", rule.name());
      assert!(units > 0.);
    }
  }

  /// Runs where `SKENE_TEST_FONT` names a font file without kerning between `l` and `i`: with
  /// the round rule set, every glyph Parley lays out advances by whole device pixels.
  #[test]
  fn parley_takes_the_rule() {
    use parley::{
      fontique::Blob,
      FontContext,
      FontFamily,
      LayoutContext,
      StyleProperty,
    };

    let Some(data) = std::env::var_os("SKENE_TEST_FONT").and_then(|path| std::fs::read(path).ok())
    else {
      return;
    };
    let mut fonts = FontContext::default();
    let families = fonts.collection.register_fonts(Blob::new(Arc::new(data)), None);
    let name = fonts.collection.family_name(families[0].0).unwrap().to_owned();
    let mut layouts: LayoutContext<()> = LayoutContext::new();
    let density = 2.625;
    let advances = |layouts: &mut LayoutContext<()>, fonts: &mut FontContext| {
      let mut builder = layouts.ranged_builder(fonts, "lilili", 1., false);

      builder.push_default(StyleProperty::FontFamily(FontFamily::named(&name)));
      builder.push_default(StyleProperty::FontSize(17.));

      let mut layout = builder.build("lilili");

      layout.break_all_lines(None);

      let mut out = Vec::new();

      for line in layout.lines() {
        for item in line.items() {
          if let parley::PositionedLayoutItem::GlyphRun(run) = item {
            out.extend(run.glyphs().map(|glyph| glyph.advance * density));
          }
        }
      }

      out
    };

    parley_engine::set_advance_rule(Some(Arc::new(HintedAdvances::new(Rule::Round, density))));

    let rounded = advances(&mut layouts, &mut fonts);

    parley_engine::set_advance_rule(None);

    let linear = advances(&mut layouts, &mut fonts);

    eprintln!("rounded {rounded:?}\nlinear {linear:?}");
    assert!(rounded.iter().all(|px| (px - px.round()).abs() < 1e-3));
    assert!(linear.iter().any(|px| (px - px.round()).abs() > 1e-3));
  }
}
