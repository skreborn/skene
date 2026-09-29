//! The environment a server render answers with, parsed from the client hints a
//! request carries.

use skene_core::{ColorScheme, PointerPrecision, Size};

/// The environment a render answers with: what the client hints say, else the
/// defaults.
///
/// Without hints a render assumes a 1024 by 768 viewport, the light scheme, no
/// reduced-motion or reduced-transparency preference, a fine pointer that hovers and a
/// device pixel ratio of one.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct RenderHints {
  /// The viewport's size, in logical pixels.
  pub viewport: Size<f32>,
  /// The color scheme in force.
  pub scheme: ColorScheme,
  /// Whether the reader prefers reduced motion.
  pub reduced_motion: bool,
  /// Whether the reader prefers reduced transparency.
  pub reduced_transparency: bool,
  /// How precisely the reader's primary pointer points; no request header says.
  pub pointer: PointerPrecision,
  /// Whether the reader's primary pointer hovers; no request header says.
  pub hover: bool,
  /// The device pixels per logical pixel.
  pub device_pixel_ratio: f32,
}

impl RenderHints {
  /// The defaults a render assumes without hints.
  pub const DEFAULT: Self = Self {
    viewport: Size::new(1024., 768.),
    scheme: ColorScheme::Light,
    reduced_motion: false,
    reduced_transparency: false,
    pointer: PointerPrecision::Fine,
    hover: true,
    device_pixel_ratio: 1.,
  };

  /// The request headers a render reads: the color scheme, the reduced-motion and
  /// reduced-transparency preferences, the viewport's width and height, and the device
  /// pixel ratio.
  pub const REQUEST_HEADERS: [&'static str; 6] = [
    "Sec-CH-Prefers-Color-Scheme",
    "Sec-CH-Prefers-Reduced-Motion",
    "Sec-CH-Prefers-Reduced-Transparency",
    "Sec-CH-Viewport-Width",
    "Sec-CH-Viewport-Height",
    "Sec-CH-DPR",
  ];

  /// The hints `headers` carry over the defaults, each value read as the structured
  /// field it is (a quoted string or a token, an integer or a decimal); header names
  /// compare case-insensitively, and a header that is absent or malformed leaves its
  /// default.
  pub fn from_headers<'a>(headers: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
    let mut hints = Self::DEFAULT;

    for (name, value) in headers {
      if name.eq_ignore_ascii_case("Sec-CH-Prefers-Color-Scheme") {
        match word(value).as_deref() {
          Some(scheme) if scheme.eq_ignore_ascii_case("dark") => hints.scheme = ColorScheme::Dark,
          Some(scheme) if scheme.eq_ignore_ascii_case("light") => {
            hints.scheme = ColorScheme::Light;
          }
          _ => {}
        }
      }
      else if name.eq_ignore_ascii_case("Sec-CH-Prefers-Reduced-Motion") {
        hints.reduced_motion =
          word(value).is_some_and(|motion| motion.eq_ignore_ascii_case("reduce"));
      }
      else if name.eq_ignore_ascii_case("Sec-CH-Prefers-Reduced-Transparency") {
        hints.reduced_transparency =
          word(value).is_some_and(|transparency| transparency.eq_ignore_ascii_case("reduce"));
      }
      else if name.eq_ignore_ascii_case("Sec-CH-Viewport-Width") {
        if let Some(width) = positive(value) {
          hints.viewport.width = width;
        }
      }
      else if name.eq_ignore_ascii_case("Sec-CH-Viewport-Height") {
        if let Some(height) = positive(value) {
          hints.viewport.height = height;
        }
      }
      else if name.eq_ignore_ascii_case("Sec-CH-DPR")
        && let Some(ratio) = positive(value)
      {
        hints.device_pixel_ratio = ratio;
      }
    }

    hints
  }

  /// The response headers asking a client for the hints: `Accept-CH` naming them,
  /// `Critical-CH` asking for them on the first request, and `Vary` so a cache keys
  /// on them.
  #[must_use]
  pub fn response_headers() -> Vec<(&'static str, String)> {
    let names = Self::REQUEST_HEADERS.join(", ");

    vec![
      ("Accept-CH", names.clone()),
      ("Critical-CH", names.clone()),
      ("Vary", names),
    ]
  }
}

