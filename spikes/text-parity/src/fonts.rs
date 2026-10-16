//! The fonts Parley shapes with: fontique's system backend, or fonts named explicitly.
//!
//! fontique has a backend per platform (CoreText, DirectWrite, fontconfig, and on Android a
//! reader of `/system/etc/fonts.xml`). Where a platform's list is better taken from the
//! platform itself (Android's `ASystemFontIterator`, which also sees updatable and OEM fonts), a
//! [`FontSource`] names the files, the generic families and the per-script fallbacks, and the
//! collection holds exactly those.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
  time::Instant,
};

use parley::{
  FontContext,
  FontData,
  fontique::{
    Blob,
    Collection,
    CollectionOptions,
    FallbackKey,
    FamilyId,
    FontInfoOverride,
    FontStyle,
    FontWeight,
    GenericFamily,
    Language,
    Script,
    SourceCache,
    SourceKind,
  },
};
use serde::{Deserialize, Serialize};
use skrifa::{FontRef, MetadataProvider as _, Tag, string::StringId};

use crate::record::{FontIdentity, Variation};

/// One font file to register.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontFile {
  /// The file.
  pub path: PathBuf,
  /// The family to register it under, its own name table's where `None`.
  #[serde(default)]
  pub family: Option<String>,
  /// The weight to register it at, where the platform declares one (a variable font listed
  /// once per weight).
  #[serde(default)]
  pub weight: Option<f32>,
  /// Whether to register it as italic.
  #[serde(default)]
  pub italic: Option<bool>,
  /// The variation axes' default values to register it with.
  #[serde(default)]
  pub axes: Vec<Variation>,
}

/// The families a generic family resolves to, first first.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenericFamilies {
  /// The generic family, as CSS names it: `system-ui`, `sans-serif`, `serif`, `monospace`,
  /// `emoji`.
  pub generic: String,
  /// The families.
  pub families: Vec<String>,
}

/// The families a script falls back to, under an optional locale.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fallbacks {
  /// The ISO 15924 script: `Hani`, `Thai`, `Arab`.
  pub script: String,
  /// The BCP 47 locale that narrows it: `ja`, `zh-Hant`.
  #[serde(default)]
  pub locale: Option<String>,
  /// The families, first first.
  pub families: Vec<String>,
}

/// Where the collection's fonts come from.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontSource {
  /// Whether fontique's system backend contributes the platform's fonts.
  #[serde(default = "yes")]
  pub system: bool,
  /// Files registered beside (or instead of) the system's.
  #[serde(default)]
  pub files: Vec<FontFile>,
  /// Generic families, replacing the backend's where named.
  #[serde(default)]
  pub generics: Vec<GenericFamilies>,
  /// Fallbacks, replacing the backend's for each script and locale named.
  #[serde(default)]
  pub fallbacks: Vec<Fallbacks>,
}

const fn yes() -> bool {
  true
}

impl Default for FontSource {
  fn default() -> Self {
    Self::system()
  }
}

impl FontSource {
  /// The platform's fonts, through fontique's backend.
  #[must_use]
  pub const fn system() -> Self {
    Self {
      system: true,
      files: Vec::new(),
      generics: Vec::new(),
      fallbacks: Vec::new(),
    }
  }

  /// A source read from JSON.
  ///
  /// # Errors
  ///
  /// Where the file cannot be read or does not hold a source.
  pub fn read(path: &Path) -> Result<Self, String> {
    let json =
      std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;

    serde_json::from_str(&json).map_err(|error| format!("{}: {error}", path.display()))
  }
}

/// The family and PostScript names in a font's name table: the typographic family where it
/// has one.
#[must_use]
pub fn names_of(data: &[u8], index: u32) -> Option<(String, String)> {
  let font = FontRef::from_index(data, index).ok()?;
  let name = |id: StringId| {
    font
      .localized_strings(id)
      .english_or_first()
      .map(|name| name.to_string())
  };
  let family = name(StringId::TYPOGRAPHIC_FAMILY_NAME).or_else(|| name(StringId::FAMILY_NAME))?;
  let postscript = name(StringId::POSTSCRIPT_NAME).unwrap_or_default();

  Some((family, postscript))
}

/// Names read from font files, cached by file and face.
#[derive(Default)]
pub struct FontNames {
  cache: HashMap<(PathBuf, u32), Option<(String, String)>>,
}

impl FontNames {
  /// The family and PostScript names of face `index` of the file at `path`.
  pub fn of_file(&mut self, path: &Path, index: u32) -> Option<(String, String)> {
    self
      .cache
      .entry((path.to_path_buf(), index))
      .or_insert_with(|| {
        std::fs::read(path)
          .ok()
          .and_then(|data| names_of(&data, index))
      })
      .clone()
  }
}

/// A font collection ready to shape with, and what it knows of each font's identity.
pub struct Fonts {
  /// Parley's font context: the collection and its source cache.
  pub context: FontContext,
  /// How long making the collection took.
  pub startup_ms: f64,
  registered: HashMap<u64, PathBuf>,
  identities: HashMap<(u64, u32), FontIdentity>,
}

