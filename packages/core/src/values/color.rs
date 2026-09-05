use std::hash::{Hash, Hasher};

use color::{AlphaColor, ColorSpace, Hsl, Hwb, Lab, Lch, LinearSrgb, Oklab, Oklch, Srgb};
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

  #[inline(always)]
  fn from_space(value: AlphaColor<impl ColorSpace>) -> Self {
    let [lightness, chroma, hue, alpha] = AlphaColor::convert::<Oklch>(value).components;

    Self {
      lightness,
      chroma,
      hue,
      alpha,
    }
  }

  #[inline(always)]
  fn to_space<S: ColorSpace>(self) -> AlphaColor<S> {
    AlphaColor::<Oklch>::new([self.lightness, self.chroma, self.hue, self.alpha]).convert::<S>()
  }

  /// The color as an sRGB alpha color, for a renderer.
  #[inline(always)]
  #[must_use]
  pub fn to_alpha_color(self) -> AlphaColor<Srgb> {
    self.to_space::<Srgb>()
  }

  /// Creates an opaque color in the sRGB color space.
  ///
  /// Components on a scale of `0` to `1`.
  #[inline(always)]
  pub fn srgb(red: f32, green: f32, blue: f32) -> Self {
    Self::from_space(AlphaColor::<Srgb>::new([red, green, blue, 1.]))
  }

  /// Creates an opaque color in the linear-light sRGB color space.
  ///
  /// Components on a scale of `0` to `1`.
  #[inline(always)]
  pub fn linear_srgb(red: f32, green: f32, blue: f32) -> Self {
    Self::from_space(AlphaColor::<LinearSrgb>::new([red, green, blue, 1.]))
  }

  /// Creates an opaque color in the HSL color space.
  ///
  /// `hue` in degrees; `saturation` and `lightness` on a scale of `0` to `100`.
  #[inline(always)]
  pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Self {
    Self::from_space(AlphaColor::<Hsl>::new([hue, saturation, lightness, 1.]))
  }

  /// Creates an opaque color in the HWB color space.
  ///
  /// `hue` in degrees; `whiteness` and `blackness` on a scale of `0` to `100`.
  #[inline(always)]
  pub fn hwb(hue: f32, whiteness: f32, blackness: f32) -> Self {
    Self::from_space(AlphaColor::<Hwb>::new([hue, whiteness, blackness, 1.]))
  }

  /// Creates an opaque color in the CIE Lab color space.
  ///
  /// `lightness` on a scale of `0` to `100`.
  #[inline(always)]
  pub fn lab(lightness: f32, a_axis: f32, b_axis: f32) -> Self {
    Self::from_space(AlphaColor::<Lab>::new([lightness, a_axis, b_axis, 1.]))
  }

  /// Creates an opaque color in the CIE `LCh` color space.
  ///
  /// `lightness` on a scale of `0` to `100` and `hue` in degrees.
  #[inline(always)]
  pub fn lch(lightness: f32, chroma: f32, hue: f32) -> Self {
    Self::from_space(AlphaColor::<Lch>::new([lightness, chroma, hue, 1.]))
  }

  /// Creates an opaque color in the Oklab color space.
  ///
  /// `lightness` on a scale of `0` to `1`.
  #[inline(always)]
  pub fn oklab(lightness: f32, a_axis: f32, b_axis: f32) -> Self {
    Self::from_space(AlphaColor::<Oklab>::new([lightness, a_axis, b_axis, 1.]))
  }

  /// Creates an opaque color in the Oklch color space.
  ///
  /// `lightness` on a scale of `0` to `1` and `hue` in degrees.
  #[inline(always)]
  pub fn oklch(lightness: f32, chroma: f32, hue: f32) -> Self {
    Self::from_space(AlphaColor::<Oklch>::new([lightness, chroma, hue, 1.]))
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

    Self::from_space(AlphaColor::<Oklab>::new(std::array::from_fn(|component| {
      from[component] + (to[component] - from[component]) * amount
    })))
  }

  /// This color's `[red, green, blue, alpha]` components in the sRGB color space.
  #[inline(always)]
  #[must_use]
  pub fn to_rgba(self) -> [f32; 4] {
    self.to_space::<Srgb>().components
  }

  /// This color's `[red, green, blue, alpha]` components in the linear-light sRGB color space.
  #[inline(always)]
  #[must_use]
  pub fn to_linear_srgb(self) -> [f32; 4] {
    self.to_space::<LinearSrgb>().components
  }

  /// This color's `[hue, saturation, lightness, alpha]` components in the HSL color space.
  #[inline(always)]
  #[must_use]
  pub fn to_hsl(self) -> [f32; 4] {
    self.to_space::<Hsl>().components
  }

  /// This color's `[hue, whiteness, blackness, alpha]` components in the HWB color space.
  #[inline(always)]
  #[must_use]
  pub fn to_hwb(self) -> [f32; 4] {
    self.to_space::<Hwb>().components
  }

  /// This color's `[lightness, axis_a, axis_b, alpha]` components in the CIE Lab color space.
  #[inline(always)]
  #[must_use]
  pub fn to_lab(self) -> [f32; 4] {
    self.to_space::<Lab>().components
  }

  /// This color's `[lightness, chroma, hue, alpha]` components in the CIE `LCh` color space.
  #[inline(always)]
  #[must_use]
  pub fn to_lch(self) -> [f32; 4] {
    self.to_space::<Lch>().components
  }

  /// This color's `[lightness, axis_a, axis_b, alpha]` components in the Oklab color space.
  #[inline(always)]
  #[must_use]
  pub fn to_oklab(self) -> [f32; 4] {
    self.to_space::<Oklab>().components
  }

  /// This color's `[lightness, chroma, hue, alpha]` components in the Oklch color space.
  #[inline(always)]
  #[must_use]
  pub fn to_oklch(self) -> [f32; 4] {
    self.to_space::<Oklch>().components
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
