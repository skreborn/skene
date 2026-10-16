//! Colors, in sRGB with straight alpha.

use std::fmt;

/// An sRGB color with straight alpha, eight bits a channel.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub struct Color {
  /// Red, 0 to 255.
  pub red: u8,
  /// Green, 0 to 255.
  pub green: u8,
  /// Blue, 0 to 255.
  pub blue: u8,
  /// Opacity, 0 (transparent) to 255 (opaque).
  pub alpha: u8,
}

impl Color {
  /// Opaque white.
  pub const WHITE: Self = Self::rgb(0xff_ffff);
  /// Opaque black.
  pub const BLACK: Self = Self::rgb(0x00_0000);
  /// Fully transparent.
  pub const TRANSPARENT: Self = Self::rgba(0x0000_0000);

  /// An opaque color from `0xRRGGBB`.
  #[inline(always)]
  pub const fn rgb(hex: u32) -> Self {
    Self {
      red: (hex >> 16) as u8,
      green: (hex >> 8) as u8,
      blue: hex as u8,
      alpha: 0xff,
    }
  }

  /// A color from `0xRRGGBBAA`.
  #[inline(always)]
  pub const fn rgba(hex: u32) -> Self {
    Self {
      red: (hex >> 24) as u8,
      green: (hex >> 16) as u8,
      blue: (hex >> 8) as u8,
      alpha: hex as u8,
    }
  }

  /// This color at opacity `alpha`.
  #[inline(always)]
  pub const fn with_alpha(self, alpha: u8) -> Self {
    Self { alpha, ..self }
  }

  /// Each channel as a fraction of one: red, green, blue, alpha.
  #[inline(always)]
  #[must_use]
  pub fn fractions(self) -> [f32; 4] {
    [self.red, self.green, self.blue, self.alpha].map(|channel| f32::from(channel) / 255.)
  }
}

/// `#rrggbb`, or `#rrggbbaa` where the color is not opaque.
impl fmt::Display for Color {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "#{:02x}{:02x}{:02x}",
      self.red, self.green, self.blue
    )?;

    if self.alpha != 0xff {
      write!(formatter, "{:02x}", self.alpha)?;
    }

    Ok(())
  }
}
