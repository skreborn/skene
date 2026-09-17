use std::hash::{Hash, Hasher};

use color::{AlphaColor, ColorSpace, Hsl, Hwb, Lab, Srgb};
use serde::{Deserialize, Serialize};

use crate::Scalar;

/// One of the two color schemes a surface is painted for.
///
/// Names a scheme outright, where [`Media::Dark`](crate::Media::Dark)
/// and its light twin ask the platform which one is in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[must_use]
pub enum ColorScheme {
  /// Dark ink on a light surface.
  Light,
  /// Light ink on a dark surface.
  Dark,
}

/// Color value, stored as its Oklch `[lightness, chroma, hue, alpha]` components.
///
/// Every conversion runs through the same arithmetic on every target, so a color built
/// on a server and the same color built in a browser are the same bits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[must_use]
pub struct Color {
  lightness: f32,
  chroma: f32,
  hue: f32,
  alpha: f32,
}

impl Color {
  /// Fully transparent color.
  pub const TRANSPARENT: Self = Self {
    lightness: 0.,
    chroma: 0.,
    hue: 0.,
    alpha: 0.,
  };

  /// A color from its Oklab components.
  #[inline(always)]
  fn from_oklab(oklab: [f32; 3], alpha: f32) -> Self {
    let [lightness, chroma, hue] = convert::oklab_to_oklch(oklab);

    Self {
      lightness,
      chroma,
      hue,
      alpha,
    }
  }

  /// A color from its linear-light sRGB components.
  #[inline(always)]
  fn from_linear(linear: [f32; 3], alpha: f32) -> Self {
    Self::from_oklab(convert::linear_to_oklab(linear), alpha)
  }

  /// A color from its sRGB components.
  #[inline(always)]
  fn from_srgb(srgb: [f32; 3], alpha: f32) -> Self {
    Self::from_linear(srgb.map(convert::srgb_to_linear), alpha)
  }

  /// This color's Oklab components, alpha aside.
  #[inline(always)]
  fn oklab_components(self) -> [f32; 3] {
    convert::oklch_to_oklab([self.lightness, self.chroma, self.hue])
  }

  /// This color's linear-light sRGB components, alpha aside.
  #[inline(always)]
  fn linear_components(self) -> [f32; 3] {
    convert::oklab_to_linear(self.oklab_components())
  }

  /// This color's sRGB components, alpha aside.
  #[inline(always)]
  fn srgb_components(self) -> [f32; 3] {
    self.linear_components().map(convert::linear_to_srgb)
  }

  /// The components with `alpha` appended.
  #[inline(always)]
  const fn with([first, second, third]: [f32; 3], alpha: f32) -> [f32; 4] {
    [first, second, third, alpha]
  }

  /// The color as an sRGB alpha color, for a renderer.
  #[inline(always)]
  #[must_use]
  pub fn to_alpha_color(self) -> AlphaColor<Srgb> {
    AlphaColor::<Srgb>::new(self.to_rgba())
  }

  /// Creates an opaque color in the sRGB color space.
  ///
  /// Components on a scale of `0` to `1`.
  #[inline(always)]
  pub fn srgb(red: f32, green: f32, blue: f32) -> Self {
    Self::from_srgb([red, green, blue], 1.)
  }

  /// Creates an opaque color in the linear-light sRGB color space.
  ///
  /// Components on a scale of `0` to `1`.
  #[inline(always)]
  pub fn linear_srgb(red: f32, green: f32, blue: f32) -> Self {
    Self::from_linear([red, green, blue], 1.)
  }

  /// Creates an opaque color in the HSL color space.
  ///
  /// `hue` in degrees; `saturation` and `lightness` on a scale of `0` to `100`.
  #[inline(always)]
  pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Self {
    Self::from_srgb(Hsl::convert::<Srgb>([hue, saturation, lightness]), 1.)
  }

  /// Creates an opaque color in the HWB color space.
  ///
  /// `hue` in degrees; `whiteness` and `blackness` on a scale of `0` to `100`.
  #[inline(always)]
  pub fn hwb(hue: f32, whiteness: f32, blackness: f32) -> Self {
    Self::from_srgb(Hwb::convert::<Srgb>([hue, whiteness, blackness]), 1.)
  }

  /// Creates an opaque color in the CIE Lab color space.
  ///
  /// `lightness` on a scale of `0` to `100`.
  #[inline(always)]
  pub fn lab(lightness: f32, a_axis: f32, b_axis: f32) -> Self {
    Self::from_linear(Lab::to_linear_srgb([lightness, a_axis, b_axis]), 1.)
  }

