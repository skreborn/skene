//! Tokens resolve where they are defined, as CSS computes a registered custom property:
//! a definition naming another token reads it at the defining scope, and a scope
//! beneath that redefines it changes nothing above. A token set read with no theme
//! above lays its defaults at the root once, and one read under a theme lays nothing.

use skene_simple_terminal::{glyph, *};
use skene_terminal::{
  self as terminal,
  Definition,
  Length,
  Scene,
  Terminal,
  Token,
  TokenName,
  TokenValue,
  prelude::*,
  testing,
};

use crate::common::{blue, colors_at, green, red, rgb, with_platform};

const SURFACE: TokenRef<Color> = TokenRef::new("surface");
const SHADE: TokenRef<Color> = TokenRef::new("shade");

/// A palette whose shade is defined through its surface.
#[derive(Debug, Clone, PartialEq)]
struct Outer {
  /// The surface.
  surface: Token<Color>,
  /// The surface, lightened.
  shade: Token<Color>,
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for Outer {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.surface.define(), self.shade.define()]
  }
}

/// A nested scope redefining the surface alone.
#[derive(Debug, Clone, PartialEq)]
struct Inner {
  /// The surface.
  surface: Token<Color>,
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for Inner {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.surface.define()]
  }
}

const CARD: TokenRef<Color> = TokenRef::new("card");
const GUTTER: TokenRef<Length> = TokenRef::new("gutter");
const ACCENT: TokenRef<Color> = TokenRef::new("accent");

/// A component library's palette: a card color and a gutter length.
#[derive(Debug, Clone, PartialEq)]
struct Cards {
  /// The card's surface.
  card: Token<Color>,
  /// The room around a card's content.
  gutter: Token<Length>,
}

impl Cards {
  fn with_card(card: Color) -> Self {
    Self {
      card: Token::new(CARD.name().as_str(), card),
      ..Self::default()
    }
  }
}

impl Default for Cards {
  fn default() -> Self {
    Self {
      card: Token::new(CARD.name().as_str(), green()),
      gutter: Token::new(GUTTER.name().as_str(), 1.),
    }
  }
}

impl<V: From<Definition<Color>> + From<Definition<Length>> + 'static> TokenSet<V> for Cards {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.card.define(), self.gutter.define()]
  }
}

/// A second token set: an accent color.
#[derive(Debug, Clone, PartialEq)]
struct Accents {
  /// The accent.
  accent: Token<Color>,
}

impl Default for Accents {
  fn default() -> Self {
    Self {
      accent: Token::new(ACCENT.name().as_str(), blue()),
    }
  }
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for Accents {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.accent.define()]
  }
}

/// How many tokens the root defines.
fn root_tokens() -> usize {
  Scene::<Terminal>::read(|scene| scene.root_definitions().len())
}

/// A set read with no theme above lays its defaults at the root once, however often it
/// is read; a second set lays its own beside them, and a box below no theme paints
/// both, its padding the default gutter.
#[test]
fn a_token_set_read_without_a_theme_defines_its_defaults_once() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      let before = root_tokens();
      let cards = Cards::current();

      assert_eq!(cards, Cards::default());
      assert_eq!(Cards::watch().get(), cards);
      assert_eq!(Cards::current(), cards);
      assert_eq!(root_tokens(), before + 2, "the set's two defaults, once");

      let _ = Accents::current();

      assert_eq!(root_tokens(), before + 3, "the second set's beside them");

      Container::new()
        .set_width(4)
        .set_height(3)
        .set_background(CARD)
        .set_padding(GUTTER)
        .set_child(Container::new().set_background(ACCENT))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 3, 0, 0).1, Some(rgb(green())), "the card");
    assert_eq!(colors_at(6, 3, 1, 1).1, Some(rgb(blue())), "the accent");
    assert_eq!(
      colors_at(6, 3, 0, 1).1,
      Some(rgb(green())),
      "a gutter's cell"
    );
    assert_eq!(colors_at(6, 3, 3, 1).1, Some(rgb(green())), "the other's");
  });
}

/// Under a theme the read is the theme's palette and nothing is laid at the root; a
/// box outside the theme paints nothing of the set until a later read outside it lays
/// the default.
#[test]
fn a_token_set_read_under_a_theme_defines_nothing() {
  with_platform(|| {
    let before = Signal::new(0);
    let _ = terminal::mount(move || {
      before.set(root_tokens());

      Column::new()
        .set_children((
          Theme::new(Cards::with_card(red())).set_child_builder(move || {
            assert_eq!(Cards::current(), Cards::with_card(red()));

            Container::new()
              .set_width(2)
              .set_height(1)
              .set_background(CARD)
          }),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(CARD),
        ))
        .into_any()
        .build()
    });

    assert_eq!(root_tokens(), before.get(), "nothing laid under the theme");
    assert_eq!(colors_at(4, 2, 0, 0).1, Some(rgb(red())));
    assert_eq!(
      colors_at(4, 2, 0, 1).1,
      None,
      "no card outside the theme yet"
    );

    let _ = Cards::current();

    assert_eq!(root_tokens(), before.get() + 2);
    assert_eq!(colors_at(4, 2, 0, 0).1, Some(rgb(red())));
    assert_eq!(colors_at(4, 2, 0, 1).1, Some(rgb(green())), "the default");
  });
}

/// The shade a nested scope reads is the outer palette's, computed from the outer
/// surface, though the nested scope redefines the surface.
#[test]
fn a_definition_reads_its_tokens_where_it_is_defined() {
  with_platform(|| {
    let blue = Color::srgb(0.2, 0.4, 0.8);
    let orange = Color::srgb(0.9, 0.5, 0.1);
    let _ = terminal::mount(move || {
      Theme::new(Outer {
        surface: Token::new(SURFACE.name().as_str(), blue),
        shade: Token::new(SHADE.name().as_str(), Expr::from(SURFACE).lighter(0.2)),
      })
      .set_child_builder(move || {
        Row::new().set_children((
          Theme::new(Inner {
            surface: Token::new(SURFACE.name().as_str(), orange),
          })
          .set_child_builder(|| {
            Container::new()
              .set_width(2)
              .set_height(1)
              .set_background(SHADE)
          }),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(Expr::from(blue).lighter(0.2)),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(Expr::from(orange).lighter(0.2)),
        ))
      })
      .into_any()
      .build()
    });

    let colors = testing::render_to_colors(6, 1);

    assert_eq!(colors[0].1, colors[2].1, "the outer surface, lightened");
    assert_ne!(colors[0].1, colors[4].1, "not the nested one");
  });
}

/// The glyph tokens are a token set: a theme redefines a control's marks beneath it,
/// and a node under the theme resolves the redefinition.
#[test]
fn a_theme_redefines_the_glyph_tokens() {
  with_platform(|| {
    let node = terminal::mount(|| {
      Theme::new(SimpleGlyphs::default().set_checkbox("X", "O"))
        .set_child_builder(|| Text::new("under"))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(8, 1);

    let (checked, unchecked) = Scene::<terminal::Terminal>::with(|scene| {
      let text = scene.children(node).next().expect("the themed text");

      (
        scene.resolve_token(text, glyph::CHECKBOX_CHECKED.name()),
        scene.resolve_token(text, glyph::CHECKBOX_UNCHECKED.name()),
      )
    });

    assert_eq!(
      checked,
      Some(TokenValue::Text(Definition::from("X".to_owned())))
    );
    assert_eq!(
      unchecked,
      Some(TokenValue::Text(Definition::from("O".to_owned())))
    );
  });
}
