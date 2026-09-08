//! Terminal color depth and background luminance: what the loop resolves from the
//! terminal's replies, and the quantization a painted RGB takes at each depth.
//!
//! Nothing here reads the environment or the terminal. The loop assumes 16 colors and
//! a dark background until a reply says otherwise.

use termina::{
  escape::csi::Sgr,
  style::{ColorSpec, RgbColor},
};

/// The true-color background the depth probe sets before asking DECRQSS to report it.
pub(crate) const TRUE_COLOR_PROBE: RgbColor = RgbColor::new(0x12, 0x34, 0x56);

/// The palette background the depth probe sets before asking DECRQSS to report it.
pub(crate) const PALETTE_PROBE: u8 = 200;

/// How many colors the terminal renders; painted RGB quantizes down to this depth.
///
/// Depths order from fewest colors to most.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ColorDepth {
  /// The 16 base colors.
  Ansi16,
  /// The xterm 256-color palette.
  Ansi256,
  /// 24-bit RGB.
  TrueColor,
}

impl ColorDepth {
  /// The depth a DECRQSS graphic-rendition reply proves: true color when the probe's
  /// true-color background came back, 256 colors when its palette background did, and
  /// nothing otherwise.
  pub(crate) fn reported(attributes: &[Sgr]) -> Option<Self> {
    if attributes.contains(&Sgr::Background(TRUE_COLOR_PROBE.into())) {
      return Some(Self::TrueColor);
    }

    attributes
      .contains(&Sgr::Background(ColorSpec::PaletteIndex(PALETTE_PROBE)))
      .then_some(Self::Ansi256)
  }

  /// The nearest color the terminal renders at this depth.
  pub(crate) fn terminal_color(self, (red, green, blue): (u8, u8, u8)) -> ColorSpec {
    match self {
      Self::TrueColor => RgbColor::new(red, green, blue).into(),
      Self::Ansi256 => ColorSpec::PaletteIndex(nearest_256(red, green, blue)),
      Self::Ansi16 => ColorSpec::PaletteIndex(nearest_16(red, green, blue)),
    }
  }
}

/// Whether a background counts as dark, by Rec. 709 luminance.
pub(crate) fn is_dark(color: RgbColor) -> bool {
  let luminance = 0.2126 * f32::from(color.red)
    + 0.7152 * f32::from(color.green)
    + 0.0722 * f32::from(color.blue);

  luminance < 127.5
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

#[cfg(test)]
mod tests {
  use super::*;

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
  fn a_decrqss_reply_proves_the_depth_its_probe_came_back_at() {
    assert_eq!(
      ColorDepth::reported(&[Sgr::Background(TRUE_COLOR_PROBE.into())]),
      Some(ColorDepth::TrueColor)
    );
    assert_eq!(
      ColorDepth::reported(&[Sgr::Background(ColorSpec::PaletteIndex(PALETTE_PROBE))]),
      Some(ColorDepth::Ansi256)
    );
    assert_eq!(ColorDepth::reported(&[Sgr::Reset]), None);
    assert!(
      ColorDepth::Ansi16 < ColorDepth::Ansi256 && ColorDepth::Ansi256 < ColorDepth::TrueColor
    );
  }

  #[test]
  fn luminance_splits_dark_from_light() {
    assert!(is_dark(RgbColor::new(30, 30, 46)));
    assert!(!is_dark(RgbColor::new(255, 255, 255)));
  }
}
