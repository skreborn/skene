//! The token definitions the web accepts, and their custom-property registrations.

use skene_core::{
  Color,
  Dimension,
  next::{Definition, TokenName},
};

/// The erased token definition the web driver stores per token: a color, a number, or
/// a dimension.
///
/// Each kind registers its custom property with the matching `@property` syntax, so a
/// token interpolates and inherits as a typed value.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum WebTokenValue {
  /// A color token.
  Color(Definition<Color, f32>),
  /// A unitless number token; a property measured in pixels reads it as pixels.
  Number(Definition<f32, f32>),
  /// A dimension token.
  Dimension(Definition<Dimension<f32>, f32>),
}

impl WebTokenValue {
  /// The `@property` rule registering the token `name` with this kind's syntax.
  #[must_use]
  pub(crate) fn registration(&self, name: TokenName) -> String {
    let (syntax, initial) = match self {
      Self::Color(_) => ("<color>", "transparent"),
      Self::Number(_) => ("<number>", "0"),
      Self::Dimension(_) => ("<length-percentage> | auto", "0px"),
    };

    format!("@property --{name}{{syntax:\"{syntax}\";inherits:true;initial-value:{initial}}}")
  }
}

impl From<Definition<Color, f32>> for WebTokenValue {
  #[inline(always)]
  fn from(definition: Definition<Color, f32>) -> Self {
    Self::Color(definition)
  }
}

impl From<Definition<f32, f32>> for WebTokenValue {
  #[inline(always)]
  fn from(definition: Definition<f32, f32>) -> Self {
    Self::Number(definition)
  }
}

impl From<Definition<Dimension<f32>, f32>> for WebTokenValue {
  #[inline(always)]
  fn from(definition: Definition<Dimension<f32>, f32>) -> Self {
    Self::Dimension(definition)
  }
}
