//! Tokens resolve where they are defined, as CSS computes a registered custom property:
//! a definition naming another token reads it at the defining scope, and a scope
//! beneath that redefines it changes nothing above. A token set read with no theme
//! above lays its defaults at the root once, and one read under a theme lays nothing.
//! A linked palette's default is laid at the root once, the first time a node names one
//! of its tokens, under the claim a read of the set makes, and a theme of it still wins
//! beneath it, laying no palette whose tokens it defines itself. The nearest theme's palette
//! answers, as the tree stands after a box moves, an expression or a derived leaf over a token
//! resolves through it, and the token answers the scheme where it is read. The platform's colors at
//! the root follow what the terminal reports and the app's scheme.

use std::{any::TypeId, cell::Cell, rc::Rc};

use skene_simple_tui::{glyph, *};
use skene_tui::{
  Definition,
  Definitions,
  Length,
  Scene,
  StaticLeaf,
  Token,
  TokenName,
  TokenScope,
  TokenValue,
  platform::terminal::mount,
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
      card: Token::new(CARD, card),
      ..Self::default()
    }
  }
}

impl Default for Cards {
  fn default() -> Self {
    Self {
      card: Token::new(CARD, green()),
      gutter: Token::new(GUTTER, 1.),
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
      accent: Token::new(ACCENT, blue()),
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
  Scene::read(|scene| scene.root_definitions().len())
}

/// A set read with no theme above lays its defaults at the root once, however often it
/// is read; a second set lays its own beside them, and a box below no theme paints
/// both, its padding the default gutter.
#[test]
fn a_token_set_read_without_a_theme_defines_its_defaults_once() {
  with_platform(|| {
    let _ = mount(|| {
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
    let _ = mount(move || {
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

/// A component library's palette whose references carry its default: a surface and an
/// ink.
#[derive(Debug, Clone, PartialEq)]
struct Linked {
  /// The surface.
  surface: Token<Color>,
  /// The ink.
  ink: Token<Color>,
}

impl Linked {
  const INK: TokenRef<Color> = TokenRef::of::<Self, i32>("linked-ink");
  const SURFACE: TokenRef<Color> = TokenRef::of::<Self, i32>("linked-surface");

  fn with_surface(surface: Color) -> Self {
    Self {
      surface: Token::new(Self::SURFACE, surface),
      ..Self::default()
    }
  }
}

impl Default for Linked {
  fn default() -> Self {
    Self {
      surface: Token::new(Self::SURFACE, green()),
      ink: Token::new(Self::INK, blue()),
    }
  }
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for Linked {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.surface.define(), self.ink.define()]
  }
}

/// A second linked palette whose one token aliases the first palette's ink.
#[derive(Debug, Clone, PartialEq)]
struct Aliasing {
  /// The first palette's ink, by reference.
  tint: Token<Color>,
}

impl Aliasing {
  const TINT: TokenRef<Color> = TokenRef::of::<Self, i32>("aliasing-tint");
}

impl Default for Aliasing {
  fn default() -> Self {
    Self {
      tint: Token::new(Self::TINT, Linked::INK),
    }
  }
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for Aliasing {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.tint.define()]
  }
}

/// A linked palette whose one token aliases its own surface, as Simple's typography
/// aliases its foreground.
#[derive(Debug, Clone, PartialEq)]
struct SelfAliasing {
  /// The surface.
  surface: Token<Color>,
  /// The surface, by reference.
  tint: Token<Color>,
}

impl SelfAliasing {
  const SURFACE: TokenRef<Color> = TokenRef::of::<Self, i32>("self-aliasing-surface");
  const TINT: TokenRef<Color> = TokenRef::of::<Self, i32>("self-aliasing-tint");
}

impl Default for SelfAliasing {
  fn default() -> Self {
    Self {
      surface: Token::new(Self::SURFACE, green()),
      tint: Token::new(Self::TINT, Self::SURFACE),
    }
  }
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for SelfAliasing {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.surface.define(), self.tint.define()]
  }
}

/// Whether the root defines `reference`.
fn root_defines(reference: TokenRef<Color>) -> bool {
  Scene::read(|scene| scene.root_definitions().get(reference.name()).is_some())
}

/// Whether the root's definitions of [`Linked`] are claimed, by a read or a reference.
fn linked_claimed() -> bool {
  Scene::read(|scene| scene.defines_root_set(TypeId::of::<Linked>()))
}

/// A box below no theme naming a linked token paints the palette's default, with no
/// read of the palette.
#[test]
fn a_linked_token_paints_its_palettes_default_below_no_theme() {
  with_platform(|| {
    let _ = mount(|| {
      Container::new()
        .set_width(2)
        .set_height(1)
        .set_background(Linked::SURFACE)
        .into_any()
        .build()
    });

    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(green())), "the default");
    assert!(root_defines(Linked::INK), "the whole palette");
  });
}

/// A theme of a linked palette still wins beneath it, the default answering outside it.
#[test]
fn a_theme_of_a_linked_palette_wins_over_its_default() {
  with_platform(|| {
    let _ = mount(|| {
      Column::new()
        .set_children((
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(Linked::SURFACE),
          Theme::new(Linked::with_surface(red())).set_child_builder(|| {
            Container::new()
              .set_width(2)
              .set_height(1)
              .set_background(Linked::SURFACE)
          }),
        ))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(2, 2, 0, 0).1, Some(rgb(green())), "the default");
    assert_eq!(colors_at(2, 2, 0, 1).1, Some(rgb(red())), "the theme's");
  });
}

/// However many nodes name a linked palette's tokens, its default is laid at the root
/// once.
#[test]
fn many_references_lay_a_palette_once() {
  with_platform(|| {
    let before = Signal::new(0);
    let _ = mount(move || {
      before.set(root_tokens());

      Column::new()
        .set_children(
          (0..8)
            .map(|index| {
              Container::new()
                .set_width(2)
                .set_height(1)
                .set_background(if index % 2 == 0 {
                  Linked::SURFACE
                }
                else {
                  Linked::INK
                })
                .set_child(Text::new("x").set_color(Linked::INK))
                .into_any()
            })
            .collect::<Vec<_>>(),
        )
        .into_any()
        .build()
    });

    assert_eq!(root_tokens(), before.get() + 2, "the palette's two, once");
    assert_eq!(colors_at(2, 8, 0, 1).1, Some(rgb(blue())));
  });
}

/// A read of a linked palette and a reference to one of its tokens lay its default under
/// one claim, whichever comes first.
#[test]
fn a_read_and_a_reference_lay_a_linked_palette_once() {
  with_platform(|| {
    let before = Signal::new(0);
    let _ = mount(move || {
      before.set(root_tokens());

      Container::new()
        .set_width(2)
        .set_height(1)
        .set_background(Linked::SURFACE)
        .into_any()
        .build()
    });

    assert!(
      linked_claimed(),
      "the reference claims the set a read claims"
    );
    assert_eq!(Linked::current(), Linked::default());
    assert_eq!(
      root_tokens(),
      before.get() + 2,
      "the read lays nothing more"
    );
  });

  with_platform(|| {
    let before = root_tokens();

    assert_eq!(Linked::current(), Linked::default());
    assert!(linked_claimed());
    assert_eq!(root_tokens(), before + 2, "the read lays the default");

    let _ = mount(|| {
      Container::new()
        .set_width(2)
        .set_height(1)
        .set_background(Linked::INK)
        .into_any()
        .build()
    });

    assert_eq!(root_tokens(), before + 2, "the reference lays nothing more");
    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(blue())));
  });
}

/// A mount naming none of a linked palette's tokens leaves the root without them.
#[test]
fn an_unreferenced_palette_lays_nothing() {
  with_platform(|| {
    let _ = mount(|| {
      Container::new()
        .set_width(2)
        .set_height(1)
        .set_background(red())
        .into_any()
        .build()
    });

    assert!(!root_defines(Linked::SURFACE));
    assert!(!root_defines(Linked::INK));
  });
}

/// A derived leaf switching to a linked token lays the palette's default on that write.
#[test]
fn a_derived_leaf_naming_a_linked_token_lays_its_default() {
  with_platform(|| {
    let linked = Signal::new(false);
    let _ = mount(move || {
      Container::new()
        .set_width(2)
        .set_height(1)
        .derive_background(move || {
          if linked.get() {
            StaticLeaf::from(Linked::SURFACE)
          }
          else {
            StaticLeaf::Fixed(red())
          }
        })
        .into_any()
        .build()
    });

    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(red())));
    assert!(!root_defines(Linked::SURFACE), "nothing named it yet");

    linked.set(true);

    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(green())), "the default");
    assert!(root_defines(Linked::SURFACE));
  });
}

