//! Terminal capability detection: color depth and background darkness.
//!
//! Both detections degrade quietly: an unresponsive or unknown terminal gets 16 colors
//! and a dark background.

use std::env;
#[cfg(unix)]
use std::{
  io::Write,
  time::{Duration, Instant},
};

use crossterm::style::Color as TerminalColor;

/// How many colors the terminal renders; painted RGB quantizes down to this depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorDepth {
  /// 24-bit RGB.
  TrueColor,
  /// The xterm 256-color palette.
  Ansi256,
  /// The 16 base colors.
  Ansi16,
}

impl ColorDepth {
  /// Detects the terminal's color depth from `COLORTERM` and `TERM`.
  pub(crate) fn detect() -> Self {
    Self::from_variables(
      env::var("COLORTERM").ok().as_deref(),
      env::var("TERM").ok().as_deref(),
    )
  }

  /// The depth `COLORTERM` and `TERM` values name; 16 colors when neither says more.
  fn from_variables(colorterm: Option<&str>, term: Option<&str>) -> Self {
    let truecolor = colorterm
      .is_some_and(|value| value.contains("truecolor") || value.contains("24bit"))
      || term.is_some_and(|value| value.contains("direct"));

    if truecolor {
      return Self::TrueColor;
    }

    if term.is_some_and(|value| value.contains("256")) {
      return Self::Ansi256;
    }

    Self::Ansi16
  }

  /// The nearest color the terminal renders at this depth.
  pub(crate) fn terminal_color(self, (red, green, blue): (u8, u8, u8)) -> TerminalColor {
    match self {
      Self::TrueColor => TerminalColor::Rgb {
        r: red,
        g: green,
        b: blue,
      },
      Self::Ansi256 => TerminalColor::AnsiValue(nearest_256(red, green, blue)),
      Self::Ansi16 => TerminalColor::AnsiValue(nearest_16(red, green, blue)),
    }
  }
}

/// The squared distance between two RGB colors.
const fn distance(
  (red, green, blue): (u8, u8, u8),
  (other_red, other_green, other_blue): (u8, u8, u8),
) -> i32 {
  let delta_red = red as i32 - other_red as i32;
  let delta_green = green as i32 - other_green as i32;
  let delta_blue = blue as i32 - other_blue as i32;

  delta_red * delta_red + delta_green * delta_green + delta_blue * delta_blue
}

/// The xterm 256-palette index nearest to an RGB color.
fn nearest_256(red: u8, green: u8, blue: u8) -> u8 {
  const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

  let level = |component: u8| {
    LEVELS
      .iter()
      .enumerate()
      .min_by_key(|(_, level)| (i32::from(component) - i32::from(**level)).abs())
      .map_or(0, |(index, _)| index)
  };

  let (red_index, green_index, blue_index) = (level(red), level(green), level(blue));
  let cube_rgb = (LEVELS[red_index], LEVELS[green_index], LEVELS[blue_index]);
  #[expect(clippy::cast_possible_truncation)]
  let cube = 16 + 36 * red_index as u8 + 6 * green_index as u8 + blue_index as u8;

  let average = (u16::from(red) + u16::from(green) + u16::from(blue)) / 3;
  let step = ((i32::from(average) - 8 + 5) / 10).clamp(0, 23);
  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let gray_level = (8 + 10 * step) as u8;
  #[expect(clippy::cast_sign_loss)]
  let gray = 232 + step as u8;

  let target = (red, green, blue);

  if distance(target, (gray_level, gray_level, gray_level)) < distance(target, cube_rgb) {
    gray
  }
  else {
    cube
  }
}

/// The 16-color palette index nearest to an RGB color, against the xterm defaults.
fn nearest_16(red: u8, green: u8, blue: u8) -> u8 {
  const PALETTE: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
  ];

  let target = (red, green, blue);

  #[expect(clippy::cast_possible_truncation)]
  PALETTE
    .iter()
    .enumerate()
    .min_by_key(|(_, candidate)| distance(target, **candidate))
    .map_or(0, |(index, _)| index as u8)
}

/// Whether the terminal background is dark: an OSC 11 query first, `COLORFGBG` when the
/// terminal stays silent, dark when neither answers.
///
/// Call with raw mode enabled: the reply arrives on stdin and must not echo.
pub(crate) fn background_is_dark() -> bool {
  if let Some((red, green, blue)) = query_background() {
    return is_dark(red, green, blue);
  }

  env::var("COLORFGBG")
    .ok()
    .and_then(|value| colorfgbg_is_dark(&value))
    .unwrap_or(true)
}

/// Whether an RGB background counts as dark, by Rec. 709 luminance.
fn is_dark(red: u8, green: u8, blue: u8) -> bool {
  let luminance = 0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue);

  luminance < 127.5
}

/// Whether a `COLORFGBG` value (`"fg;bg"`, possibly with segments between) names a dark
/// background.
fn colorfgbg_is_dark(value: &str) -> Option<bool> {
  let background: u8 = value.rsplit(';').next()?.trim().parse().ok()?;

  Some(!matches!(background, 7 | 15))
}

