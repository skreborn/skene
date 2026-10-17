//! fontconfig, the system's font configuration: per-character fallback in the order the
//! user's configuration sorts the fonts, and the rendering settings it gives each font.
//!
//! The bindings are declared here and linked against the system's `libfontconfig`; no build
//! script runs.

use std::{
  collections::HashMap,
  ffi::{CStr, CString, c_char, c_int},
  os::unix::ffi::OsStrExt as _,
  path::Path,
  ptr,
  rc::Rc,
};

#[repr(C)]
struct FcConfig {
  _opaque: [u8; 0],
}

#[repr(C)]
struct FcPattern {
  _opaque: [u8; 0],
}

#[repr(C)]
struct FcCharSet {
  _opaque: [u8; 0],
}

#[repr(C)]
struct FcFontSet {
  nfont: c_int,
  sfont: c_int,
  fonts: *mut *mut FcPattern,
}

/// `FcResultMatch`.
const MATCH: c_int = 0;

/// `FcMatchPattern`.
const MATCH_PATTERN: c_int = 0;

/// `FC_SLANT_ROMAN`.
const SLANT_ROMAN: c_int = 0;

/// `FC_SLANT_ITALIC`.
const SLANT_ITALIC: c_int = 100;

#[link(name = "fontconfig")]
unsafe extern "C" {
  fn FcInitLoadConfigAndFonts() -> *mut FcConfig;
  fn FcConfigCreate() -> *mut FcConfig;
  fn FcConfigParseAndLoad(config: *mut FcConfig, file: *const u8, complain: c_int) -> c_int;
  fn FcConfigBuildFonts(config: *mut FcConfig) -> c_int;
  fn FcConfigDestroy(config: *mut FcConfig);
  fn FcPatternCreate() -> *mut FcPattern;
  fn FcPatternDestroy(pattern: *mut FcPattern);
  fn FcPatternAddString(pattern: *mut FcPattern, object: *const c_char, value: *const u8) -> c_int;
  fn FcPatternAddInteger(pattern: *mut FcPattern, object: *const c_char, value: c_int) -> c_int;
  fn FcPatternAddBool(pattern: *mut FcPattern, object: *const c_char, value: c_int) -> c_int;
  fn FcPatternAddDouble(pattern: *mut FcPattern, object: *const c_char, value: f64) -> c_int;
  fn FcPatternGetString(
    pattern: *const FcPattern,
    object: *const c_char,
    id: c_int,
    value: *mut *mut u8,
  ) -> c_int;
  fn FcPatternGetInteger(
    pattern: *const FcPattern,
    object: *const c_char,
    id: c_int,
    value: *mut c_int,
  ) -> c_int;
  fn FcPatternGetBool(
    pattern: *const FcPattern,
    object: *const c_char,
    id: c_int,
    value: *mut c_int,
  ) -> c_int;
  fn FcPatternGetCharSet(
    pattern: *const FcPattern,
    object: *const c_char,
    id: c_int,
    value: *mut *mut FcCharSet,
  ) -> c_int;
  fn FcCharSetHasChar(charset: *const FcCharSet, ucs4: u32) -> c_int;
  fn FcConfigSubstitute(config: *mut FcConfig, pattern: *mut FcPattern, kind: c_int) -> c_int;
  fn FcDefaultSubstitute(pattern: *mut FcPattern);
  fn FcFontSort(
    config: *mut FcConfig,
    pattern: *mut FcPattern,
    trim: c_int,
    charsets: *mut *mut FcCharSet,
    result: *mut c_int,
  ) -> *mut FcFontSet;
  fn FcFontRenderPrepare(
    config: *mut FcConfig,
    pattern: *mut FcPattern,
    font: *mut FcPattern,
  ) -> *mut FcPattern;
  fn FcFontSetDestroy(set: *mut FcFontSet);
  fn FcWeightFromOpenType(weight: c_int) -> c_int;
}

/// A fontconfig object's name.
fn object(name: &'static [u8]) -> *const c_char {
  CStr::from_bytes_with_nul(name)
    .expect("object names end in a nul")
    .as_ptr()
}

