//! The NDK's font API (API 29, `libandroid`): `ASystemFontIterator` over the system's fonts and
//! `AFontMatcher`, which answers with Minikin's own choice for a run of text.

use std::{
  ffi::{c_char, CStr, CString},
  time::Instant,
};

use serde::Serialize;

#[repr(C)]
struct AFont {
  _private: [u8; 0],
}

#[repr(C)]
struct AFontMatcher {
  _private: [u8; 0],
}

#[repr(C)]
struct ASystemFontIterator {
  _private: [u8; 0],
}

#[cfg_attr(target_os = "android", link(name = "android"))]
extern "C" {
  fn ASystemFontIterator_open() -> *mut ASystemFontIterator;
  fn ASystemFontIterator_close(iterator: *mut ASystemFontIterator);
  fn ASystemFontIterator_next(iterator: *mut ASystemFontIterator) -> *mut AFont;
  fn AFont_close(font: *mut AFont);
  fn AFont_getFontFilePath(font: *const AFont) -> *const c_char;
  fn AFont_getWeight(font: *const AFont) -> u16;
  fn AFont_isItalic(font: *const AFont) -> bool;
  fn AFont_getLocale(font: *const AFont) -> *const c_char;
  fn AFont_getCollectionIndex(font: *const AFont) -> usize;
  fn AFont_getAxisCount(font: *const AFont) -> usize;
  fn AFont_getAxisTag(font: *const AFont, axis: u32) -> u32;
  fn AFont_getAxisValue(font: *const AFont, axis: u32) -> f32;
  fn AFontMatcher_create() -> *mut AFontMatcher;
  fn AFontMatcher_destroy(matcher: *mut AFontMatcher);
  fn AFontMatcher_setStyle(matcher: *mut AFontMatcher, weight: u16, italic: bool);
  fn AFontMatcher_setLocales(matcher: *mut AFontMatcher, tags: *const c_char);
  fn AFontMatcher_setFamilyVariant(matcher: *mut AFontMatcher, variant: u32);
  fn android_get_device_api_level() -> i32;
  fn AFontMatcher_match(
    matcher: *const AFontMatcher,
    family: *const c_char,
    text: *const u16,
    length: u32,
    run_length: *mut u32,
  ) -> *mut AFont;
}

/// `AFAMILY_VARIANT_COMPACT`: the UI fonts a `Paint` without `elegantTextHeight` asks for.
pub const VARIANT_COMPACT: u32 = 1;

/// One font as the NDK describes it.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct FontInfo {
  /// The file.
  pub path: String,
  /// The face in a collection.
  pub index: u32,
  /// The declared weight.
  pub weight: u16,
  /// Whether the face is italic.
  pub italic: bool,
  /// The declared locale, where one is.
  pub locale: Option<String>,
  /// The variation axes' values.
  pub axes: Vec<(String, f32)>,
}

fn tag_name(tag: u32) -> String {
  String::from_utf8_lossy(&tag.to_be_bytes()).into_owned()
}

fn text_of(pointer: *const c_char) -> Option<String> {
  if pointer.is_null() {
    return None;
  }

  // SAFETY: the NDK returns a NUL-terminated string valid while the font is open.
  Some(
    unsafe { CStr::from_ptr(pointer) }
      .to_string_lossy()
      .into_owned(),
  )
}

impl FontInfo {
  /// Reads `font` and closes it.
  fn take(font: *mut AFont) -> Self {
    // SAFETY: `font` is a live font from the NDK, closed once at the end.
    unsafe {
      let axes = (0..AFont_getAxisCount(font) as u32)
        .map(|axis| {
          (
            tag_name(AFont_getAxisTag(font, axis)),
            AFont_getAxisValue(font, axis),
          )
        })
        .collect();
      let info = Self {
        path: text_of(AFont_getFontFilePath(font)).unwrap_or_default(),
        index: AFont_getCollectionIndex(font) as u32,
        weight: AFont_getWeight(font),
        italic: AFont_isItalic(font),
        // Android 10's AFont_getLocale inverts its null check: it dereferences the missing
        // locale and answers null for a present one, so it is asked from Android 11 on.
        locale: if android_get_device_api_level() >= 30 {
          text_of(AFont_getLocale(font)).filter(|locale| !locale.is_empty())
        }
        else {
          None
        },
        axes,
      };

      AFont_close(font);

      info
    }
  }
}

