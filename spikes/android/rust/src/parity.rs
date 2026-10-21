//! The text-parity side of the spike: the corpus, the system's fonts as the NDK and fontique see
//! them, a font source fed from `AFontMatcher`, Parley's runs through the shared harness, the
//! plan a platform rasterizer draws Parley's glyph runs from (option C), and Skene's port of the
//! platform's nonlinear font scale.

use std::{
  collections::{BTreeMap, HashMap},
  path::{Path, PathBuf},
  time::Instant,
};

use jni::{
  errors::ThrowRuntimeExAndDefault,
  objects::{JClass, JString},
  sys::{jboolean, jdouble, jfloat, jint},
  Env,
  EnvUnowned,
};
use parley::fontique::{
  Collection,
  CollectionOptions,
  FallbackKey,
  GenericFamily,
  Language,
  Script,
};
use serde::Serialize;
use serde_json::json;
use skrifa::{raw::types::F2Dot14, FontRef, MetadataProvider as _, Tag};
use text_parity::{
  corpus::Corpus,
  fonts::{names_of, Fallbacks, FontFile, FontSource, GenericFamilies},
  record::{RecordSet, Variation},
};

use crate::ndk::{self, FontInfo, Matcher, VARIANT_COMPACT};

/// The scripts the fed source names a fallback for: the script, a sample, the locale list the
/// matcher is asked with, and the locale the fallback is keyed by (Han per locale).
pub const FALLBACKS: &[(&str, &str, &str, Option<&str>)] = &[
  ("Hani", "今", "ja-JP", Some("ja")),
  ("Hani", "今", "zh-Hans-CN", Some("zh-Hans")),
  ("Hani", "今", "zh-Hant-TW", Some("zh-Hant")),
  ("Hani", "今", "ko-KR", Some("ko")),
  ("Hani", "今", "en-US", None),
  ("Hira", "あ", "ja-JP", None),
  ("Kana", "ア", "ja-JP", None),
  ("Hang", "가", "ko-KR", None),
  ("Bopo", "ㄅ", "zh-Hant-TW", None),
  ("Thai", "ก", "th-TH", None),
  ("Khmr", "ក", "km-KH", None),
  ("Laoo", "ກ", "lo-LA", None),
  ("Mymr", "က", "my-MM", None),
  ("Deva", "क", "hi-IN", None),
  ("Beng", "ক", "bn-BD", None),
  ("Taml", "த", "ta-IN", None),
  ("Arab", "ب", "ar", None),
  ("Hebr", "א", "he", None),
  ("Grek", "α", "el", None),
  ("Cyrl", "д", "ru", None),
  ("Ethi", "ሀ", "am", None),
  ("Geor", "ა", "ka", None),
  ("Armn", "ա", "hy", None),
  ("Sinh", "ක", "si", None),
  ("Guru", "ਕ", "pa", None),
  ("Gujr", "ક", "gu", None),
  ("Orya", "କ", "or", None),
  ("Telu", "క", "te", None),
  ("Knda", "ಕ", "kn", None),
  ("Mlym", "ക", "ml", None),
  ("Tibt", "ཀ", "bo", None),
];

/// The emoji samples whose fonts make up the emoji generic family, first first.
pub const EMOJI_SAMPLES: &[&str] = &[
  "😀",
  "🇯🇵",
  "1\u{FE0F}\u{20E3}",
  "🫨",
  "🙂\u{200D}↔\u{FE0F}",
  "🪉",
];

fn string(env: &Env, value: &JString) -> String {
  value.try_to_string(env).unwrap_or_default()
}

fn answer<'local>(env: &mut Env<'local>, text: &str) -> JString<'local> {
  JString::new(env, text).unwrap_or_default()
}

/// The resident set size of this process in kilobytes.
pub fn rss_kb() -> u64 {
  std::fs::read_to_string("/proc/self/status")
    .ok()
    .and_then(|status| {
      status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
    })
    .unwrap_or(0)
}