impl Default for RenderHints {
  #[inline(always)]
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// The item the structured field `value` holds, its parameters aside.
fn item(value: &str) -> Option<sfv::BareItem> {
  sfv::Parser::new(value.trim())
    .parse::<sfv::Item>()
    .ok()
    .map(|item| item.bare_item)
}

/// The text of the string or token the structured field `value` holds.
fn word(value: &str) -> Option<String> {
  let item = item(value)?;

  item
    .as_string()
    .map(|string| string.as_str().to_owned())
    .or_else(|| item.as_token().map(|token| token.as_str().to_owned()))
}

/// The positive number, an integer or a decimal, the structured field `value` holds.
#[expect(
  clippy::cast_precision_loss,
  clippy::cast_possible_truncation,
  reason = "a hint's size and ratio lie far within an f32's range"
)]
fn positive(value: &str) -> Option<f32> {
  let item = item(value)?;
  let number = item
    .as_integer()
    .map(|integer| i64::from(integer) as f64)
    .or_else(|| item.as_decimal().map(f64::from))?;

  (number > 0.).then_some(number as f32)
}

#[cfg(test)]
mod tests {
  use skene_core::{ColorScheme, Size};

  use super::RenderHints;

  /// Every hint header is read, case-insensitively, over the defaults.
  #[test]
  fn headers_fill_the_hints() {
    let hints = RenderHints::from_headers([
      ("sec-ch-prefers-color-scheme", "dark"),
      ("Sec-CH-Prefers-Reduced-Motion", "reduce"),
      ("Sec-CH-Prefers-Reduced-Transparency", "reduce"),
      ("Sec-CH-Viewport-Width", "390"),
      ("sec-ch-viewport-height", " 844 "),
      ("Sec-CH-DPR", "3"),
      ("Cookie", "ignored"),
    ]);

    assert_eq!(hints.scheme, ColorScheme::Dark);
    assert!(hints.reduced_motion);
    assert!(hints.reduced_transparency);
    assert_eq!(hints.viewport, Size::new(390., 844.));
    assert!((hints.device_pixel_ratio - 3.).abs() < f32::EPSILON);
  }

  /// Each hint reads as the structured field a browser sends: the scheme and the
  /// motion preference quoted, the sizes as integers or decimals, parameters aside.
  #[test]
  fn hints_read_as_structured_fields() {
    let hints = RenderHints::from_headers([
      ("Sec-CH-Prefers-Color-Scheme", "\"dark\""),
      ("Sec-CH-Prefers-Reduced-Motion", "\"reduce\";q=1"),
      ("Sec-CH-Viewport-Width", "1280"),
      ("Sec-CH-Viewport-Height", "720.5"),
      ("Sec-CH-DPR", "1.25"),
    ]);

    assert_eq!(hints.scheme, ColorScheme::Dark);
    assert!(hints.reduced_motion);
    assert_eq!(hints.viewport, Size::new(1280., 720.5));
    assert!((hints.device_pixel_ratio - 1.25).abs() < f32::EPSILON);
  }

  /// A malformed or absent header leaves its default.
  #[test]
  fn malformed_headers_keep_the_defaults() {
    let hints = RenderHints::from_headers([
      ("Sec-CH-Prefers-Color-Scheme", "sepia"),
      ("Sec-CH-Viewport-Width", "wide"),
      ("Sec-CH-DPR", "-2"),
    ]);

    assert_eq!(hints, RenderHints::DEFAULT);
  }

  /// The response asks for every hint the render reads.
  #[test]
  fn response_headers_name_every_hint() {
    let headers = RenderHints::response_headers();

    assert_eq!(headers.len(), 3);

    for (_, value) in &headers {
      for name in RenderHints::REQUEST_HEADERS {
        assert!(value.contains(name), "{value} lacks {name}");
      }
    }
  }
}