/// How a font is rasterized, as fontconfig's configuration (and the desktop's settings
/// beneath it) give it for that font.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RenderSettings {
  /// Whether edges are antialiased (grayscale), else drawn in whole pixels.
  pub antialias: bool,
  /// Whether outlines are hinted.
  pub hinting: bool,
  /// How strongly: 0 none, 1 slight, 2 medium, 3 full.
  pub hint_style: u8,
  /// Whether FreeType's autohinter replaces the font's own hints.
  pub autohint: bool,
  /// Whether a font's embedded bitmaps are used at sizes they cover.
  pub embedded_bitmaps: bool,
}

impl RenderSettings {
  /// fontconfig's own defaults.
  pub const DEFAULT: Self = Self {
    antialias: true,
    hinting: true,
    hint_style: 1,
    autohint: false,
    embedded_bitmaps: true,
  };
}

impl Default for RenderSettings {
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// The rendering settings a desktop states beneath fontconfig (XSETTINGS' `Xft/*`, GNOME's
/// font settings): each applies where the configuration names none, so a rule of the user's
/// own (`local.conf`, `~/.config/fontconfig/fonts.conf`) wins.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct DesktopSettings {
  /// Antialiasing.
  pub antialias: Option<bool>,
  /// Hinting.
  pub hinting: Option<bool>,
  /// The hint style, 0 to 3.
  pub hint_style: Option<u8>,
}

/// One font of a sorted list: its family, its file, and what it covers.
struct Candidate {
  family: Rc<str>,
  pattern: *mut FcPattern,
  charset: *mut FcCharSet,
}

/// The fonts fontconfig sorts for one request, best first.
pub struct Sorted {
  set: *mut FcFontSet,
  candidates: Vec<Candidate>,
  ascii: u128,
}

impl Sorted {
  /// Whether the best font covers every character of `text`, which is ASCII: answered from
  /// what it was found to cover when sorted, `false` where `text` is not ASCII.
  #[must_use]
  pub fn first_covers_ascii(&self, text: &str) -> bool {
    text
      .bytes()
      .all(|byte| byte < 128 && self.ascii & (1 << byte) != 0)
  }

  /// The first font covering every one of `characters`, its family.
  #[must_use]
  pub fn covering(&self, characters: &[char]) -> Option<Rc<str>> {
    self
      .candidates
      .iter()
      .find(|candidate| {
        !candidate.charset.is_null()
          && characters.iter().all(
            |character| unsafe { FcCharSetHasChar(candidate.charset, u32::from(*character)) } != 0,
          )
      })
      .map(|candidate| Rc::clone(&candidate.family))
  }

  /// Whether the font of `family` in this list covers every one of `characters`.
  #[must_use]
  pub fn family_covers(&self, family: &str, characters: &[char]) -> bool {
    self.candidates.iter().any(|candidate| {
      &*candidate.family == family
        && !candidate.charset.is_null()
        && characters.iter().all(
          |character| unsafe { FcCharSetHasChar(candidate.charset, u32::from(*character)) } != 0,
        )
    })
  }

  /// The best font's family.
  #[must_use]
  pub fn first(&self) -> Option<Rc<str>> {
    self
      .candidates
      .first()
      .map(|candidate| Rc::clone(&candidate.family))
  }
}

impl Drop for Sorted {
  fn drop(&mut self) {
    unsafe { FcFontSetDestroy(self.set) };
  }
}

/// A request to fontconfig: a family (or a generic name), a weight and a slant.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Request {
  /// The family or the generic name (`sans-serif`, `monospace`, `emoji`).
  pub family: String,
  /// The OpenType weight.
  pub weight: u16,
  /// Whether italic.
  pub italic: bool,
}

/// The user's fontconfig configuration, loaded once: the fonts it sorts per request (kept),
/// and the rendering settings it gives each font.
pub struct Fontconfig {
  config: *mut FcConfig,
  sorted: HashMap<Request, Rc<Sorted>>,
  desktop: DesktopSettings,
  generation: u64,
}

