//! What the view tests share: a component library's palette, a typography set, and
//! the property keys the seams are asked about.

use skene_common::{typography, web::WebProperty};
use skene_core::{
  Color,
  Length,
  Media,
  NodeId,
  Property,
  PropertyKey,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
};
use skene_headless::{TokenValue, children};

pub(crate) const COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Color);
pub(crate) const TEXT_COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::TextColor);
pub(crate) const WIDTH: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Width);
pub(crate) const PADDING: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Padding);
pub(crate) const SPACING: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Spacing);

pub(crate) const WHITE: Color = Color::TRANSPARENT;

pub(crate) fn red() -> Color {
  Color::srgb(1., 0., 0.)
}

pub(crate) fn green() -> Color {
  Color::srgb(0., 1., 0.)
}

pub(crate) fn blue() -> Color {
  Color::srgb(0., 0., 1.)
}

pub(crate) fn black() -> Color {
  Color::srgb(0., 0., 0.)
}

/// A component library's palette: a card color and a gutter length.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TestPalette {
  pub card: Token<Color, f32>,
  pub gutter: Token<Length, f32>,
}

impl TestPalette {
  pub(crate) const CARD: TokenRef<Color> = TokenRef::new("card");
  pub(crate) const GUTTER: TokenRef<Length> = TokenRef::new("gutter");

  pub(crate) fn with_card(card: Color) -> Self {
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
      gutter: Token::new("gutter", Length(8.)),
    }
  }
}

impl TokenSet<TokenValue> for TestPalette {
  fn definitions(&self) -> Vec<(TokenName, TokenValue)> {
    vec![self.card.define(), self.gutter.define()]
  }
}

/// A facade's typography: the tokens every run inherits through.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TestTypography {
  pub ink: Token<Color, f32>,
  pub link_ink: Token<Color, f32>,
  pub selection_ink: Token<Color, f32>,
  pub selection_surface: Token<Color, f32>,
}

impl TestTypography {
  pub(crate) fn with_ink(ink: Color) -> Self {
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

impl TokenSet<TokenValue> for TestTypography {
  fn definitions(&self) -> Vec<(TokenName, TokenValue)> {
    vec![
      self.ink.define(),
      self.link_ink.define(),
      self.selection_ink.define(),
      self.selection_surface.define(),
    ]
  }
}

/// The content between a container's anchors: its one child.
pub(crate) fn only_child(node: NodeId) -> NodeId {
  children(node)[1]
}
