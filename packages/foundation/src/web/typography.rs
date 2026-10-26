//! The token set every run inherits its text properties through.

use crate::{Color, Length, Media, TokenName, TokenSet, typography};

/// A token's definition in logical pixels; see [`crate::Definition`].
type Definition<T> = crate::Definition<T, f32>;
/// A token in logical pixels; see [`crate::Token`].
type Token<T> = crate::Token<T, f32>;

/// The text inheritance: the inks, the selection colors and the type metrics every
/// run reads where it sets nothing of its own.
///
/// Defines the [`typography`] token names, so `Theme::new(Typography::default())`
/// inks and sizes everything beneath it, and a palette that defines the same names
/// (as `SimplePalette` does for the inks) takes over where it is nested. Every setter
/// takes a [`Definition`](crate::Definition), which carries its own conditions; the
/// default's ink answers the dark scheme that way.
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
  /// The font size, in logical pixels.
  pub size: Token<Length>,
  /// The font weight, `100` to `900`.
  pub weight: Token<f32>,
  /// The line height, as a multiple of the font size.
  pub line_height: Token<f32>,
  /// The extra space between letters, in logical pixels.
  pub letter_spacing: Token<Length>,
}

impl Typography {
  /// Sets the text color.
  pub fn set_ink(mut self, ink: impl Into<Definition<Color>>) -> Self {
    self.ink = self.ink.redefine(ink);

    self
  }

  /// Sets the text color of a link.
  pub fn set_link_ink(mut self, ink: impl Into<Definition<Color>>) -> Self {
    self.link_ink = self.link_ink.redefine(ink);

    self
  }

  /// Sets the text color while selected.
  pub fn set_selection_ink(mut self, ink: impl Into<Definition<Color>>) -> Self {
    self.selection_ink = self.selection_ink.redefine(ink);

    self
  }

  /// Sets the background behind selected text.
  pub fn set_selection_surface(mut self, surface: impl Into<Definition<Color>>) -> Self {
    self.selection_surface = self.selection_surface.redefine(surface);

    self
  }

  /// Sets the font size, in logical pixels.
  pub fn set_size(mut self, size: impl Into<Definition<Length>>) -> Self {
    self.size = self.size.redefine(size);

    self
  }

  /// Sets the font weight, `100` to `900`.
  pub fn set_weight(mut self, weight: impl Into<Definition<f32>>) -> Self {
    self.weight = self.weight.redefine(weight);

    self
  }

  /// Sets the line height, as a multiple of the font size.
  pub fn set_line_height(mut self, line_height: impl Into<Definition<f32>>) -> Self {
    self.line_height = self.line_height.redefine(line_height);

    self
  }

  /// Sets the extra space between letters, in logical pixels.
  pub fn set_letter_spacing(mut self, letter_spacing: impl Into<Definition<Length>>) -> Self {
    self.letter_spacing = self.letter_spacing.redefine(letter_spacing);

    self
  }
}

/// A definition taking `dark`'s value under the dark scheme and `light`'s otherwise.
fn pair(light: Color, dark: Color) -> Definition<Color> {
  Definition::from(light).when(Media::Dark, dark)
}

impl Default for Typography {
  /// A near-black ink turning near-white under the dark scheme, a blue link, a blue
  /// selection, 16 pixels at weight 400 with a line height of 1.5.
  fn default() -> Self {
    Self {
      ink: Token::new(
        typography::INK,
        pair(Color::srgb(0.09, 0.09, 0.11), Color::srgb(0.93, 0.93, 0.95)),
      ),
      link_ink: Token::new(
        typography::LINK_INK,
        pair(Color::srgb(0.15, 0.39, 0.92), Color::srgb(0.55, 0.7, 1.)),
      ),
      selection_ink: Token::new(
        typography::SELECTION_INK,
        pair(Color::srgb(1., 1., 1.), Color::srgb(0.06, 0.06, 0.08)),
      ),
      selection_surface: Token::new(
        typography::SELECTION_SURFACE,
        pair(Color::srgb(0.22, 0.47, 0.95), Color::srgb(0.45, 0.62, 0.98)),
      ),
      size: Token::new(typography::SIZE, 16.),
      weight: Token::new(typography::WEIGHT, 400.),
      line_height: Token::new(typography::LINE_HEIGHT, 1.5),
      letter_spacing: Token::new(typography::LETTER_SPACING, 0.),
    }
  }
}

impl<V> TokenSet<V> for Typography
where
  V: From<Definition<Color>> + From<Definition<f32>> + From<Definition<Length>> + 'static,
{
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![
      self.ink.define(),
      self.link_ink.define(),
      self.selection_ink.define(),
      self.selection_surface.define(),
      self.size.define(),
      self.weight.define(),
      self.line_height.define(),
      self.letter_spacing.define(),
    ]
  }
}
