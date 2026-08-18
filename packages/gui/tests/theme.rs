//! The Simple theme, realized against the recording driver: which values a token puts
//! on a property, and which of them is a conditional variant.

#![cfg(not(target_arch = "wasm32"))]

use skene_gui::{
  Column,
  Container,
  HasChildren,
  RawButton,
  SimplePalette,
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

/// The color a token takes under the reader's dark preference.
fn dark(token: &skene_gui::Token) -> Color {
  token
    .variants()
    .iter()
    .find(|(condition, _)| *condition == Condition::DarkMode)
    .map(|(_, color)| *color)
    .expect("a dark variant")
}

/// The default palette's card token, the one most of these read.
fn card() -> skene_gui::Token {
  SimplePalette::default().card
}

/// A token carrying a scheme condition puts its unconditional value on the property and
/// the conditional one beside it as a variant—so the scheme is answered by declarative
/// styling rather than by anything the app runs.
#[test]
fn a_conditional_token_sets_a_property_and_a_variant() {
  let root = SimpleTheme::new(SimplePalette::default())
    .set_child(|| Container::new().set_color(SimplePalette::current().card))
    .build();
  // A theme realizes no node of its own: it provides the palette and builds its child.
  let style = record::box_style(root).expect("a box style");
  let variants = record::box_variants(root);

  assert_eq!(style.color, Some(base(&card())));
  assert_eq!(variants.len(), 1);
  assert_eq!(variants[0].0, Condition::DarkMode);
  assert_eq!(variants[0].1.color, Some(dark(&card())));
}

/// A token with no conditions records exactly one value: nothing is left for a condition
/// to answer, and a palette says so per property rather than per palette.
#[test]
fn an_unconditional_token_records_one_value_and_no_variant() {
  let flat = Color::srgb(0.2, 0.4, 0.8);
  let palette = SimplePalette::default().set_card(flat);
  let root = SimpleTheme::new(palette)
    .set_child(|| Container::new().set_color(SimplePalette::current().card))
    .build();

  assert_eq!(
    record::box_style(root).expect("a box style").color,
    Some(flat)
  );
  assert!(record::box_variants(root).is_empty());
}

/// A palette's own conditions are not the scheme's: a token answers a viewport the same
/// way it answers a preference, through the same setter.
#[test]
fn a_token_answers_any_condition_the_same_way() {
  let wide = Color::srgb(0.3, 0.7, 0.5);
  let palette = SimplePalette::default().set_card_when(Condition::MinWidth(1200.), wide);
  let root = SimpleTheme::new(palette)
    .set_child(|| Container::new().set_color(SimplePalette::current().card))
    .build();
  let variants = record::box_variants(root);

  assert_eq!(
    variants.len(),
    2,
    "the scheme's variant, then the viewport's"
  );
  assert_eq!(variants[0].0, Condition::DarkMode);
  assert_eq!(variants[1].0, Condition::MinWidth(1200.));
  assert_eq!(variants[1].1.color, Some(wide));
}

/// A token applied under a condition of the setter's own narrows rather than losing its
/// conditions: the state's own variant, then that state under the dark scheme.
#[test]
fn a_token_set_under_a_condition_narrows_to_the_scheme() {
  let root = SimpleTheme::new(SimplePalette::default())
    .set_child(|| {
      Container::new().set_color_when(
        Condition::Hovered,
        SimplePalette::current().secondary.surface,
      )
    })
    .build();
  let surface = SimplePalette::default().secondary.surface;
  let variants = record::box_variants(root);

  assert_eq!(variants.len(), 2);
  assert_eq!(variants[0].0, Condition::Hovered);
  assert_eq!(variants[0].1.color, Some(base(&surface)));
  assert_eq!(
    variants[1].0,
    Condition::Hovered.both(Condition::DarkMode),
    "the token's own condition is kept, narrowed to the setter's"
  );
  assert_eq!(variants[1].1.color, Some(dark(&surface)));
}

/// A nested theme replaces the palette beneath it, and leaves the one above alone.
#[test]
fn a_nested_theme_replaces_the_palette_beneath_it() {
  let ink = Color::srgb(0.1, 0.9, 0.4);
  let inner = SimplePalette::default().set_card(ink);
  let root = SimpleTheme::new(SimplePalette::default())
    .set_child(move || {
      Column::new().set_children((
        Container::new().set_color(SimplePalette::current().card),
        SimpleTheme::new(inner.clone())
          .set_child(|| Container::new().set_color(SimplePalette::current().card)),
      ))
    })
    .build();
  let outer_box = record::children(root)[0];
  let inner_box = record::children(root)[1];

  assert_eq!(
    record::box_variants(outer_box)[0].1.color,
    Some(dark(&card()))
  );
  assert_eq!(
    record::box_style(inner_box).expect("a box style").color,
    Some(ink)
  );
  assert!(
    record::box_variants(inner_box).is_empty(),
    "the nested token carries no conditions, so nothing is left conditional"
  );
}

/// A later setter wins over an earlier one, so a token is a default rather than a rule.
#[test]
fn a_views_own_color_wins_over_the_palettes() {
  let ink = Color::srgb(0.9, 0.2, 0.2);
  let root = SimpleTheme::new(SimplePalette::default())
    .set_child(move || {
      Container::new()
        .set_color(SimplePalette::current().card)
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
  let run = SimpleTheme::new(SimplePalette::default())
    .set_child(|| Text::new("themed"))
    .build();
  let foreground = SimplePalette::default().foreground;
  let variants = record::run_variants(run);

  assert_eq!(
    record::run_style(run).expect("a run style").color,
    Some(base(&foreground))
  );
  assert!(
    variants
      .iter()
      .any(|(condition, style)| *condition == Condition::DarkMode
        && style.color == Some(dark(&foreground))),
    "the inherited ink carries the scheme with it"
  );
}

/// A run's own ink wins over the inherited one, and its own variant is applied after the
/// theme's so it wins there too.
#[test]
fn a_runs_own_ink_wins_over_the_inherited_one() {
  let run = SimpleTheme::new(SimplePalette::default())
    .set_child(|| Text::new("themed").set_color(SimplePalette::current().muted_foreground))
    .build();
  let muted = SimplePalette::default().muted_foreground;
  let variants = record::run_variants(run);

  assert_eq!(
    record::run_style(run).expect("a run style").color,
    Some(base(&muted))
  );
  assert_eq!(
    variants.last().expect("the run's own variant").1.color,
    Some(dark(&muted))
  );
}

/// Choosing a scheme tells the platform which value the conditional tokens take, and
/// leaves the tokens alone: the choice is the document's, not a theme's, so nothing about
/// the tree changes shape.
#[test]
fn a_chosen_scheme_is_told_to_the_platform() {
  let root = SimpleTheme::new(SimplePalette::default())
    .set_child(|| Container::new().set_color(SimplePalette::current().card))
    .build();

  skene_gui::set_scheme(Some(ColorScheme::Dark));

  assert_eq!(record::color_scheme(), Some(ColorScheme::Dark));
  assert_eq!(
    record::box_style(root).expect("a box style").color,
    Some(base(&card())),
    "the token still carries both values; the platform picks between them"
  );
  assert_eq!(record::box_variants(root).len(), 1);

  skene_gui::set_scheme(None);
}

/// A derived scheme tells the driver which scheme is in force and leaves the tree alone,
/// so everything the subtree holds survives the change: this is why it is not a rebuild.
#[test]
fn a_derived_scheme_keeps_the_subtree_it_re_inks() {
  let scheme = Signal::new(ColorScheme::Light);
  let root = SimpleTheme::new(SimplePalette::default())
    .set_child(move || {
      skene_gui::derive_scheme(move || Some(scheme.get()));

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

  skene_gui::set_scheme(None);
}