impl Fontconfig {
  /// The configuration fontconfig loads for this process (honouring `FONTCONFIG_FILE` and the
  /// user's own files), or `None` where it loads none.
  #[must_use]
  pub fn load() -> Option<Self> {
    let config = unsafe { FcInitLoadConfigAndFonts() };

    (!config.is_null()).then(|| Self {
      config,
      sorted: HashMap::new(),
      desktop: DesktopSettings::default(),
      generation: 0,
    })
  }

  /// Loads the configuration `file` states in place of this one, or the process's again where
  /// `file` is `None`, the desktop's settings kept beneath it; whether it loaded. Rendering
  /// settings read before are stale.
  pub fn reload(&mut self, file: Option<&Path>) -> bool {
    let config = match file {
      None => unsafe { FcInitLoadConfigAndFonts() },
      Some(file) => {
        let Ok(path) = CString::new(file.as_os_str().as_bytes()) else {
          return false;
        };

        unsafe {
          let config = FcConfigCreate();

          if !config.is_null()
            && (FcConfigParseAndLoad(config, path.as_ptr().cast(), 1) == 0
              || FcConfigBuildFonts(config) == 0)
          {
            FcConfigDestroy(config);

            return false;
          }

          config
        }
      }
    };

    if config.is_null() {
      return false;
    }

    self.sorted.clear();
    unsafe { FcConfigDestroy(self.config) };
    self.config = config;
    self.generation += 1;

    true
  }

  /// Sets the desktop's settings beneath the configuration; whether they changed.
  pub fn set_desktop(&mut self, desktop: DesktopSettings) -> bool {
    if self.desktop == desktop {
      return false;
    }

    self.desktop = desktop;
    self.generation += 1;

    true
  }

  /// The desktop's settings beneath the configuration.
  #[must_use]
  pub const fn desktop(&self) -> DesktopSettings {
    self.desktop
  }

  /// How many times the desktop's settings or the configuration changed: rendering settings
  /// read at another generation are stale.
  #[must_use]
  pub const fn generation(&self) -> u64 {
    self.generation
  }

  fn pattern(&self, request: &Request) -> *mut FcPattern {
    self.sized_pattern(request, 0.)
  }

  /// The pattern of `request`, at `pixel_size` where it is above zero, substituted as
  /// fontconfig substitutes a request.
  fn sized_pattern(&self, request: &Request, pixel_size: f32) -> *mut FcPattern {
    let family = CString::new(request.family.as_str()).unwrap_or_default();

    unsafe {
      let pattern = FcPatternCreate();

      if pixel_size > 0. {
        FcPatternAddDouble(pattern, object(b"pixelsize\0"), f64::from(pixel_size));
      }

      FcPatternAddString(pattern, object(b"family\0"), family.as_ptr().cast());
      FcPatternAddInteger(
        pattern,
        object(b"weight\0"),
        FcWeightFromOpenType(c_int::from(request.weight)),
      );
      FcPatternAddInteger(
        pattern,
        object(b"slant\0"),
        if request.italic {
          SLANT_ITALIC
        } else {
          SLANT_ROMAN
        },
      );

      if let Some(antialias) = self.desktop.antialias {
        FcPatternAddBool(pattern, object(b"antialias\0"), c_int::from(antialias));
      }

      if let Some(hinting) = self.desktop.hinting {
        FcPatternAddBool(pattern, object(b"hinting\0"), c_int::from(hinting));
      }

      if let Some(style) = self.desktop.hint_style {
        FcPatternAddInteger(pattern, object(b"hintstyle\0"), c_int::from(style));
      }

      FcConfigSubstitute(self.config, pattern, MATCH_PATTERN);
      FcDefaultSubstitute(pattern);

      pattern
    }
  }

