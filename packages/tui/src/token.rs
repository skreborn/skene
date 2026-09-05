//! The erased token definition the terminal stores per token.

use skene_core::{Color, Dimension, next::Definition};

/// The erased token definition the terminal accepts: a color, a number, a dimension in
/// cells, or a text such as a control's glyph.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum TokenValue {
  /// A color token.
  Color(Definition<Color, u32>),
  /// A unitless number token.
  Number(Definition<f32, u32>),
  /// A dimension token, in cells.
  Dimension(Definition<Dimension<u32>, u32>),
  /// A text token.
  Text(Definition<String, u32>),
}

impl From<Definition<Color, u32>> for TokenValue {
  #[inline(always)]
  fn from(definition: Definition<Color, u32>) -> Self {
    Self::Color(definition)
  }
}

impl From<Definition<f32, u32>> for TokenValue {
  #[inline(always)]
  fn from(definition: Definition<f32, u32>) -> Self {
    Self::Number(definition)
  }
}

impl From<Definition<Dimension<u32>, u32>> for TokenValue {
  #[inline(always)]
  fn from(definition: Definition<Dimension<u32>, u32>) -> Self {
    Self::Dimension(definition)
  }
}

impl From<Definition<String, u32>> for TokenValue {
  #[inline(always)]
  fn from(definition: Definition<String, u32>) -> Self {
    Self::Text(definition)
  }
}