  /// Creates an opaque color in the CIE `LCh` color space.
  ///
  /// `lightness` on a scale of `0` to `100` and `hue` in degrees.
  #[inline(always)]
  pub fn lch(lightness: f32, chroma: f32, hue: f32) -> Self {
    Self::from_linear(
      Lab::to_linear_srgb(convert::oklch_to_oklab([lightness, chroma, hue])),
      1.,
    )
  }

  /// Creates an opaque color in the Oklab color space.
  ///
  /// `lightness` on a scale of `0` to `1`.
  #[inline(always)]
  pub fn oklab(lightness: f32, a_axis: f32, b_axis: f32) -> Self {
    Self::from_oklab([lightness, a_axis, b_axis], 1.)
  }

  /// Creates an opaque color in the Oklch color space.
  ///
  /// `lightness` on a scale of `0` to `1` and `hue` in degrees.
  #[inline(always)]
  pub fn oklch(lightness: f32, chroma: f32, hue: f32) -> Self {
    Self {
      lightness,
      chroma,
      hue,
      alpha: 1.,
    }
  }

  /// Returns a new color with the specified alpha component.
  #[inline(always)]
  pub fn with_alpha(self, alpha: f32) -> Self {
    Self {
      lightness: self.lightness,
      chroma: self.chroma,
      hue: self.hue,
      alpha,
    }
  }

  /// The Oklch lightness, `0` to `1`.
  #[inline(always)]
  #[must_use]
  pub const fn lightness(self) -> f32 {
    self.lightness
  }

  /// The Oklch chroma.
  #[inline(always)]
  #[must_use]
  pub const fn chroma(self) -> f32 {
    self.chroma
  }

  /// The Oklch hue, in degrees.
  #[inline(always)]
  #[must_use]
  pub const fn hue(self) -> f32 {
    self.hue
  }

  /// The alpha component, `0` to `1`.
  #[inline(always)]
  #[must_use]
  pub const fn alpha(self) -> f32 {
    self.alpha
  }

  /// This color with its Oklch lightness replaced, `0` to `1`.
  #[inline(always)]
  pub const fn with_lightness(self, lightness: f32) -> Self {
    Self { lightness, ..self }
  }

  /// This color with its Oklch chroma replaced.
  #[inline(always)]
  pub const fn with_chroma(self, chroma: f32) -> Self {
    Self { chroma, ..self }
  }

  /// This color with its Oklch hue replaced, in degrees.
  #[inline(always)]
  pub const fn with_hue(self, hue: f32) -> Self {
    Self { hue, ..self }
  }

  /// This color mixed toward `other` in Oklab: `0.` is this color, `1.` is `other`, and
  /// every component, alpha included, interpolates linearly.
  pub fn mix(self, other: Self, amount: f32) -> Self {
    let from = self.to_oklab();
    let to = other.to_oklab();
    let mixed: [f32; 4] =
      std::array::from_fn(|component| from[component] + (to[component] - from[component]) * amount);

    Self::from_oklab([mixed[0], mixed[1], mixed[2]], mixed[3])
  }

  /// This color's `[red, green, blue, alpha]` components in the sRGB color space.
  #[inline(always)]
  #[must_use]
  pub fn to_rgba(self) -> [f32; 4] {
    Self::with(self.srgb_components(), self.alpha)
  }

  /// This color's `[red, green, blue, alpha]` components in the linear-light sRGB color space.
  #[inline(always)]
  #[must_use]
  pub fn to_linear_srgb(self) -> [f32; 4] {
    Self::with(self.linear_components(), self.alpha)
  }

  /// This color's `[hue, saturation, lightness, alpha]` components in the HSL color space.
  #[inline(always)]
  #[must_use]
  pub fn to_hsl(self) -> [f32; 4] {
    Self::with(Srgb::convert::<Hsl>(self.srgb_components()), self.alpha)
  }

  /// This color's `[hue, whiteness, blackness, alpha]` components in the HWB color space.
  #[inline(always)]
  #[must_use]
  pub fn to_hwb(self) -> [f32; 4] {
    Self::with(Srgb::convert::<Hwb>(self.srgb_components()), self.alpha)
  }

  /// This color's `[lightness, axis_a, axis_b, alpha]` components in the CIE Lab color space.
  #[inline(always)]
  #[must_use]
  pub fn to_lab(self) -> [f32; 4] {
    Self::with(Lab::from_linear_srgb(self.linear_components()), self.alpha)
  }

  /// This color's `[lightness, chroma, hue, alpha]` components in the CIE `LCh` color space.
  #[inline(always)]
  #[must_use]
  pub fn to_lch(self) -> [f32; 4] {
    Self::with(
      convert::oklab_to_oklch(Lab::from_linear_srgb(self.linear_components())),
      self.alpha,
    )
  }