  /// The fonts sorted for `request`, best first, each covering something the ones before do
  /// not (fontconfig's trimmed sort, the list Pango falls back through).
  pub fn sort(&mut self, request: &Request) -> Rc<Sorted> {
    if let Some(sorted) = self.sorted.get(request) {
      return Rc::clone(sorted);
    }

    let pattern = self.pattern(request);
    let mut result = 0;
    let set = unsafe { FcFontSort(self.config, pattern, 1, ptr::null_mut(), &raw mut result) };
    let mut candidates = Vec::new();

    if !set.is_null() {
      let count = usize::try_from(unsafe { (*set).nfont }).unwrap_or(0);

      for at in 0..count {
        let font = unsafe { *(*set).fonts.add(at) };
        let mut family: *mut u8 = ptr::null_mut();
        let mut charset: *mut FcCharSet = ptr::null_mut();

        unsafe {
          if FcPatternGetString(font, object(b"family\0"), 0, &raw mut family) != MATCH {
            continue;
          }

          FcPatternGetCharSet(font, object(b"charset\0"), 0, &raw mut charset);
        }

        let family = unsafe { CStr::from_ptr(family.cast::<c_char>()) }
          .to_string_lossy()
          .into_owned();

        candidates.push(Candidate {
          family: Rc::from(family),
          pattern: font,
          charset,
        });
      }
    }

    unsafe { FcPatternDestroy(pattern) };

    let ascii = candidates
      .first()
      .filter(|candidate| !candidate.charset.is_null())
      .map_or(0, |candidate| {
        (0..128_u32)
          .filter(|character| unsafe { FcCharSetHasChar(candidate.charset, *character) } != 0)
          .fold(0_u128, |mask, character| mask | 1 << character)
      });
    let sorted = Rc::new(Sorted {
      set,
      candidates,
      ascii,
    });

    self.sorted.insert(request.clone(), Rc::clone(&sorted));

    sorted
  }

  /// The rendering settings the configuration gives the font of `family` best matching
  /// `request` at `pixel_size`: its `target="font"` rules applied over the request's, as
  /// cairo prepares a font, so a rule keyed on the size, the weight or the slant applies.
  #[must_use]
  pub fn render_settings(
    &mut self,
    request: &Request,
    family: &str,
    pixel_size: f32,
  ) -> RenderSettings {
    let sorted = self.sort(request);
    let Some(candidate) = sorted
      .candidates
      .iter()
      .find(|candidate| &*candidate.family == family)
      .or_else(|| sorted.candidates.first())
    else {
      return RenderSettings::DEFAULT;
    };
    let pattern = self.sized_pattern(request, pixel_size);
    let prepared = unsafe { FcFontRenderPrepare(self.config, pattern, candidate.pattern) };

    unsafe { FcPatternDestroy(pattern) };

    if prepared.is_null() {
      return RenderSettings::DEFAULT;
    }

    let flag = |name: &'static [u8], default: bool| {
      let mut value: c_int = 0;

      if unsafe { FcPatternGetBool(prepared, object(name), 0, &raw mut value) } == MATCH {
        value != 0
      } else {
        default
      }
    };
    let mut style: c_int = 1;

    unsafe { FcPatternGetInteger(prepared, object(b"hintstyle\0"), 0, &raw mut style) };

    let settings = RenderSettings {
      antialias: flag(b"antialias\0", true),
      hinting: flag(b"hinting\0", true),
      hint_style: u8::try_from(style.clamp(0, 3)).unwrap_or(1),
      autohint: flag(b"autohint\0", false),
      embedded_bitmaps: flag(b"embeddedbitmap\0", true),
    };

    unsafe { FcPatternDestroy(prepared) };

    settings
  }

  /// The family best matching `family` (a name or a generic one), as the configuration
  /// aliases it.
  pub fn resolve_family(&mut self, family: &str) -> Option<Rc<str>> {
    self
      .sort(&Request {
        family: family.to_owned(),
        weight: 400,
        italic: false,
      })
      .first()
  }
}

impl Drop for Fontconfig {
  fn drop(&mut self) {
    self.sorted.clear();
    unsafe { FcConfigDestroy(self.config) };
  }
}