/// Queries the terminal background via OSC 11, waiting briefly for the reply on stdin.
#[cfg(unix)]
fn query_background() -> Option<(u8, u8, u8)> {
  const PREFIX: &[u8] = b"\x1b]11;";

  let mut out = std::io::stdout();

  write!(out, "\x1b]11;?\x1b\\").ok()?;
  out.flush().ok()?;

  let deadline = Instant::now() + Duration::from_millis(100);
  let mut response = Vec::new();

  loop {
    let remaining = deadline.checked_duration_since(Instant::now())?;
    let mut descriptor = libc::pollfd {
      fd: 0,
      events: libc::POLLIN,
      revents: 0,
    };
    let timeout = i32::try_from(remaining.as_millis()).unwrap_or(i32::MAX);
    let ready = unsafe { libc::poll(&raw mut descriptor, 1, timeout) };

    if ready <= 0 {
      return None;
    }

    let mut byte = 0u8;
    let count = unsafe { libc::read(0, std::ptr::from_mut(&mut byte).cast(), 1) };

    if count != 1 {
      return None;
    }

    response.push(byte);

    // Anything that stops being an OSC 11 reply prefix is unrelated input: stop
    // consuming rather than eat the user's first keystrokes.
    if response.len() <= PREFIX.len() && response != PREFIX[..response.len()] {
      return None;
    }

    if byte == 0x07 || response.ends_with(b"\x1b\\") {
      return parse_osc11(&response);
    }

    if response.len() > 64 {
      return None;
    }
  }
}

/// No reply channel outside a Unix terminal.
#[cfg(not(unix))]
fn query_background() -> Option<(u8, u8, u8)> {
  None
}

/// Parses an OSC 11 reply (`ESC ] 11 ; rgb : rrrr / gggg / bbbb BEL|ST`) to RGB.
fn parse_osc11(response: &[u8]) -> Option<(u8, u8, u8)> {
  let text = std::str::from_utf8(response).ok()?;
  let spec = text
    .split("]11;")
    .nth(1)?
    .trim_start_matches("rgba:")
    .trim_start_matches("rgb:");
  let mut parts = spec.split('/');
  let mut component = || {
    let part = parts.next()?;
    let digits: String = part.chars().take_while(char::is_ascii_hexdigit).collect();

    if digits.is_empty() || digits.len() > 4 {
      return None;
    }

    let value = u32::from_str_radix(&digits, 16).ok()?;
    let maximum = (1u32 << (4 * digits.len())) - 1;

    #[expect(clippy::cast_possible_truncation)]
    Some((value * 255 / maximum) as u8)
  };

  Some((component()?, component()?, component()?))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn depth_reads_the_variables() {
    assert_eq!(
      ColorDepth::from_variables(Some("truecolor"), Some("xterm-256color")),
      ColorDepth::TrueColor
    );
    assert_eq!(
      ColorDepth::from_variables(None, Some("xterm-direct")),
      ColorDepth::TrueColor
    );
    assert_eq!(
      ColorDepth::from_variables(None, Some("xterm-256color")),
      ColorDepth::Ansi256
    );
    assert_eq!(
      ColorDepth::from_variables(None, Some("vt100")),
      ColorDepth::Ansi16
    );
    assert_eq!(ColorDepth::from_variables(None, None), ColorDepth::Ansi16);
  }

  #[test]
  fn quantization_finds_the_nearest_palette_entry() {
    assert_eq!(nearest_256(255, 0, 0), 196);
    assert_eq!(nearest_256(255, 255, 255), 231);
    assert_eq!(nearest_256(128, 128, 128), 244);
    assert_eq!(nearest_16(0, 0, 0), 0);
    assert_eq!(nearest_16(255, 0, 0), 9);
    assert_eq!(nearest_16(255, 255, 255), 15);
  }

  #[test]
  fn osc11_replies_parse_at_any_component_width() {
    assert_eq!(
      parse_osc11(b"\x1b]11;rgb:1e1e/2e2e/3e3e\x07"),
      Some((30, 46, 62))
    );
    assert_eq!(
      parse_osc11(b"\x1b]11;rgb:ff/80/00\x1b\\"),
      Some((255, 128, 0))
    );
    assert_eq!(
      parse_osc11(b"\x1b]11;rgba:ffff/0000/0000\x07"),
      Some((255, 0, 0))
    );
    assert_eq!(parse_osc11(b"garbage"), None);
  }

  #[test]
  fn colorfgbg_names_the_background() {
    assert_eq!(colorfgbg_is_dark("15;0"), Some(true));
    assert_eq!(colorfgbg_is_dark("0;15"), Some(false));
    assert_eq!(colorfgbg_is_dark("15;default;7"), Some(false));
    assert_eq!(colorfgbg_is_dark("nonsense"), None);
  }

  #[test]
  fn luminance_splits_dark_from_light() {
    assert!(is_dark(30, 30, 46));
    assert!(!is_dark(255, 255, 255));
  }
}
