//! Reading a token set with no theme above: the defaults land at the root once per set,
//! and a consumer resolves them.

use skene_foundation::{DecorationBase, HasChild, Inset, ThemedIn, TokenName, View, typography};
use skene_headless::{
  Headless,
  children,
  reset,
  resolve_color,
  resolve_length,
  root_definition_writes,
  root_definitions,
};
use skene_reactive::{Get, Owner};

use crate::common::{COLOR, PADDING, TestPalette, TestTypography, WHITE, black, red};

type Container = skene_foundation::Container<Headless>;
type Theme<P> = skene_foundation::Theme<Headless, P>;

/// A miss registers the set's defaults at the root once, a second set lays its own
/// beside them, and a box below no theme resolves both.
#[test]
fn themed_in_registers_root_defaults_once_and_a_box_resolves_them() {
  reset();

  let owner = Owner::new();
  let (card, palette) = owner.enter(|| {
    let palette = <TestPalette as ThemedIn<Headless>>::current();

    assert_eq!(<TestPalette as ThemedIn<Headless>>::watch().get(), palette);
    assert_eq!(<TestPalette as ThemedIn<Headless>>::current(), palette);
    assert_eq!(root_definition_writes(), 1);

    <TestTypography as ThemedIn<Headless>>::current();

    (
      Container::new()
        .set_background(TestPalette::CARD)
        .set_padding(Inset::all(1.))
        .set_child(Container::new().set_background(typography::INK))
        .build(),
      palette,
    )
  });

  assert_eq!(palette, TestPalette::default());
  assert_eq!(root_definition_writes(), 2);
  assert_eq!(root_definitions().len(), 6);
  assert!(root_definitions().get(TokenName::new("card")).is_some());
  assert!(root_definitions().get(typography::INK.name()).is_some());
  assert_eq!(resolve_color(card, COLOR), Some(WHITE));
  assert_eq!(resolve_length(card, PADDING), Some(1.));
  assert_eq!(resolve_color(children(card)[1], COLOR), Some(black()));

  owner.dispose();
}

/// Under a theme the read is the theme's palette and nothing is registered at the
/// root; a box outside the theme still falls back to the default registered later.
#[test]
fn themed_in_reads_the_theme_and_registers_nothing_under_it() {
  reset();

  let owner = Owner::new();
  let (themed, bare) = owner.enter(|| {
    let themed = Theme::new(TestPalette::with_card(red()))
      .set_child_builder(|| {
        assert_eq!(
          <TestPalette as ThemedIn<Headless>>::current(),
          TestPalette::with_card(red())
        );

        Container::new().set_background(TestPalette::CARD)
      })
      .build();

    assert_eq!(root_definition_writes(), 0);

    let bare = Container::new().set_background(TestPalette::CARD).build();

    assert_eq!(resolve_color(bare, COLOR), None);

    <TestPalette as ThemedIn<Headless>>::current();

    (themed, bare)
  });

  assert_eq!(root_definition_writes(), 1);
  assert_eq!(resolve_color(children(themed)[0], COLOR), Some(red()));
  assert_eq!(resolve_color(bare, COLOR), Some(WHITE));

  owner.dispose();
}