/// A linked palette whose default names another linked palette's token lays both.
#[test]
fn a_default_aliasing_another_palette_lays_both() {
  with_platform(|| {
    let before = Signal::new(0);
    let _ = mount(move || {
      before.set(root_tokens());

      Container::new()
        .set_width(2)
        .set_height(1)
        .set_background(Aliasing::TINT)
        .into_any()
        .build()
    });

    assert_eq!(root_tokens(), before.get() + 3, "the alias and the palette");
    assert!(root_defines(Linked::INK));
    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(blue())), "the ink");
  });
}

/// A theme of a linked palette aliasing its own tokens lays nothing at the root, so a
/// text outside it keeps its ink, where a theme aliasing another palette's token lays
/// that palette.
#[test]
fn a_theme_lays_only_the_palettes_it_leaves_undefined() {
  with_platform(|| {
    let before = Signal::new(0);
    let _ = mount(move || {
      before.set(root_tokens());

      Column::new()
        .set_children((
          Text::new("x"),
          Theme::new(SelfAliasing::default()).set_child_builder(|| Text::new("y")),
        ))
        .into_any()
        .build()
    });

    assert_eq!(root_tokens(), before.get(), "nothing laid at the root");
    assert!(!Scene::read(
      |scene| scene.defines_root_set(TypeId::of::<SelfAliasing>())
    ));
    assert!(!root_defines(SelfAliasing::TINT));
  });

  with_platform(|| {
    let before = Signal::new(0);
    let _ = mount(move || {
      before.set(root_tokens());

      Theme::new(Aliasing::default())
        .set_child_builder(|| Text::new("x"))
        .into_any()
        .build()
    });

    assert!(linked_claimed(), "the aliased palette");
    assert!(root_defines(Linked::INK));
    assert!(!root_defines(Aliasing::TINT), "the theme's own scope alone");
    assert_eq!(root_tokens(), before.get() + 2);
  });
}

