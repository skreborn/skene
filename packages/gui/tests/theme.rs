//! The Simple theme, realized against the recording driver: which values a token puts
//! on a property, and which of them is a conditional variant.

#![cfg(not(target_arch = "wasm32"))]

use skene_gui::{
  Column,
  Container,
  HasChildren,
  Palette,
  RawButton,
  SimpleTheme,
  Text,
  View,
  core::{Color, ColorScheme, Composable, Condition},
  prelude::{Get, Set, Signal, Update},
  record,
};

/// The color a token takes when nothing overrides it.
fn base(token: &skene_gui::Token) -> Color {
  *token.base()
}

/// An adaptive token puts its light value on the property and its dark one beside it as
/// a variant—so the scheme is answered by declarative styling rather than by anything
/// the app runs.
#[test]
fn an_adaptive_token_sets_a_property_and_a_dark_variant() {
  let root = SimpleTheme::adaptive(Palette::light(), Palette::dark())
    .set_child(|| Container::new().set_color(Palette::current().card))
    .build();
  // A theme realizes no node of its own: it provides the palette and builds its child.
  let style = record::box_style(root).expect("a box style");
  let variants = record::box_variants(root);

  assert_eq!(style.color, Some(base(&Palette::light().card)));
  assert_eq!(variants.len(), 1);
  assert_eq!(variants[0].0, Condition::DarkMode);
  assert_eq!(variants[0].1.color, Some(base(&Palette::dark().card)));
}

/// A theme carrying one palette resolves to exactly one value: no variant is recorded,
/// since there is nothing left for a condition to answer.
#[test]
fn one_palette_records_one_value_and_no_variant() {
  let root = SimpleTheme::new(Palette::light())
    .set_child(|| Container::new().set_color(Palette::current().card))
    .build();

  assert_eq!(
    record::box_style(root).expect("a box style").color,
    Some(base(&Palette::light().card))
  );
  assert!(record::box_variants(root).is_empty());
}

/// A token applied under a condition of the setter's own narrows rather than losing its
/// conditions: the state's own variant, then that state under the dark scheme.
#[test]
fn a_token_set_under_a_condition_narrows_to_the_scheme() {
  let root = SimpleTheme::adaptive(Palette::light(), Palette::dark())
    .set_child(|| {
      Container::new().set_color_when(Condition::Hovered, Palette::current().secondary.surface)
    })
    .build();
  let variants = record::box_variants(root);

  assert_eq!(variants.len(), 2);
  assert_eq!(variants[0].0, Condition::Hovered);
  assert_eq!(
    variants[0].1.color,
    Some(base(&Palette::light().secondary.surface))
  );
  assert_eq!(
    variants[1].0,
    Condition::Hovered.both(Condition::DarkMode),
    "the token's own condition is kept, narrowed to the setter's"
  );
  assert_eq!(
    variants[1].1.color,
    Some(base(&Palette::dark().secondary.surface))
  );
}

/// A nested theme replaces the palette beneath it, and leaves the one above alone.
#[test]
fn a_nested_theme_replaces_the_palette_beneath_it() {
  let ink = Color::srgb(0.1, 0.9, 0.4);
  let mut inner = Palette::dark();

  inner.card = ink.into();

  let root = SimpleTheme::adaptive(Palette::light(), Palette::dark())
    .set_child(move || {
      Column::new().set_children((
        Container::new().set_color(Palette::current().card),
        SimpleTheme::new(inner.clone())
          .set_child(|| Container::new().set_color(Palette::current().card)),
      ))
    })
    .build();
  let outer_box = record::children(root)[0];
  let inner_box = record::children(root)[1];

  assert_eq!(
    record::box_variants(outer_box)[0].1.color,
    Some(base(&Palette::dark().card))
  );
  assert_eq!(
    record::box_style(inner_box).expect("a box style").color,
    Some(ink)
  );
  assert!(
    record::box_variants(inner_box).is_empty(),
    "the nested palette is flat, so nothing is left conditional"
  );
}

/// A later setter wins over an earlier one, so a token is a default rather than a rule.
#[test]
fn a_views_own_color_wins_over_the_palettes() {
  let ink = Color::srgb(0.9, 0.2, 0.2);
  let root = SimpleTheme::new(Palette::dark())
    .set_child(move || {
      Container::new()
        .set_color(Palette::current().card)
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
  let run = SimpleTheme::adaptive(Palette::light(), Palette::dark())
    .set_child(|| Text::new("themed"))
    .build();
  let variants = record::run_variants(run);

  assert_eq!(
    record::run_style(run).expect("a run style").color,
    Some(base(&Palette::light().foreground))
  );
  assert!(
    variants
      .iter()
      .any(|(condition, style)| *condition == Condition::DarkMode
        && style.color == Some(base(&Palette::dark().foreground))),
    "the inherited ink carries the scheme with it"
  );
}

/// A run's own ink wins over the inherited one, and its own variant is applied after the
/// theme's so it wins there too.
#[test]
fn a_runs_own_ink_wins_over_the_inherited_one() {
  let run = SimpleTheme::adaptive(Palette::light(), Palette::dark())
    .set_child(|| Text::new("themed").set_color(Palette::current().muted_foreground))
    .build();
  let variants = record::run_variants(run);

  assert_eq!(
    record::run_style(run).expect("a run style").color,
    Some(base(&Palette::light().muted_foreground))
  );
  assert_eq!(
    variants.last().expect("the run's own variant").1.color,
    Some(base(&Palette::dark().muted_foreground))
  );
}

/// Choosing a scheme resolves every token to one value: nothing is left for a condition
/// to answer, and the choice costs no rebuild since it is made before the child builds.
#[test]
fn a_chosen_scheme_leaves_nothing_conditional() {
  let root = SimpleTheme::adaptive(Palette::light(), Palette::dark())
    .set_scheme(ColorScheme::Dark)
    .set_child(|| Container::new().set_color(Palette::current().card))
    .build();

  assert_eq!(
    record::box_style(root).expect("a box style").color,
    Some(base(&Palette::dark().card))
  );
  assert!(record::box_variants(root).is_empty());
}

/// A derived scheme tells the driver which scheme is in force and leaves the tree alone,
/// so everything the subtree holds survives the change: this is why it is not a rebuild.
#[test]
fn a_derived_scheme_keeps_the_subtree_it_re_inks() {
  let scheme = Signal::new(ColorScheme::Light);
  let root = SimpleTheme::adaptive(Palette::light(), Palette::dark())
    .derive_scheme(move || scheme.get())
    .set_child(|| {
      // Component-local state, exactly as a control inside the theme would hold it.
      let count = Signal::new(0u32);

      Column::new().set_children((
        Text::derive(move || count.get().to_string()),
        RawButton::new().handle_click(move || count.update(|value| *value += 1)),
      ))
    })
    .build();
  let counter = record::children(root)[0];

  let _ = record::trigger_click(record::children(root)[1]);

  assert_eq!(record::text(counter).as_deref(), Some("1"));
  assert_eq!(record::color_scheme(), Some(ColorScheme::Light));

  scheme.set(ColorScheme::Dark);

  assert_eq!(
    record::color_scheme(),
    Some(ColorScheme::Dark),
    "the driver is told which scheme paints"
  );

  assert_eq!(
    record::children(root)[0],
    counter,
    "the same node, not a rebuilt one"
  );

  assert_eq!(
    record::text(counter).as_deref(),
    Some("1"),
    "the state inside the theme outlives the scheme change"
  );
}
