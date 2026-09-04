//! What the view tests share: a component library's palette, a typography set, and
//! the property keys the seams are asked about.

#![allow(dead_code)]

use skene_core::{
  Color,
  Spacing,
  next::{
    Length,
    Media,
    NoProperty,
    NodeId,
    Property,
    PropertyKey,
    Token,
    TokenName,
    TokenRef,
    TokenSet,
  },
};
use skene_headless::{HeadlessTokenValue, children};
use skene_views::typography;

pub const COLOR: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Color);
pub const TEXT_COLOR: PropertyKey<NoProperty> = PropertyKey::Shared(Property::TextColor);
pub const WIDTH: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Width);
pub const PADDING: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Padding);
pub const SPACING: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Spacing);
pub const ASPECT_RATIO: PropertyKey<NoProperty> = PropertyKey::Shared(Property::AspectRatio);

pub const WHITE: Color = Color::TRANSPARENT;

pub fn red() -> Color {
  Color::srgb(1., 0., 0.)
}

pub fn green() -> Color {
  Color::srgb(0., 1., 0.)
}

pub fn blue() -> Color {
  Color::srgb(0., 0., 1.)
}

pub fn black() -> Color {
  Color::srgb(0., 0., 0.)
}

/// A component library's palette: a card color and a gutter length.
#[derive(Debug, Clone, PartialEq)]
pub struct TestPalette {
  pub card: Token<Color, f32>,
  pub gutter: Token<f32, f32>,
}

impl TestPalette {
  pub const CARD: TokenRef<Color> = TokenRef::new("card");
  pub const GUTTER: TokenRef<f32> = TokenRef::new("gutter");
  /// The gutter as an expression reads it.
  pub const GUTTER_LENGTH: TokenRef<Length> = TokenRef::new("gutter");
  /// The gutter as the spacing property reads it.
  pub const GUTTER_SPACING: TokenRef<Spacing<f32>> = TokenRef::new("gutter");

  pub fn with_card(card: Color) -> Self {
    Self {
      card: Token::new("card", card),
      ..Self::default()
    }
  }
}

impl Default for TestPalette {
  fn default() -> Self {
    Self {
      card: Token::new("card", WHITE).define_when(Media::Dark, black()),
      gutter: Token::new("gutter", 8.),
    }
  }
}

impl TokenSet<HeadlessTokenValue> for TestPalette {
  fn definitions(&self) -> Vec<(TokenName, HeadlessTokenValue)> {
    vec![self.card.define(), self.gutter.define()]
  }
}

/// A facade's typography: the tokens every run inherits through.
#[derive(Debug, Clone, PartialEq)]
pub struct TestTypography {
  pub ink: Token<Color, f32>,
  pub link_ink: Token<Color, f32>,
  pub selection_ink: Token<Color, f32>,
  pub selection_surface: Token<Color, f32>,
}

impl TestTypography {
  pub fn with_ink(ink: Color) -> Self {
    Self {
      ink: Token::new(typography::INK.name().as_str(), ink),
      ..Self::default()
    }
  }
}

impl Default for TestTypography {
  fn default() -> Self {
    Self {
      ink: Token::new(typography::INK.name().as_str(), black()),
      link_ink: Token::new(typography::LINK_INK.name().as_str(), blue()),
      selection_ink: Token::new(typography::SELECTION_INK.name().as_str(), WHITE),
      selection_surface: Token::new(typography::SELECTION_SURFACE.name().as_str(), green()),
    }
  }
}

impl TokenSet<HeadlessTokenValue> for TestTypography {
  fn definitions(&self) -> Vec<(TokenName, HeadlessTokenValue)> {
    vec![
      self.ink.define(),
      self.link_ink.define(),
      self.selection_ink.define(),
      self.selection_surface.define(),
    ]
  }
}

/// The content between a container's anchors: its one child.
pub fn only_child(node: NodeId) -> NodeId {
  children(node)[1]
}