/// The shade a nested scope reads is the outer palette's, computed from the outer
/// surface, though the nested scope redefines the surface.
#[test]
fn a_definition_reads_its_tokens_where_it_is_defined() {
  with_platform(|| {
    let blue = Color::srgb(0.2, 0.4, 0.8);
    let orange = Color::srgb(0.9, 0.5, 0.1);
    let _ = mount(move || {
      Theme::new(Outer {
        surface: Token::new(SURFACE, blue),
        shade: Token::new(SHADE, Expr::from(SURFACE).lighter(0.2)),
      })
      .set_child_builder(move || {
        Row::new().set_children((
          Theme::new(Inner {
            surface: Token::new(SURFACE, orange),
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
    let node = mount(|| {
      Theme::new(SimpleGlyphs::default().set_checkbox("X", "O"))
        .set_child_builder(|| Text::new("under"))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(8, 1);

    let (checked, unchecked) = Scene::with(|scene| {
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

/// A box under a nested theme paints the inner palette's token, one under the outer
/// theme alone the outer's, and a read under the nested theme is the inner palette.
#[test]
fn nested_themes_resolve_the_nearest_palette() {
  with_platform(|| {
    let seen = Rc::new(Cell::new(None::<Cards>));

    let _ = mount({
      let seen = Rc::clone(&seen);

      move || {
        Theme::new(Cards::default())
          .set_child_builder(move || {
            Column::new().set_children((
              Container::new()
                .set_width(2)
                .set_height(1)
                .set_background(CARD),
              Theme::new(Cards::with_card(red())).set_child_builder(move || {
                seen.set(Some(Cards::current()));

                Container::new()
                  .set_width(2)
                  .set_height(1)
                  .set_background(CARD)
              }),
            ))
          })
          .into_any()
          .build()
      }
    });

    assert_eq!(
      colors_at(4, 2, 0, 0).1,
      Some(rgb(green())),
      "the outer card"
    );
    assert_eq!(colors_at(4, 2, 0, 1).1, Some(rgb(red())), "the inner card");
    assert_eq!(seen.take(), Some(Cards::with_card(red())));
  });
}

/// A token resolves to its nearest definer as the tree stands now, whatever a node
/// looked up before: a box taken out of its theme falls back to the root, put back it
/// reads the theme again, and a scope defined between them answers in the theme's place.
#[test]
fn a_token_resolves_to_its_nearest_definer_as_the_tree_stands() {
  with_platform(|| {
    let theme = mount(|| {
      Theme::new(Cards::with_card(red()))
        .set_child_builder(|| {
          Column::new().set_children((Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(CARD)
            .into_any(),))
        })
        .into_any()
        .build()
    });

    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(red())));

    let color = |color: Color| Some(TokenValue::Color(Definition::from(color)));

    Scene::with(|scene| {
      let column = scene.children(theme).next().expect("the themed column");
      let leaf = scene.children(column).next().expect("the box");
      let root = scene.root_definitions().get(CARD.name()).cloned();

      assert_eq!(scene.resolve_token(leaf, CARD.name()), color(red()));

      scene.detach(column, leaf);

      assert_eq!(
        scene.resolve_token(leaf, CARD.name()),
        root,
        "out of the theme"
      );

      scene.attach(column, leaf, None);

      assert_eq!(
        scene.resolve_token(leaf, CARD.name()),
        color(red()),
        "back under it"
      );

      scene.define_tokens(
        TokenScope::Node(column),
        Definitions::from(vec![(
          CARD.name(),
          TokenValue::Color(Definition::from(blue())),
        )]),
      );

      assert_eq!(
        scene.resolve_token(leaf, CARD.name()),
        color(blue()),
        "under a nearer definer"
      );
    });

    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(blue())));
  });
}

/// An expression over a token resolves through the token's definition, and a length
/// expression over the viewport follows the terminal's size, as the signal does.
#[test]
fn expressions_resolve_over_tokens_and_the_environment() {
  with_platform(|| {
    let _ = mount(|| {
      Theme::new(Cards::with_card(blue()))
        .set_child_builder(|| {
          Column::new().set_children((
            Container::new()
              .set_width(Expr::<Length>::viewport_width(0.5))
              .set_height(1)
              .set_background(Expr::from(CARD).mix(red(), 0.5)),
            Container::new()
              .set_width(2)
              .set_height(1)
              .set_background(Expr::from(blue()).mix(red(), 0.5)),
          ))
        })
        .into_any()
        .build()
    });
    let colors = testing::render_to_colors(20, 2);

    assert_eq!(colors[0].1, colors[20].1, "the card, mixed");
    assert_ne!(colors[0].1, Some(rgb(blue())));
    assert_eq!(colors[9].1, colors[0].1, "half the terminal's width");
    assert_eq!(colors[10].1, None);

    testing::trigger_resize(10, 2);

    let colors = testing::render_to_colors(10, 2);

    assert_eq!(colors[4].1, colors[10].1);
    assert_eq!(colors[5].1, None, "half the narrower terminal's width");
    assert_eq!(
      skene_tui::media::viewport().get(),
      skene_foundation::Size::new(10, 2)
    );
  });
}

/// A derived leaf may name a token or an expression over one: the box paints what the
/// leaf names through the theme in force, a write switches between them, and the
/// token answers the scheme where the leaf is read.
#[test]
fn a_derived_leaf_names_a_token_or_an_expression_through_the_theme() {
  with_platform(|| {
    let muted = Signal::new(false);

    testing::trigger_color_scheme(ColorScheme::Light);

    let _ = mount(move || {
      Column::new()
        .set_children((
          Theme::new(Cards {
            card: Token::new(CARD, green()).define_when(Media::Dark, red()),
            ..Cards::default()
          })
          .set_child_builder(move || {
            Container::new()
              .set_width(2)
              .set_height(1)
              .derive_background(move || {
                if muted.get() {
                  StaticLeaf::from(Expr::from(CARD).mix(blue(), 0.5))
                }
                else {
                  StaticLeaf::from(CARD)
                }
              })
          }),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(Expr::from(green()).mix(blue(), 0.5)),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(Expr::from(red()).mix(blue(), 0.5)),
        ))
        .into_any()
        .build()
    });
    let cell = |row: usize| colors_at(2, 3, 0, row).1;

    assert_eq!(cell(0), Some(rgb(green())), "the token");

    muted.set(true);

    assert_eq!(cell(0), cell(1), "the expression over it");

    testing::trigger_color_scheme(ColorScheme::Dark);

    assert_eq!(cell(0), cell(2), "the token's dark definition, mixed");
  });
}

/// A run's color resolves like a box's, tokens included: the run inks its glyphs in the
/// palette's card and lays no background of its own.
#[test]
fn a_runs_color_resolves_through_the_theme() {
  with_platform(|| {
    let _ = mount(|| {
      Theme::new(Cards::with_card(green()))
        .set_child_builder(|| Text::new("t").set_color(CARD))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(2, 1, 0, 0), (Some(rgb(green())), None));
  });
}

/// A palette that checks, as it is read for its definitions, that the scene is free.
#[derive(Debug, Clone, PartialEq)]
struct Watchful {
  /// The card's surface.
  card: Token<Color>,
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for Watchful {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    assert!(
      !Scene::is_borrowed(),
      "a token set is read with the scene released"
    );

    vec![self.card.define()]
  }
}

/// A theme reads its palette's definitions with the scene released, on its first
/// definition and on every redefinition, so a palette may reach the scene.
#[test]
fn a_theme_reads_its_definitions_with_the_scene_released() {
  with_platform(|| {
    let card = Signal::new(green());

    let _ = mount(move || {
      Theme::derive(move || Watchful {
        card: Token::new(CARD, card.get()),
      })
      .set_child_builder(|| {
        Container::new()
          .set_width(2)
          .set_height(1)
          .set_background(CARD)
      })
      .into_any()
      .build()
    });

    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(green())));

    card.set(red());

    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(red())));
  });
}

/// A set of one's own redefining the accent among the platform's colors.
#[derive(Debug, Clone, PartialEq)]
struct OwnAccent {
  /// The accent.
  accent: Token<Color>,
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for OwnAccent {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.accent.define()]
  }
}

/// The color the root defines `reference` as.
fn root_color(reference: TokenRef<Color>) -> (u8, u8, u8) {
  Scene::read(
    |scene| match scene.root_definitions().get(reference.name()) {
      Some(TokenValue::Color(definition)) => match definition.base() {
        StaticLeaf::Fixed(color) => rgb(*color),
        leaf => panic!("{} is a fixed color, not {leaf:?}", reference.name()),
      },
      value => panic!("{} is a color, not {value:?}", reference.name()),
    },
  )
}

/// The platform's colors at the root are the terminal's: white on black for the dark
/// scheme it assumes until it reports its colors, then the foreground and background it
/// reports and the blue of its base colors as the accent, and the chosen scheme's own
/// while the app's choice is not what the terminal shows. A box painted in them shows
/// them, and a theme beneath that redefines one shows its own.
#[test]
fn the_system_colors_at_the_root_follow_the_reported_colors_and_the_scheme_choice() {
  with_platform(|| {
    let _ = mount(|| {
      Column::new()
        .set_children((
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(SystemColors::WINDOW),
          Theme::new(OwnAccent {
            accent: Token::new(SystemColors::ACCENT, red()),
          })
          .set_child_builder(|| {
            Container::new()
              .set_width(2)
              .set_height(1)
              .set_background(SystemColors::ACCENT)
          }),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(SystemColors::ACCENT),
        ))
        .into_any()
        .build()
    });

    assert_eq!(root_color(SystemColors::INK), (255, 255, 255));
    assert_eq!(root_color(SystemColors::WINDOW), (0, 0, 0));
    assert_eq!(
      root_color(SystemColors::ACCENT),
      (0, 0, 238),
      "xterm's blue"
    );
    assert_eq!(
      colors_at(2, 3, 0, 1).1,
      Some(rgb(red())),
      "the theme's accent"
    );

    testing::trigger_terminal_colors((220, 220, 220), (40, 42, 54));

    assert_eq!(root_color(SystemColors::INK), (220, 220, 220));
    assert_eq!(root_color(SystemColors::WINDOW), (40, 42, 54));
    assert_eq!(root_color(SystemColors::ON_ACCENT), (220, 220, 220));
    assert_eq!(colors_at(2, 3, 0, 0).1, Some((40, 42, 54)), "the window");
    assert_eq!(
      colors_at(2, 3, 0, 1).1,
      Some(rgb(red())),
      "the theme's accent"
    );
    assert_eq!(
      colors_at(2, 3, 0, 2).1,
      Some((0, 0, 238)),
      "the root's accent"
    );

    set_scheme(Some(ColorScheme::Light));

    assert_eq!(root_color(SystemColors::INK), (0, 0, 0));
    assert_eq!(root_color(SystemColors::WINDOW), (255, 255, 255));
    assert_eq!(colors_at(2, 3, 0, 0).1, Some((255, 255, 255)));

    set_scheme(Some(ColorScheme::Dark));

    assert_eq!(root_color(SystemColors::INK), (220, 220, 220));
    assert_eq!(root_color(SystemColors::WINDOW), (40, 42, 54));
    assert_eq!(colors_at(2, 3, 0, 0).1, Some((40, 42, 54)));

    set_scheme(None);

    assert_eq!(root_color(SystemColors::WINDOW), (40, 42, 54));
  });
}
