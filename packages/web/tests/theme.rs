//! The Simple palette under `Theme`, realized against the headless platform: nested
//! palettes, a palette flip that rebuilds nothing, and the scheme the app puts in
//! force.

#![cfg(skene_web_platform = "headless")]

use skene_foundation::{Color, ColorScheme, Media, Property, PropertyKey, StaticLeaf};
use skene_simple::web::prelude::*;
use skene_web::{
  Activate,
  BasicButton,
  Column,
  Container,
  HasChildren,
  NodeId,
  Text,
  Theme,
  ThemedIn,
  Typography,
  View,
  WebProperty,
  headless::{children, end_turn, reset, resolve_color, text, trigger_click},
  media,
  prelude::{Get, Owner, Set, Signal, Update},
};

const COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Color);
const TEXT_COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::TextColor);

/// The nodes between a container's anchors: its content, in order.
fn content(node: NodeId) -> Vec<NodeId> {
  let nodes = children(node);

  nodes[1..nodes.len() - 1].to_vec()
}

fn fixed(leaf: &StaticLeaf<Color>) -> Color {
  match leaf {
    StaticLeaf::Fixed(color) => *color,
    _ => panic!("a fixed color"),
  }
}

/// The default palette's card color under the light scheme.
fn card_light() -> Color {
  fixed(SimplePalette::default().card.definition().base())
}

/// The default palette's card color under the dark scheme.
fn card_dark() -> Color {
  let palette = SimplePalette::default();
  let (condition, leaf) = &palette.card.definition().variants()[0];

  assert_eq!(*condition, Media::Dark);

  fixed(leaf)
}

/// A nested theme replaces the palette beneath it and leaves the one above alone;
/// each consumer resolves the nearest definition.
#[test]
fn a_nested_theme_replaces_the_palette_beneath_it() {
  reset();

  let owner = Owner::new();
  let ink = Color::srgb(0.1, 0.9, 0.4);
  let root = owner.enter(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(move || {
        Column::new().set_children((
          Container::new().set_color(SimplePalette::current().card),
          Theme::new(SimplePalette::default().set_card(ink))
            .set_child_builder(|| Container::new().set_color(SimplePalette::CARD)),
        ))
      })
      .build()
  });
  let column = children(root)[0];
  let outer_box = content(column)[0];
  let inner_box = children(content(column)[1])[0];

  assert_eq!(resolve_color(outer_box, COLOR), Some(card_light()));
  assert_eq!(resolve_color(inner_box, COLOR), Some(ink));

  owner.dispose();
}

/// A palette flip redefines the theme's tokens and touches no node: the same ids
/// stand, and the state inside survives.
#[test]
fn a_palette_flip_keeps_the_nodes_and_their_state() {
  reset();

  let owner = Owner::new();
  let palette = Signal::transient(SimplePalette::default());
  let root = owner.enter(|| {
    Theme::derive(move || palette.get())
      .set_child_builder(|| {
        let count = Signal::new(0u32);

        Column::new().set_children((
          Container::new().set_color(SimplePalette::CARD),
          Text::derive(move || count.get().to_string()),
          BasicButton::new(Activate::new()).handle_click(move || count.update(|value| *value += 1)),
        ))
      })
      .build()
  });
  let column = children(root)[0];
  let before = content(column);

  let _ = trigger_click(before[2]);

  assert_eq!(text(before[1]).as_deref(), Some("1"));
  assert_eq!(resolve_color(before[0], COLOR), Some(card_light()));

  let flipped = Color::srgb(0.9, 0.2, 0.2);

  palette.set(SimplePalette::default().set_card(flipped));

  assert_eq!(content(column), before, "the same nodes, not rebuilt ones");
  assert_eq!(text(before[1]).as_deref(), Some("1"));
  assert_eq!(resolve_color(before[0], COLOR), Some(flipped));

  owner.dispose();
}

/// The scheme in force is the document's: `media::set_scheme` picks which value a
/// conditional token takes, the signal follows, and the tree is left alone.
#[test]
fn the_scheme_follows_the_media_choice() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(|| Container::new().set_color(SimplePalette::CARD))
      .build()
  });
  let card = children(root)[0];

  assert_eq!(media::scheme().get(), ColorScheme::Light);
  assert_eq!(resolve_color(card, COLOR), Some(card_light()));

  media::set_scheme(Some(ColorScheme::Dark));

  assert_eq!(media::scheme().get(), ColorScheme::Dark);
  assert_eq!(media::chosen_scheme(), Some(ColorScheme::Dark));
  assert_eq!(resolve_color(card, COLOR), Some(card_dark()));
  assert_eq!(children(root)[0], card, "re-inked, not rebuilt");

  media::set_scheme(None);

  assert_eq!(media::scheme().get(), ColorScheme::Light);
  assert_eq!(resolve_color(card, COLOR), Some(card_light()));

  owner.dispose();
}

/// A derived scheme follows its signal the same way, and a `Media::Dark` watch agrees
/// with it.
#[test]
fn a_derived_scheme_re_inks_without_a_rebuild() {
  reset();

  let owner = Owner::new();
  let scheme = Signal::new(Some(ColorScheme::Light));
  let dark = owner.enter(|| media::watch(&Media::Dark));
  let root = owner.enter(|| {
    media::derive_scheme(move || scheme.get());

    Theme::new(SimplePalette::default())
      .set_child_builder(|| Container::new().set_color(SimplePalette::CARD))
      .build()
  });
  let card = children(root)[0];

  assert!(!dark.get());

  scheme.set(Some(ColorScheme::Dark));
  end_turn();

  assert!(dark.get());
  assert_eq!(resolve_color(card, COLOR), Some(card_dark()));
  assert_eq!(children(root)[0], card);

  scheme.set(None);
  media::set_scheme(None);

  owner.dispose();
}

/// The facade's typography defines the run tokens: a text under no theme is inked in
/// the default ink, and a text under the Simple theme in the palette's foreground.
#[test]
fn typography_inks_the_runs() {
  reset();

  let owner = Owner::new();
  let (bare, themed) = owner.enter(|| {
    let typography = Typography::current();
    let bare = Text::new("bare").build();
    let themed = Theme::new(SimplePalette::default())
      .set_child_builder(|| Text::new("themed"))
      .build();

    assert_eq!(typography, Typography::default());

    (bare, themed)
  });
  let ink = fixed(Typography::default().ink.definition().base());
  let foreground = fixed(SimplePalette::default().foreground.definition().base());

  assert_eq!(resolve_color(bare, TEXT_COLOR), Some(ink));
  assert_eq!(
    resolve_color(children(themed)[0], TEXT_COLOR),
    Some(foreground)
  );

  owner.dispose();
}