/// The corpus with every variant spelled out.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_corpus<'local>(
  mut env: EnvUnowned<'local>,
  _class: JClass<'local>,
) -> JString<'local> {
  env
    .with_env(|env| -> jni::errors::Result<_> {
      let json = serde_json::to_string(&Corpus::builtin().expanded()).unwrap_or_default();

      Ok(answer(env, &json))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Every font `ASystemFontIterator` lists, with the time it took and per-directory counts.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_systemFonts<'local>(
  mut env: EnvUnowned<'local>,
  _class: JClass<'local>,
) -> JString<'local> {
  env
    .with_env(|env| -> jni::errors::Result<_> {
      let (fonts, ms) = ndk::system_fonts();
      let mut directories: BTreeMap<String, usize> = BTreeMap::new();

      for font in &fonts {
        let directory = Path::new(&font.path)
          .parent()
          .map(|parent| parent.display().to_string())
          .unwrap_or_default();

        *directories.entry(directory).or_default() += 1;
      }

      let json =
        json!({"ms": ms, "count": fonts.len(), "directories": directories, "fonts": fonts});

      Ok(answer(env, &json.to_string()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The runs `AFontMatcher` makes of `text` in `family`, `locales` a comma-separated list.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_matchRuns<'local>(
  mut env: EnvUnowned<'local>,
  _class: JClass<'local>,
  family: JString<'local>,
  text: JString<'local>,
  locales: JString<'local>,
  weight: jint,
  italic: jboolean,
) -> JString<'local> {
  env
    .with_env(|env| -> jni::errors::Result<_> {
      let family = string(env, &family);
      let text = string(env, &text);
      let locales = string(env, &locales);
      let matcher = Matcher::new(weight as u16, italic, &locales, VARIANT_COMPACT);
      let runs = matcher.runs(&family, &text);

      Ok(answer(
        env,
        &serde_json::to_string(&runs).unwrap_or_default(),
      ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Nanoseconds per `AFontMatcher_match` call for `text`, over `count` calls.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_matchBench<'local>(
  mut env: EnvUnowned<'local>,
  _class: JClass<'local>,
  text: JString<'local>,
  locales: JString<'local>,
  count: jint,
) -> jdouble {
  env
    .with_env(|env| -> jni::errors::Result<_> {
      let text = string(env, &text);
      let locales = string(env, &locales);
      let matcher = Matcher::new(400, false, &locales, VARIANT_COMPACT);

      Ok(matcher.bench("sans-serif", &text, count as usize))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The family and PostScript names of face `index` of the font file at `path`, a newline
/// between them; empty where the file cannot be read.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_fontNames<'local>(
  mut env: EnvUnowned<'local>,
  _class: JClass<'local>,
  path: JString<'local>,
  index: jint,
) -> JString<'local> {
  env
    .with_env(|env| -> jni::errors::Result<_> {
      let path = string(env, &path);
      let names = std::fs::read(&path)
        .ok()
        .and_then(|data| names_of(&data, index as u32))
        .map(|(family, postscript)| format!("{family}\n{postscript}"))
        .unwrap_or_default();

      Ok(answer(env, &names))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// A font source built from `AFontMatcher`'s answers: the generic families and a fallback per
/// script (Han per locale), each the file Minikin picks for a sample.
pub struct Fed {
  /// The source.
  pub source: FontSource,
  /// What each question was answered with.
  pub answers: Vec<serde_json::Value>,
  /// How long the questions took.
  pub ms: f64,
}

#[derive(Default)]
struct Registry {
  files: Vec<FontFile>,
  registered: HashMap<String, usize>,
  owners: HashMap<(String, u16, bool), String>,
  names: HashMap<(String, u32), Option<(String, String)>>,
  renamed: HashMap<(String, u32), String>,
}

impl Registry {
  fn names(&mut self, path: &str, index: u32) -> Option<(String, String)> {
    self
      .names
      .entry((path.to_owned(), index))
      .or_insert_with(|| {
        std::fs::read(path)
          .ok()
          .and_then(|data| names_of(&data, index))
      })
      .clone()
  }

  /// Registers `font` and answers the family it is registered under.
  fn add(&mut self, font: &FontInfo) -> Option<String> {
    let (family, _) = self.names(&font.path, font.index)?;
    let face = (font.path.clone(), font.index);

    if let Some(renamed) = self.renamed.get(&face) {
      return Some(renamed.clone());
    }

    let owner = (family.to_lowercase(), font.weight, font.italic);
    let id = format!("{}#{}", font.path, font.index);
    let family = match self.owners.get(&owner) {
      Some(other) if *other != id => {
        let stem = Path::new(&font.path)
          .file_stem()
          .map(|stem| stem.to_string_lossy().into_owned())
          .unwrap_or_default();
        let renamed = format!("{family} ({stem} {})", font.index);

        self.renamed.insert(face, renamed.clone());

        renamed
      }
      _ => {
        self.owners.insert(owner, id);

        family
      }
    };
    let renamed = self.renamed.contains_key(&(font.path.clone(), font.index));
    let key = format!(
      "{}@{}{}[{:?}]{}",
      font.path,
      font.weight,
      if font.italic { "i" } else { "" },
      font.axes,
      if renamed {
        family.clone()
      }
      else {
        String::new()
      }
    );

    if !self.registered.contains_key(&key) {
      self.registered.insert(key, self.files.len());
      self.files.push(FontFile {
        path: PathBuf::from(&font.path),
        family: renamed.then(|| family.clone()),
        weight: Some(f32::from(font.weight)),
        italic: Some(font.italic),
        axes: font
          .axes
          .iter()
          .map(|(tag, value)| Variation {
            tag: tag.clone(),
            value: *value,
          })
          .collect(),
      });
    }

    Some(family)
  }
}

impl Fed {
  /// Asks the matcher every question and builds the source.
  pub fn build() -> Self {
    let started = Instant::now();
    let mut registry = Registry::default();
    let mut answers = Vec::new();
    let mut generics = Vec::new();
    let mut ask = |registry: &mut Registry,
                   family: &str,
                   sample: &str,
                   locales: &str,
                   weight: u16,
                   italic: bool| {
      let matcher = Matcher::new(weight, italic, locales, VARIANT_COMPACT);
      let units: Vec<u16> = sample.encode_utf16().collect();
      let found = matcher.first(family, &units);
      let name = found.as_ref().and_then(|(font, _)| registry.add(font));

      answers.push(json!({
        "family": family, "sample": sample, "locales": locales, "weight": weight, "italic": italic,
        "font": found.as_ref().map(|(font, _)| font), "registered_as": name,
      }));

      name
    };
    let push_unique = |list: &mut Vec<String>, name: Option<String>| {
      if let Some(name) = name {
        if !list.contains(&name) {
          list.push(name);
        }
      }
    };

    for (generic, family) in [
      ("system-ui", "sans-serif"),
      ("sans-serif", "sans-serif"),
      ("serif", "serif"),
      ("monospace", "monospace"),
    ] {
      let mut families = Vec::new();
      let weights: &[u16] = if family == "monospace" {
        &[400]
      }
      else {
        &[400, 100, 200, 300, 500, 600, 700, 800, 900]
      };

      for italic in [false, true] {
        for weight in weights {
          let name = ask(&mut registry, family, "a", "en-US", *weight, italic);

          if *weight == 400 && !italic {
            push_unique(&mut families, name);
          }
        }
      }

      generics.push(GenericFamilies {
        generic: generic.to_owned(),
        families,
      });
    }

    let mut emoji = Vec::new();

    for sample in EMOJI_SAMPLES {
      let name = ask(&mut registry, "sans-serif", sample, "en-US", 400, false);

      push_unique(&mut emoji, name);
    }

    generics.push(GenericFamilies {
      generic: "emoji".to_owned(),
      families: emoji,
    });

    let mut fallbacks = Vec::new();

    for (script, sample, locales, locale) in FALLBACKS {
      let mut families = Vec::new();

      for (weight, italic) in [(400, false), (700, false), (400, true)] {
        let name = ask(&mut registry, "sans-serif", sample, locales, weight, italic);

        if weight == 400 && !italic {
          push_unique(&mut families, name);
        }
      }

      if !families.is_empty() {
        fallbacks.push(Fallbacks {
          script: (*script).to_owned(),
          locale: locale.map(str::to_owned),
          families,
        });
      }
    }

    Self {
      source: FontSource {
        system: false,
        files: registry.files,
        generics,
        fallbacks,
      },
      answers,
      ms: started.elapsed().as_secs_f64() * 1e3,
    }
  }
}

/// The font source fed from `AFontMatcher`, with each question's answer and the time taken.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_fedSource<'local>(
  mut env: EnvUnowned<'local>,
  _class: JClass<'local>,
) -> JString<'local> {
  env
    .with_env(|env| -> jni::errors::Result<_> {
      let fed = Fed::build();
      let json = json!({"ms": fed.ms, "source": fed.source, "answers": fed.answers});

      Ok(answer(env, &json.to_string()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// fontique's own Android collection (`/system/fonts` scanned, `fonts.xml` read): how long it
/// takes, what it costs in memory, its families, generic families and per-script fallbacks.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_stockCollection<'local>(
  mut env: EnvUnowned<'local>,
  _class: JClass<'local>,
) -> JString<'local> {
  env
    .with_env(|env| -> jni::errors::Result<_> {
      let before = rss_kb();
      let started = Instant::now();
      let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: true,
      });
      let families: Vec<String> = collection.family_names().map(str::to_owned).collect();
      let ms = started.elapsed().as_secs_f64() * 1e3;
      let after = rss_kb();
      let mut generics = BTreeMap::new();

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

        generics.insert(name, names);
      }

      let mut fallbacks = Vec::new();

      for (script, _, _, locale) in FALLBACKS {
        let language = locale.and_then(|locale| Language::parse(locale).ok());
        let Ok(parsed) = Script::parse(script)
        else {
          continue;
        };
        let key = FallbackKey::new(parsed, language.as_ref());
        let ids: Vec<_> = collection.fallback_families(key).collect();
        let names: Vec<String> = ids
          .iter()
          .filter_map(|id| collection.family_name(*id).map(str::to_owned))
          .collect();

        fallbacks.push(json!({"script": script, "locale": locale, "families": names}));
      }

      let mut named = BTreeMap::new();

      for name in [
        "sans-serif",
        "serif",
        "monospace",
        "Roboto",
        "Roboto Flex",
        "Noto Sans",
        "Noto Serif",
        "Noto Color Emoji",
        "Noto Sans CJK JP",
        "Noto Sans Thai",
        "Noto Sans Thai UI",
      ] {
        named.insert(
          name,
          collection
            .family_by_name(name)
            .map(|family| family.fonts().len()),
        );
      }

      let json = json!({
        "ms": ms, "rss_before_kb": before, "rss_after_kb": after, "families": families.len(),
        "family_names": families, "generics": generics, "fallbacks": fallbacks, "named_fonts": named,
      });

      Ok(answer(env, &json.to_string()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// One Parley run through the harness, configured as `run::parley_json` takes it: `ok: ` and the
/// summary, or `error: ` and why.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_parley<'local>(
  mut env: EnvUnowned<'local>,
  _class: JClass<'local>,
  config: JString<'local>,
) -> JString<'local> {
  env
    .with_env(|env| -> jni::errors::Result<_> {
      let config = string(env, &config);
      let result = match text_parity::run::parley_json(&config) {
        Ok(summary) => format!("ok: {summary}"),
        Err(error) => format!("error: {error}"),
      };

      Ok(answer(env, &result))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// A glyph run as a platform rasterizer draws it: the font, its axes in user space, and the
/// glyphs, with the run's text where it draws text (not a hyphen or an ellipsis Parley's
/// caller added).
#[derive(Serialize)]
struct PlanRun {
  line: usize,
  path: Option<String>,
  index: u32,
  size: f32,
  axes: Vec<(String, f32)>,
  embolden: bool,
  skew: Option<f32>,
  rtl: bool,
  text: Option<[usize; 2]>,
  left: f32,
  advance: f32,
  baseline: f32,
  glyphs: Vec<u32>,
  xs: Vec<f32>,
  ys: Vec<f32>,
}

#[derive(Serialize)]
struct PlanVariant {
  variant: String,
  stem: String,
  locale: String,
  width: f32,
  height: f32,
  images: BTreeMap<String, String>,
  runs: Vec<PlanRun>,
}

/// The user-space value of each axis whose normalized coordinate (F2Dot14 bits) is `coords`,
/// found by bisection through the font's own normalization (avar included).
fn user_axes(data: &[u8], index: u32, coords: &[i16]) -> Vec<(String, f32)> {
  let Ok(font) = FontRef::from_index(data, index)
  else {
    return Vec::new();
  };
  let axes = font.axes();
  let mut out = Vec::new();

  for (at, axis) in axes.iter().enumerate() {
    let Some(target) = coords.get(at).copied()
    else {
      break;
    };
    let tag = axis.tag();
    let normalized = |value: f32| -> i16 {
      let location = axes.location([(tag, value)]);

      location
        .coords()
        .get(at)
        .map_or(0, |coord: &F2Dot14| coord.to_bits())
    };
    let (mut low, mut high) = (axis.min_value(), axis.max_value());

    for _ in 0..40 {
      let middle = (low + high) / 2.;

      if normalized(middle) < target {
        low = middle;
      }
      else {
        high = middle;
      }
    }

    out.push((tag_text(tag), ((low + high) / 2. * 100.).round() / 100.));
  }

  out
}

fn tag_text(tag: Tag) -> String {
  String::from_utf8_lossy(&tag.to_be_bytes()).into_owned()
}

/// Writes the plan for option C from Parley's record set at `records` (made with `glyphs`) to
/// `out`: every variant's glyph runs with their fonts' axes in user space.
fn plan(records: &Path, out: &Path) -> Result<usize, String> {
  let set = RecordSet::read(records)?;
  let corpus = Corpus::builtin();
  let mut files: HashMap<String, Option<Vec<u8>>> = HashMap::new();
  let mut variants = Vec::new();

  for record in &set.records {
    let Some(variant) = corpus.variant(&record.variant)
    else {
      continue;
    };
    let mut runs = Vec::new();

    for (at, glyph_run) in record.glyph_runs.iter().enumerate() {
      let text_run = record.runs.get(at);
      let clusters: Vec<_> = text_run
        .map(|run| {
          let range = run.range.utf8.unwrap_or_default();

          record
            .clusters
            .iter()
            .filter(|cluster| {
              let inner = cluster.range.utf8.unwrap_or_default();

              cluster.line == run.line && inner[0] >= range[0] && inner[1] <= range[1]
            })
            .collect()
        })
        .unwrap_or_default();
      let left = clusters
        .first()
        .map(|cluster| cluster.x)
        .or_else(|| glyph_run.glyphs.first().map(|glyph| glyph.x))
        .unwrap_or(0.);
      let advance = clusters.iter().map(|cluster| cluster.advance).sum();
      let axes = match &glyph_run.font.path {
        Some(path) if !glyph_run.coords.is_empty() => {
          let data = files
            .entry(path.clone())
            .or_insert_with(|| std::fs::read(path).ok());

          data
            .as_deref()
            .map(|data| user_axes(data, glyph_run.font.index, &glyph_run.coords))
            .unwrap_or_default()
        }
        _ => glyph_run
          .variations
          .iter()
          .map(|variation| (variation.tag.clone(), variation.value))
          .collect(),
      };

      runs.push(PlanRun {
        line: glyph_run.line,
        path: glyph_run.font.path.clone(),
        index: glyph_run.font.index,
        size: glyph_run.size,
        axes,
        embolden: glyph_run.embolden,
        skew: glyph_run.skew,
        rtl: text_run.is_some_and(|run| run.rtl),
        text: text_run.and_then(|run| run.range.utf16),
        left,
        advance,
        baseline: glyph_run.glyphs.first().map_or(0., |glyph| glyph.y),
        glyphs: glyph_run.glyphs.iter().map(|glyph| glyph.id).collect(),
        xs: glyph_run.glyphs.iter().map(|glyph| glyph.x).collect(),
        ys: glyph_run.glyphs.iter().map(|glyph| glyph.y).collect(),
      });
    }

    let (width, height) = record.image_box(variant.width);

    variants.push(PlanVariant {
      variant: record.variant.clone(),
      stem: variant.file_stem(),
      locale: variant.case.locale.clone(),
      width,
      height,
      images: record.images.clone(),
      runs,
    });
  }

  let count = variants.len();
  let json = serde_json::to_string(&json!({"scales": set.scales, "variants": variants}))
    .map_err(|error| error.to_string())?;

  std::fs::write(out, json).map_err(|error| format!("{}: {error}", out.display()))?;

  Ok(count)
}

/// Writes the option C plan from `records` to `out`: `ok: ` and the variant count, or `error: `.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_plan<'local>(
  mut env: EnvUnowned<'local>,
  _class: JClass<'local>,
  records: JString<'local>,
  out: JString<'local>,
) -> JString<'local> {
  env
    .with_env(|env| -> jni::errors::Result<_> {
      let records = string(env, &records);
      let out = string(env, &out);
      let result = match plan(Path::new(&records), Path::new(&out)) {
        Ok(count) => format!("ok: {count}"),
        Err(error) => format!("error: {error}"),
      };

      Ok(answer(env, &result))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

const SP: [f32; 9] = [8., 10., 12., 14., 18., 20., 24., 30., 100.];

/// Android 14's nonlinear font-scale tables (`FontScaleConverterFactory`), dp per sp of [`SP`].
const TABLES: [(f32, [f32; 9]); 5] = [
  (1.15, [9.2, 11.5, 13.8, 16.4, 19.8, 21.8, 25.2, 30., 100.]),
  (1.3, [10.4, 13., 15.6, 18.8, 21.6, 23.6, 26.4, 30., 100.]),
  (1.5, [12., 15., 18., 22., 24., 26., 28., 30., 100.]),
  (1.8, [14.4, 18., 21.6, 24.4, 27.6, 30.8, 32.8, 34.8, 100.]),
  (2., [16., 20., 24., 26., 30., 34., 36., 38., 100.]),
];

fn lerp(start: f32, end: f32, at: f32) -> f32 {
  start + (end - start) * at
}

fn constrained_map(
  range_min: f32,
  range_max: f32,
  value_min: f32,
  value_max: f32,
  value: f32,
) -> f32 {
  let at = ((value - value_min) / (value_max - value_min)).clamp(0., 1.);

  lerp(range_min, range_max, at)
}

fn lookup(sp: f32, from: &[f32; 9], to: &[f32; 9]) -> f32 {
  let positive = sp.abs();
  let sign = sp.signum();

  if let Some(index) = from.iter().position(|value| *value == positive) {
    return sign * to[index];
  }

  let lower = from.iter().rposition(|value| *value < positive);

  match lower {
    Some(lower) if lower >= from.len() - 1 => sp * (to[from.len() - 1] / from[from.len() - 1]),
    None => sign * constrained_map(0., to[0], 0., from[0], positive),
    Some(lower) => {
      sign
        * constrained_map(
          to[lower],
          to[lower + 1],
          from[lower],
          from[lower + 1],
          positive,
        )
    }
  }
}

fn key(scale: f32) -> i32 {
  (scale * 100.) as i32
}

/// The dp a size of `sp` takes at `scale` on API level `sdk`, as Skene's resolver computes it:
/// linear before Android 14, and from it Android 14's curve (`FontScaleConverterFactory.forScale`
/// and `FontScaleConverter.convertSpToDp`) ported.
pub fn sp_to_dp(sp: f32, scale: f32, sdk: i32) -> f32 {
  if sdk < 34 || scale < TABLES[0].0 - 0.02 {
    return sp * scale;
  }

  if let Some((_, table)) = TABLES.iter().find(|(at, _)| key(*at) == key(scale)) {
    return lookup(sp, &SP, table);
  }

  let higher = TABLES.iter().position(|(at, _)| key(*at) > key(scale));

  match higher {
    Some(higher) if higher > 0 => {
      let (start, low) = TABLES[higher - 1];
      let (end, high) = TABLES[higher];
      let start = key(start) as f32 / 100.;
      let end = key(end) as f32 / 100.;
      let at = constrained_map(0., 1., start, end, scale);
      let mut table = [0.; 9];

      for (index, size) in SP.iter().enumerate() {
        table[index] = lerp(lookup(*size, &SP, &low), lookup(*size, &SP, &high), at);
      }

      lookup(sp, &SP, &table)
    }
    _ => sp * scale,
  }
}

/// Skene's port of the platform's font scale: dp for `sp` at `scale` on API level `sdk`.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_Parity_spToDp<'local>(
  _env: EnvUnowned<'local>,
  _class: JClass<'local>,
  sp: jfloat,
  scale: jfloat,
  sdk: jint,
) -> jfloat {
  sp_to_dp(sp, scale, sdk)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Runs where `SKENE_VARIABLE_FONT` names a variable font with `wght` and `opsz` axes
  /// (Roboto Flex).
  #[test]
  fn axes_round_trip() {
    let Some(data) =
      std::env::var_os("SKENE_VARIABLE_FONT").and_then(|path| std::fs::read(path).ok())
    else {
      return;
    };
    let font = FontRef::new(&data).unwrap();
    let axes = font.axes();
    let location = axes.location([(Tag::new(b"wght"), 700.), (Tag::new(b"opsz"), 20.)]);
    let coords: Vec<i16> = location
      .coords()
      .iter()
      .map(|coord| coord.to_bits())
      .collect();
    let user = user_axes(&data, 0, &coords);

    eprintln!("{user:?}");
    assert!(user
      .iter()
      .any(|(tag, value)| tag == "wght" && (value - 700.).abs() < 0.5));
    assert!(user
      .iter()
      .any(|(tag, value)| tag == "opsz" && (value - 20.).abs() < 0.5));
  }

  #[test]
  fn plans_glyph_runs() {
    let dir = std::env::temp_dir().join(format!("skene-plan-{}", std::process::id()));
    let run = text_parity::run::ParleyRun {
      out: dir.clone(),
      scales: vec![2.],
      glyphs: true,
      labels: 0,
      repeats: 1,
      only: Some("weight-bold/17/max".to_owned()),
      ..text_parity::run::ParleyRun::default()
    };

    text_parity::run::parley(&run).expect("parley");

    let out = dir.join("plan.json");
    let count = plan(&dir.join("records.json"), &out).expect("plan");
    let plan: serde_json::Value =
      serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let runs = plan["variants"][0]["runs"].as_array().unwrap();

    assert_eq!(count, 1);
    assert!(!runs.is_empty());
    assert!(runs[0]["text"].is_array(), "a text run carries its range");
    assert!(runs[0]["path"].is_string(), "{:?}", runs[0]);
    assert!(runs[0]["advance"].as_f64().unwrap() > 10.);
    eprintln!("{}", serde_json::to_string(&runs[0]["axes"]).unwrap());
    std::fs::remove_dir_all(dir).ok();
  }

  #[test]
  fn curve() {
    assert_eq!(sp_to_dp(14., 1., 34), 14.);
    assert!(
      (sp_to_dp(17., 1.3, 29) - 22.1).abs() < 1e-4,
      "linear before Android 14"
    );
    assert_eq!(sp_to_dp(14., 2., 34), 26.);
    assert_eq!(sp_to_dp(16., 2., 34), 28.);
    assert_eq!(sp_to_dp(200., 2., 34), 200.);
    assert!((sp_to_dp(4., 2., 34) - 8.).abs() < 1e-5);
    assert!((sp_to_dp(17., 1.3, 34) - 20.9).abs() < 1e-4);
    assert!((sp_to_dp(17., 1.4, 34) - lerp(20.9, 23.5, (1.4 - 1.3) / (1.5 - 1.3))).abs() < 1e-3);
  }
}
