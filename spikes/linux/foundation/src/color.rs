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

  /// The color `amount` of the way from this one to `other` (`0` this one, `1` the other),
  /// mixed in Oklab with premultiplied alpha, as a transition moves between them.
  pub fn mix(self, other: Self, amount: f32) -> Self {
    let (from, to) = (self.to_oklab(), other.to_oklab());
    let alpha = from[3] + (to[3] - from[3]) * amount;
    let component = |index: usize| {
      let start = from[index] * from[3];
      let mixed = start + (to[index] * to[3] - start) * amount;

      if alpha > 0. { mixed / alpha } else { mixed }
    };

    Self::from_oklab([component(0), component(1), component(2)], alpha)
  }

  /// Lightness, a and b in Oklab, then alpha, each as a float.
  #[must_use]
  pub fn to_oklab(self) -> [f32; 4] {
    let [red, green, blue, alpha] = self.fractions();
    let linear = |channel: f32| {
      if channel <= 0.040_45 {
        channel / 12.92
      } else {
        ((channel + 0.055) / 1.055).powf(2.4)
      }
    };
    let (red, green, blue) = (linear(red), linear(green), linear(blue));
    let long = (0.412_221_47 * red + 0.536_332_55 * green + 0.051_445_99 * blue).cbrt();
    let medium = (0.211_903_5 * red + 0.680_699_5 * green + 0.107_396_96 * blue).cbrt();
    let short = (0.088_302_46 * red + 0.281_718_85 * green + 0.629_978_7 * blue).cbrt();

    [
      0.210_454_26 * long + 0.793_617_8 * medium - 0.004_072_047 * short,
      1.977_998_5 * long - 2.428_592_2 * medium + 0.450_593_7 * short,
      0.025_904_037 * long + 0.782_771_77 * medium - 0.808_675_77 * short,
      alpha,
    ]
  }

  /// The color of Oklab `lightness, a, b` at `alpha` (`0` to `1`), each channel rounded.
  pub fn from_oklab([lightness, a, b]: [f32; 3], alpha: f32) -> Self {
    let long = (lightness + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let medium = (lightness - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let short = (lightness - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let encode = |channel: f32| {
      let channel = channel.clamp(0., 1.);
      let encoded = if channel <= 0.003_130_8 {
        channel * 12.92
      } else {
        1.055 * channel.powf(1. / 2.4) - 0.055
      };

      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a channel held to 0..=255"
      )]
      let byte = (encoded * 255.).round().clamp(0., 255.) as u8;

      byte
    };

    Self {
      red: encode(4.076_741_7 * long - 3.307_711_6 * medium + 0.230_969_94 * short),
      green: encode(-1.268_438 * long + 2.609_757_4 * medium - 0.341_319_38 * short),
      blue: encode(-0.004_196_086_3 * long - 0.703_418_6 * medium + 1.707_614_7 * short),
      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an alpha held to 0..=255"
      )]
      alpha: (alpha.clamp(0., 1.) * 255.).round() as u8,
    }
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
