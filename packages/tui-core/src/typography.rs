//! The token set every run inherits its text properties through.

use skene_core::{Color, Media, TokenName, TokenSet};
use skene_views::typography;

/// A token's definition in cells; see [`skene_core::Definition`].
type Definition<T> = skene_core::Definition<T, u32>;
/// A token in cells; see [`skene_core::Token`].
type Token<T> = skene_core::Token<T, u32>;

/// The text inheritance: the inks, the selection colors and the weight every run reads
/// where it sets nothing of its own.
///
/// Defines the [`typography`] token names, so `Theme::new(Typography::default())` inks
/// everything beneath it, and a palette that defines the same names (as
/// `SimplePalette` does for the inks) takes over where it is nested. Every setter takes
/// a [`Definition`](skene_core::Definition), which carries its own conditions. The default follows the
/// terminal: a transparent ink shows the terminal's own foreground, transparent
/// selection colors paint the selection in reverse video, and the link ink answers the
/// scheme.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Typography {
  /// The text color.
  pub ink: Token<Color>,
  /// The text color of a link.
  pub link_ink: Token<Color>,
  /// The text color while selected.
  pub selection_ink: Token<Color>,
  /// The background behind selected text.
  pub selection_surface: Token<Color>,
  /// The font weight, `100` to `900`; `600` and above reads as bold.
  pub weight: Token<f32>,
}

impl Typography {
  /// Sets the text color.
  #[inline(always)]
  pub fn set_ink(mut self, ink: impl Into<Definition<Color>>) -> Self {
    self.ink = self.ink.redefine(ink);

    self
  }

  /// Sets the text color of a link.
  #[inline(always)]
  pub fn set_link_ink(mut self, ink: impl Into<Definition<Color>>) -> Self {
    self.link_ink = self.link_ink.redefine(ink);

    self
  }

  /// Sets the text color while selected.
  #[inline(always)]
  pub fn set_selection_ink(mut self, ink: impl Into<Definition<Color>>) -> Self {
    self.selection_ink = self.selection_ink.redefine(ink);

    self
  }

  /// Sets the background behind selected text.
  #[inline(always)]
  pub fn set_selection_surface(mut self, surface: impl Into<Definition<Color>>) -> Self {
    self.selection_surface = self.selection_surface.redefine(surface);

    self
  }

  /// Sets the font weight, `100` to `900`.
  #[inline(always)]
  pub fn set_weight(mut self, weight: impl Into<Definition<f32>>) -> Self {
    self.weight = self.weight.redefine(weight);

    self
  }
}

impl Default for Typography {
  /// The terminal's own foreground, a blue link ink answering the scheme, reverse-video
  /// selection, weight 400.
  fn default() -> Self {
    Self {
      ink: Token::new(typography::INK.name().as_str(), Color::TRANSPARENT),
      link_ink: Token::new(
        typography::LINK_INK.name().as_str(),
        Definition::from(Color::srgb(0., 0.4, 0.8)).when(Media::Dark, Color::srgb(0.4, 0.6, 1.)),
      ),
      selection_ink: Token::new(
        typography::SELECTION_INK.name().as_str(),
        Color::TRANSPARENT,
      ),
      selection_surface: Token::new(
        typography::SELECTION_SURFACE.name().as_str(),
        Color::TRANSPARENT,
      ),
      weight: Token::new(typography::WEIGHT.name().as_str(), 400.),
    }
  }
}

impl<V> TokenSet<V> for Typography
where
  V: From<Definition<Color>> + From<Definition<f32>> + 'static,
{
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![
      self.ink.define(),
      self.link_ink.define(),
      self.selection_ink.define(),
      self.selection_surface.define(),
      self.weight.define(),
    ]
  }
}
