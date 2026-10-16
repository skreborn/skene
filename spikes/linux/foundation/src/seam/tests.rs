//! The shared half's unit tests: the resolver at each level, the state rules, the text
//! arithmetic and the contents table.

use std::rc::Rc;

use crate::{
  Color, Corners, Definitions, Environment, Inset, Media, Owner, Signal, Spacing, State, Token,
  TokenRef, Value, When,
  seam::{
    content::{Content, Graphemes, Unit, map_through, splice},
    node::{ControlKind, LayoutKind, NodeKind},
    record::{Prop, Record, Resolved, keys},
    style::{CompiledStyle, Keep, KeptCondition, KeptLeaf, KeptValue, ScopeChain},
  },
};

const CARD: TokenRef<Color> = TokenRef::new("card");
const ACCENT: TokenRef<Color> = TokenRef::new("accent");
const GAP: TokenRef<f32> = TokenRef::new("gap");

fn theme() -> ScopeChain {
  let mut definitions = Definitions::new();

  definitions.add(&Token::new(CARD, Color::WHITE).when(Media::Dark, Color::rgb(0x1c1c1e)));
  definitions.add(&Token::new(GAP, 12));

  ScopeChain::root().scope(Signal::new(Rc::new(definitions)))
}

fn card_record() -> Record {
  let mut record = Record::new();

  record
    .set(
      keys::BACKGROUND,
      Value::from(CARD).when(State::Hovered, Color::rgb(0xeef3ff)),
    )
    .set(keys::PADDING, GAP)
    .set(keys::CORNER_RADIUS, Corners::all(16.))
    .set(
      keys::SPACING,
      Spacing::from(8.).when(Media::MinWidth(900), Spacing::from(16.)),
    );

  record
}

#[test]
fn colors_and_states_keeps_color_tokens_and_states_and_resolves_the_rest() {
  Owner::root().run(|| {
    let scope = theme();

    Environment::current().width.set(320.);

    let style = card_record().compile(Keep::ColorsAndStates, |_| true, &scope, &|_| false);
    let background = &style.props[0].1;

    assert!(matches!(
      background.base,
      KeptLeaf::Token { name: "card", tag } if tag != 0
    ));
    assert_eq!(
      background.variants[0].0,
      KeptCondition::State(State::Hovered)
    );
    assert_eq!(
      style.resolved(Prop::Padding),
      Some(&Resolved::Inset(Inset::all(12.)))
    );

    Environment::current().width.set(1000.);

    let wide = card_record().compile(Keep::ColorsAndStates, |_| true, &scope, &|_| false);

    assert_eq!(
      wide.resolved(Prop::Spacing),
      Some(&Resolved::Spacing(Spacing::from(16.)))
    );
    assert_ne!(style.address, wide.address);
    assert_eq!(style.state_rules().len(), 2);
  });
}

#[test]
fn nothing_kept_answers_states_and_media_in_order() {
  Owner::root().run(|| {
    let scope = theme();
    let hovered = card_record().compile(Keep::Nothing, |_| true, &scope, &|state| {
      state == State::Hovered
    });

    assert_eq!(
      hovered.resolved(Prop::Background),
      Some(&Resolved::Color(Color::rgb(0xeef3ff)))
    );

    Environment::current().dark.set(true);

    let dark = card_record().compile(Keep::Nothing, |_| true, &scope, &|_| false);

    assert_eq!(
      dark.resolved(Prop::Background),
      Some(&Resolved::Color(Color::rgb(0x1c1c1e)))
    );
  });
}

/// The address is the style's content alone: equal themes, and a later render on the same
/// thread with new scope identities, give the same address at every level.
#[test]
fn the_address_is_content_alone() {
  Owner::root().run(|| {
    for keep in [Keep::Nothing, Keep::ColorsAndStates, Keep::Everything] {
      let (first, second) = (theme(), theme());

      assert_ne!(first.id(), second.id());

      let a = card_record().compile(keep, |_| true, &first, &|_| false);
      let b = card_record().compile(keep, |_| true, &second, &|_| false);
      let later = card_record().compile(keep, |_| true, &theme(), &|_| false);

      assert_eq!(a.address, b.address, "{keep:?}");
      assert_eq!(a.address, later.address, "{keep:?}");
    }
  });
}

/// A tag follows the definition: different palettes differ, a nested token's definition is
/// part of it, and a scheme switch changes none.
#[test]
fn a_tag_is_the_definition() {
  Owner::root().run(|| {
    let palette = |card: u32, accent: u32| {
      let mut definitions = Definitions::new();

      definitions.add(&Token::new(ACCENT, Color::rgb(accent)));
      definitions.add(&Token::new(CARD, ACCENT).when(Media::Dark, Color::rgb(card)));

      ScopeChain::root().scope(Signal::new(Rc::new(definitions)))
    };
    let base = palette(0x111111, 0xaaaaaa).tag("card");

    assert_eq!(base, palette(0x111111, 0xaaaaaa).tag("card"));
    assert_ne!(base, palette(0x222222, 0xaaaaaa).tag("card"));
    assert_ne!(base, palette(0x111111, 0xbbbbbb).tag("card"));
    Environment::current().dark.set(true);
    assert_eq!(base, palette(0x111111, 0xaaaaaa).tag("card"));
  });
}

