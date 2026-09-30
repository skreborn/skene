//! Tokens resolve where they are defined, as CSS computes a registered custom property:
//! a definition naming another token reads it at the defining scope, and a scope
//! beneath that redefines it changes nothing above.

use skene_simple::tui::{glyph, *};
use skene_tui::{
  self as tui,
  Definition,
  Scene,
  Token,
  TokenName,
  TokenValue,
  prelude::*,
  testing,
};

use crate::common::with_platform;

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

/// The shade a nested scope reads is the outer palette's, computed from the outer
/// surface, though the nested scope redefines the surface.
#[test]
fn a_definition_reads_its_tokens_where_it_is_defined() {
  with_platform(|| {
    let blue = Color::srgb(0.2, 0.4, 0.8);
    let orange = Color::srgb(0.9, 0.5, 0.1);
    let _ = tui::mount(move || {
      Theme::new(Outer {
        surface: Token::new(SURFACE.name().as_str(), blue),
        shade: Token::new(SHADE.name().as_str(), Expr::from(SURFACE).lighter(0.2)),
      })
      .set_child_builder(move || {
        Row::new().set_children((
          Theme::new(Inner {
            surface: Token::new(SURFACE.name().as_str(), orange),
          })
          .set_child_builder(|| Container::new().set_width(2).set_height(1).set_color(SHADE)),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_color(Expr::from(blue).lighter(0.2)),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_color(Expr::from(orange).lighter(0.2)),
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
    let node = tui::mount(|| {
      Theme::new(SimpleGlyphs::default().set_checkbox("X", "O"))
        .set_child_builder(|| Text::new("under"))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(8, 1);

    let (checked, unchecked) = Scene::<tui::Tui>::with(|scene| {
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