  /// This color's `[lightness, axis_a, axis_b, alpha]` components in the Oklab color space.
  #[inline(always)]
  #[must_use]
  pub fn to_oklab(self) -> [f32; 4] {
    Self::with(self.oklab_components(), self.alpha)
  }

  /// This color's `[lightness, chroma, hue, alpha]` components in the Oklch color space.
  #[inline(always)]
  #[must_use]
  pub const fn to_oklch(self) -> [f32; 4] {
    [self.lightness, self.chroma, self.hue, self.alpha]
  }
}

impl Hash for Color {
  /// Hashes the stored components by value, so equal colors hash equally.
  fn hash<H: Hasher>(&self, state: &mut H) {
    for component in [self.lightness, self.chroma, self.hue, self.alpha] {
      state.write_u64(component.hash_bits());
    }
  }
}

/// The conversions between sRGB, Oklab and Oklch, through the pure-Rust `libm` so
/// every target computes the same bits; the arithmetic follows the `color` crate's
/// step for step.
mod convert {
  const SRGB_TO_LMS: [[f32; 3]; 3] = [
    [0.412_221_46, 0.536_332_55, 0.051_445_995],
    [0.211_903_5, 0.680_699_5, 0.107_396_96],
    [0.088_302_46, 0.281_718_85, 0.629_978_7],
  ];
  const LMS_TO_LAB: [[f32; 3]; 3] = [
    [0.210_454_26, 0.793_617_8, -0.004_072_047],
    [1.977_998_5, -2.428_592_2, 0.450_593_7],
    [0.025_904_037, 0.782_771_77, -0.808_675_77],
  ];
  const LAB_TO_LMS: [[f32; 3]; 3] = [
    [1.0, 0.396_337_78, 0.215_803_76],
    [1.0, -0.105_561_346, -0.063_854_17],
    [1.0, -0.089_484_18, -1.291_485_5],
  ];
  const LMS_TO_SRGB: [[f32; 3]; 3] = [
    [4.076_741_7, -3.307_711_6, 0.230_969_94],
    [-1.268_438, 2.609_757_4, -0.341_319_38],
    [-0.004_196_086_3, -0.703_418_6, 1.707_614_7],
  ];

  const fn matvecmul(matrix: &[[f32; 3]; 3], vector: [f32; 3]) -> [f32; 3] {
    [
      matrix[0][0] * vector[0] + matrix[0][1] * vector[1] + matrix[0][2] * vector[2],
      matrix[1][0] * vector[0] + matrix[1][1] * vector[1] + matrix[1][2] * vector[2],
      matrix[2][0] * vector[0] + matrix[2][1] * vector[1] + matrix[2][2] * vector[2],
    ]
  }

  /// One sRGB component to linear light.
  pub fn srgb_to_linear(component: f32) -> f32 {
    if component.abs() <= 0.04045 {
      component * (1.0 / 12.92)
    }
    else {
      libm::powf((component.abs() + 0.055) * (1.0 / 1.055), 2.4).copysign(component)
    }
  }

  /// One linear-light component to sRGB.
  pub fn linear_to_srgb(component: f32) -> f32 {
    if component.abs() <= 0.0031308 {
      component * 12.92
    }
    else {
      (1.055 * libm::powf(component.abs(), 1.0 / 2.4) - 0.055).copysign(component)
    }
  }

  /// Linear-light sRGB to Oklab.
  pub fn linear_to_oklab(linear: [f32; 3]) -> [f32; 3] {
    let lms = matvecmul(&SRGB_TO_LMS, linear).map(libm::cbrtf);

    matvecmul(&LMS_TO_LAB, lms)
  }

  /// Oklab to linear-light sRGB.
  pub fn oklab_to_linear(oklab: [f32; 3]) -> [f32; 3] {
    let lms = matvecmul(&LAB_TO_LMS, oklab).map(|component| component * component * component);

    matvecmul(&LMS_TO_SRGB, lms)
  }

  /// Oklab to Oklch, the hue in degrees from `0` up to `360`.
  pub fn oklab_to_oklch([lightness, a_axis, b_axis]: [f32; 3]) -> [f32; 3] {
    let mut hue = libm::atan2f(b_axis, a_axis).to_degrees();

    if hue < 0.0 {
      hue += 360.0;
    }

    [lightness, libm::hypotf(b_axis, a_axis), hue]
  }

  /// Oklch to Oklab.
  pub fn oklch_to_oklab([lightness, chroma, hue]: [f32; 3]) -> [f32; 3] {
    let radians = hue.to_radians();

    [
      lightness,
      chroma * libm::cosf(radians),
      chroma * libm::sinf(radians),
    ]
  }
}