#[test]
fn a_derived_leaf_is_a_slot_where_kept() {
  Owner::root().run(|| {
    let count = Signal::new(1u32);
    let mut record = Record::new();

    record.set(
      keys::OPACITY,
      Value::derived(move || if count.get() > 1 { 1. } else { 0.5 }),
    );

    for keep in [Keep::Everything, Keep::ColorsAndStates] {
      let first = record.compile(keep, |_| true, &ScopeChain::root(), &|_| false);

      count.set(if count.get_untracked() > 1 { 1 } else { 2 });

      let second = record.compile(keep, |_| true, &ScopeChain::root(), &|_| false);

      assert_eq!(first.address, second.address);
      assert_ne!(first.slots, second.slots);
    }
  });
}

/// The state rules are the fewest that give, through a cascade where more states win and
/// then the later rule, exactly each combination's leaves.
#[test]
fn state_rules_are_pruned_and_exact() {
  let leaf = |value: f32| KeptLeaf::Value(Resolved::Number(value));
  let varying = |state: State, value: f32| KeptValue {
    base: leaf(0.),
    variants: vec![(KeptCondition::State(state), leaf(value))],
  };
  let disjoint = CompiledStyle {
    props: vec![
      (Prop::Opacity, varying(State::Hovered, 1.)),
      (Prop::FontSize, varying(State::Pressed, 2.)),
      (Prop::LineHeight, varying(State::FocusVisible, 3.)),
      (Prop::LetterSpacing, varying(State::Disabled, 4.)),
      (Prop::CornerRadius, varying(State::Checked, 5.)),
    ],
    transitions: Vec::new(),
    slots: Vec::new(),
    address: 0,
  };
  let shared = CompiledStyle {
    props: vec![(
      Prop::Background,
      KeptValue {
        base: leaf(0.),
        variants: vec![
          (KeptCondition::State(State::Hovered), leaf(1.)),
          (KeptCondition::State(State::Pressed), leaf(2.)),
          (KeptCondition::State(State::Disabled), leaf(3.)),
          (KeptCondition::State(State::Hovered), leaf(4.)),
        ],
      },
    )],
    ..disjoint.clone()
  };

  assert_eq!(disjoint.state_rules().len(), 6);

  for style in [&disjoint, &shared] {
    let rules = style.state_rules();

    for holding in style.states().subsets() {
      for (prop, value) in &style.props {
        assert_eq!(
          CompiledStyle::winner(&rules, holding, *prop),
          Some(value.with_states(holding)),
          "{holding:?} {prop:?}"
        );
      }
    }
  }
}

#[test]
fn text_arithmetic_in_either_unit() {
  let text = "ae\u{301}b\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}c";

  assert_eq!(Graphemes::cluster(text, Unit::CodePoint, 2), 1);
  assert_eq!(Graphemes::offset(text, Unit::CodePoint, 3), 4);
  assert_eq!(Graphemes::offset(text, Unit::Utf16, 4), 12);
  assert_eq!(Graphemes::cluster(text, Unit::Utf16, 12), 4);
  assert_eq!(
    splice("abc", "aXc", Unit::CodePoint),
    (1, 2, "X".to_owned())
  );
  assert_eq!(
    splice("ab1", "AB", Unit::CodePoint),
    (0, 3, "AB".to_owned())
  );
  assert_eq!(
    splice("a\u{1F600}b", "a\u{1F600}c", Unit::CodePoint),
    (2, 3, "c".to_owned())
  );
  assert_eq!(
    splice("a\u{1F600}b", "a\u{1F600}c", Unit::Utf16),
    (3, 4, "c".to_owned())
  );
  assert_eq!(
    splice("\u{e9}", "\u{e8}", Unit::Utf16),
    (0, 1, "\u{e8}".to_owned())
  );
  assert_eq!(map_through(3, 1, 2, 1), 3);
  assert_eq!(Unit::Utf16.byte("a\u{1F600}b", 2), 1);
}

#[test]
fn every_kind_takes_what_its_contents_say() {
  assert!(NodeKind::Control(ControlKind::Slider).takes(Content::Range));
  assert!(!NodeKind::Control(ControlKind::Switch).takes(Content::Range));
  assert!(NodeKind::Native.takes(Content::Door("date")));
  assert!(!NodeKind::Layout(LayoutKind::Row).takes(Content::Door("date")));
}