/// Every font `ASystemFontIterator` lists, and how long listing them took.
pub fn system_fonts() -> (Vec<FontInfo>, f64) {
  let started = Instant::now();
  let mut fonts = Vec::new();

  // SAFETY: the iterator is opened, drained and closed here.
  unsafe {
    let iterator = ASystemFontIterator_open();

    if !iterator.is_null() {
      loop {
        let font = ASystemFontIterator_next(iterator);

        if font.is_null() {
          break;
        }

        fonts.push(FontInfo::take(font));
      }

      ASystemFontIterator_close(iterator);
    }
  }

  (fonts, started.elapsed().as_secs_f64() * 1e3)
}

/// A matched run: its UTF-16 range in the text and its font.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct MatchedRun {
  /// The first UTF-16 unit.
  pub start: usize,
  /// The end, exclusive.
  pub end: usize,
  /// The font.
  pub font: FontInfo,
}

/// An `AFontMatcher` set up with a style, a locale list and a family variant.
pub struct Matcher {
  matcher: *mut AFontMatcher,
}

impl Matcher {
  /// A matcher for `weight` and `italic`, `locales` a comma-separated list of BCP 47 tags.
  pub fn new(weight: u16, italic: bool, locales: &str, variant: u32) -> Self {
    let locales = CString::new(locales).unwrap_or_default();

    // SAFETY: the matcher is created here and destroyed on drop; the locale string is copied.
    unsafe {
      let matcher = AFontMatcher_create();

      AFontMatcher_setStyle(matcher, weight, italic);
      AFontMatcher_setLocales(matcher, locales.as_ptr());
      AFontMatcher_setFamilyVariant(matcher, variant);

      Self { matcher }
    }
  }

  /// The font Minikin picks for the start of `text` in `family`, and the UTF-16 length it covers.
  pub fn first(&self, family: &str, text: &[u16]) -> Option<(FontInfo, usize)> {
    if text.is_empty() {
      return None;
    }

    let family = CString::new(family).ok()?;
    let mut run = 0u32;

    // SAFETY: the text outlives the call; the returned font is taken (closed) at once.
    let font = unsafe {
      AFontMatcher_match(
        self.matcher,
        family.as_ptr(),
        text.as_ptr(),
        text.len() as u32,
        &mut run,
      )
    };

    if font.is_null() {
      return None;
    }

    Some((FontInfo::take(font), (run as usize).max(1)))
  }

  /// Every run of `text` with its font: the matcher asked again from each run's end.
  pub fn runs(&self, family: &str, text: &str) -> Vec<MatchedRun> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut runs = Vec::new();
    let mut at = 0;

    while at < units.len() {
      let Some((font, length)) = self.first(family, &units[at..])
      else {
        break;
      };
      let end = (at + length).min(units.len());

      runs.push(MatchedRun {
        start: at,
        end,
        font,
      });
      at = end;
    }

    runs
  }

  /// Nanoseconds per call of `count` matches of `text`.
  pub fn bench(&self, family: &str, text: &str, count: usize) -> f64 {
    let units: Vec<u16> = text.encode_utf16().collect();
    let family = CString::new(family).unwrap_or_default();
    let started = Instant::now();

    for _ in 0..count {
      let mut run = 0u32;

      // SAFETY: as in `first`.
      unsafe {
        let font = AFontMatcher_match(
          self.matcher,
          family.as_ptr(),
          units.as_ptr(),
          units.len() as u32,
          &mut run,
        );

        if !font.is_null() {
          AFont_close(font);
        }
      }
    }

    started.elapsed().as_nanos() as f64 / count.max(1) as f64
  }
}

impl Drop for Matcher {
  fn drop(&mut self) {
    // SAFETY: created in `new`, destroyed once.
    unsafe { AFontMatcher_destroy(self.matcher) }
  }
}