impl Fonts {
  /// The collection `source` describes.
  ///
  /// # Errors
  ///
  /// Where a file cannot be read, a generic family or script is unknown, or a family named
  /// for one is not in the collection.
  pub fn new(source: &FontSource) -> Result<Self, String> {
    let started = Instant::now();
    let mut collection = Collection::new(CollectionOptions {
      shared: false,
      system_fonts: source.system,
    });
    let mut registered = HashMap::new();
    let mut blobs: HashMap<PathBuf, Blob<u8>> = HashMap::new();

    if source.system {
      collection.load_system_fonts();
    }

    for file in &source.files {
      let blob = match blobs.get(&file.path) {
        Some(blob) => blob.clone(),
        None => {
          let data = std::fs::read(&file.path)
            .map_err(|error| format!("{}: {error}", file.path.display()))?;
          let blob = Blob::new(Arc::new(data));

          registered.insert(blob.id(), file.path.clone());
          blobs.insert(file.path.clone(), blob.clone());

          blob
        }
      };
      let axes: Vec<(Tag, f32)> = file
        .axes
        .iter()
        .filter_map(|axis| Some((Tag::new_checked(axis.tag.as_bytes()).ok()?, axis.value)))
        .collect();
      let added = collection.register_fonts(
        blob,
        Some(FontInfoOverride {
          family_name: file.family.as_deref(),
          weight: file.weight.map(FontWeight::new),
          style: file.italic.map(|italic| {
            if italic {
              FontStyle::Italic
            }
            else {
              FontStyle::Normal
            }
          }),
          axes: (!axes.is_empty()).then_some(axes.as_slice()),
          ..FontInfoOverride::default()
        }),
      );

      if added.is_empty() {
        return Err(format!("{}: no font in the file", file.path.display()));
      }
    }

    let family_ids = |collection: &mut Collection, names: &[String]| {
      names
        .iter()
        .map(|name| {
          collection
            .family_id(name)
            .ok_or_else(|| format!("family {name:?} is not in the collection"))
        })
        .collect::<Result<Vec<FamilyId>, String>>()
    };

    for generic in &source.generics {
      let family = GenericFamily::parse(&generic.generic)
        .ok_or_else(|| format!("unknown generic family {:?}", generic.generic))?;
      let ids = family_ids(&mut collection, &generic.families)?;

      collection.set_generic_families(family, ids.into_iter());
    }

    for fallback in &source.fallbacks {
      let script = Script::parse(&fallback.script)
        .map_err(|_| format!("unknown script {:?}", fallback.script))?;
      let locale = fallback
        .locale
        .as_deref()
        .map(Language::parse)
        .transpose()
        .map_err(|_| format!("unknown locale {:?}", fallback.locale))?;
      let ids = family_ids(&mut collection, &fallback.families)?;

      collection.set_fallbacks(FallbackKey::new(script, locale.as_ref()), ids.into_iter());
    }

    Ok(Self {
      context: FontContext {
        collection,
        source_cache: SourceCache::default(),
      },
      startup_ms: started.elapsed().as_secs_f64() * 1e3,
      registered,
      identities: HashMap::new(),
    })
  }

  /// The platform's fonts.
  ///
  /// # Panics
  ///
  /// Never: the system source names nothing that can fail.
  #[must_use]
  pub fn system() -> Self {
    Self::new(&FontSource::system()).expect("the system collection")
  }

  /// Who `font` is: its names, its file where it has one, its face and length.
  pub fn identity(&mut self, font: &FontData) -> FontIdentity {
    let key = (font.data.id(), font.index);

    if let Some(identity) = self.identities.get(&key) {
      return identity.clone();
    }

    let (family, postscript) = names_of(font.data.data(), font.index).unwrap_or_default();
    let path = self
      .registered
      .get(&font.data.id())
      .cloned()
      .or_else(|| self.system_path(font, &family));
    let identity = FontIdentity {
      family,
      postscript,
      path: path.map(|path| path.display().to_string()),
      index: font.index,
      data_len: font.data.data().len(),
    };

    self.identities.insert(key, identity.clone());

    identity
  }

  /// The file a system font was loaded from: the family's fonts searched for the one whose
  /// source the cache holds as `font`'s data.
  fn system_path(&mut self, font: &FontData, family: &str) -> Option<PathBuf> {
    let mut names = vec![family.to_owned()];

    if let Some((_, legacy)) = FontRef::from_index(font.data.data(), font.index)
      .ok()
      .and_then(|font| {
        font
          .localized_strings(StringId::FAMILY_NAME)
          .english_or_first()
          .map(|name| ((), name.to_string()))
      })
    {
      names.push(legacy);
    }

    let context = &mut self.context;

    for name in names {
      let Some(info) = context.collection.family_by_name(&name)
      else {
        continue;
      };

      for candidate in info.fonts() {
        if candidate.index() != font.index {
          continue;
        }

        let SourceKind::Path(path) = candidate.source().kind()
        else {
          continue;
        };
        let path = path.to_path_buf();

        if let Some(blob) = candidate.load(Some(&mut context.source_cache))
          && blob.id() == font.data.id()
        {
          return Some(path);
        }
      }
    }

    None
  }
}
