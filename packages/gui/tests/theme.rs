//! The Simple theme, realized against the recording driver: which values a themed
//! property records, and which of them is a conditional variant.

#![cfg(not(target_arch = "wasm32"))]

use skene_gui::{
  Column,
  Container,
  HasChildren,
  Palette,
  SimpleTheme,
  Text,
  ThemedBox,
  ThemedText,
  View,
  core::{Color, ColorScheme, Condition},
  record,
};

/// Following the platform, a themed property records the light value as its own and the
/// dark one as a variant—so the scheme is answered by declarative styling rather than by
/// anything the app runs.
#[test]
fn following_the_platform_records_a_base_and_a_dark_variant() {
  let root = SimpleTheme::new()
    .set_child(|| Container::new().theme_color(|palette| palette.card))
    .build();
  // A theme realizes no node of its own: it provides the palettes and builds its child.
  let box_node = root;
  let style = record::box_style(box_node).expect("a box style");
  let variants = record::box_variants(box_node);

  assert_eq!(style.color, Some(Palette::light().card));
  assert_eq!(variants.len(), 1);
  assert_eq!(variants[0].0, Condition::DarkMode);
  assert_eq!(variants[0].1.color, Some(Palette::dark().card));
}

/// A fixed scheme resolves to exactly one value: no variant is recorded, since there is
/// nothing left for a condition to answer.
#[test]
fn a_fixed_scheme_records_one_value_and_no_variant() {
  let root = SimpleTheme::new()
    .set_scheme(ColorScheme::Light)
    .set_child(|| Container::new().theme_color(|palette| palette.card))
    .build();

  assert_eq!(
    record::box_style(root).expect("a box style").color,
    Some(Palette::light().card)
  );
  assert!(record::box_variants(root).is_empty());
}

/// A conditional themed property narrows to the scheme rather than opening a second
/// conditional system: the state's own variant, then that state under the dark scheme.
#[test]
fn a_conditional_themed_property_narrows_to_the_scheme() {
  let root = SimpleTheme::new()
    .set_child(|| {
      Container::new().theme_color_when(Condition::Hovered, |palette| palette.secondary.surface)
    })
    .build();
  let variants = record::box_variants(root);

  assert_eq!(variants.len(), 2);
  assert_eq!(variants[0].0, Condition::Hovered);
  assert_eq!(
    variants[0].1.color,
    Some(Palette::light().secondary.surface)
  );
  assert_eq!(
    variants[1].0,
    Condition::and([Condition::Hovered, Condition::DarkMode])
  );
  assert_eq!(variants[1].1.color, Some(Palette::dark().secondary.surface));
}

/// A nested theme overlays the outer one: the palette it replaces wins beneath it, and
/// the one it leaves alone still comes from outside.
#[test]
fn a_nested_theme_overlays_the_outer_one() {
  let ink = Color::srgb(0.1, 0.9, 0.4);
  let mut inner = Palette::dark();

  inner.card = ink;

  let root = SimpleTheme::new()
    .set_child(move || {
      Column::new().set_children((
        Container::new().theme_color(|palette| palette.card),
        SimpleTheme::new()
          .set_dark(inner)
          .set_child(|| Container::new().theme_color(|palette| palette.card)),
      ))
    })
    .build();
  let outer_box = record::children(root)[0];
  let inner_box = record::children(root)[1];

  assert_eq!(
    record::box_variants(outer_box)[0].1.color,
    Some(Palette::dark().card)
  );
  assert_eq!(record::box_variants(inner_box)[0].1.color, Some(ink));
  assert_eq!(
    record::box_style(inner_box).expect("a box style").color,
    Some(Palette::light().card),
    "the palette the nested theme left alone still comes from the outer one"
  );
}

/// A view's own setter wins over the theme's, so a themed default is a default rather
/// than a rule.
#[test]
fn a_views_own_color_wins_over_the_theme() {
  let ink = Color::srgb(0.9, 0.2, 0.2);
  let root = SimpleTheme::new()
    .set_child(move || {
      Container::new()
        .theme_color(|palette| palette.card)
        .set_color(ink)
    })
    .build();

  assert_eq!(
    record::box_style(root).expect("a box style").color,
    Some(ink)
  );
}

/// The theme inks the runs beneath it, scheme and all, without any run naming a color.
#[test]
fn the_theme_inks_the_runs_beneath_it() {
  let run = SimpleTheme::new().set_child(|| Text::new("themed")).build();
  let variants = record::run_variants(run);

  assert_eq!(
    record::run_style(run).expect("a run style").color,
    Some(Palette::light().foreground)
  );
  assert_eq!(variants.len(), 1);
  assert_eq!(variants[0].0, Condition::DarkMode);
  assert_eq!(variants[0].1.color, Some(Palette::dark().foreground));
}

/// A run's own ink wins over the inherited one, and its own variant is applied after the
/// theme's so it wins there too.
#[test]
fn a_runs_own_ink_wins_over_the_inherited_one() {
  let run = SimpleTheme::new()
    .set_child(|| Text::new("themed").theme_color(|palette| palette.muted_foreground))
    .build();
  let variants = record::run_variants(run);

  assert_eq!(
    record::run_style(run).expect("a run style").color,
    Some(Palette::light().muted_foreground)
  );
  assert_eq!(
    variants.last().expect("the run's own variant").1.color,
    Some(Palette::dark().muted_foreground)
  );
}
