//! The runs: a run inherits through the typography tokens, a group redefines them for
//! its own spans, and a span embeds a view inline.

mod common;

use skene_common::{ThemedIn, typography};
use skene_core::{HasChildren, RunDisplay, StaticLeaf, StaticValue, View};
use skene_headless::{
  Headless,
  children,
  definitions,
  outline,
  reset,
  resolve_color,
  root_definitions,
  run_props,
};
use skene_reactive::{Get, Owner, Set, Signal};

use crate::common::{TEXT_COLOR, TestTypography, black, blue, green, red};

type Container = skene_common::Container<Headless>;
type Text = skene_common::Text<Headless>;
type Span = skene_common::Span<Headless>;
type RichText = skene_common::RichText<Headless>;
type Theme<S, C = skene_common::NoChild> = skene_common::Theme<Headless, S, C>;

/// A text with no color references the ink token: under a theme it resolves the
/// theme's ink, and under none the root default the typography set registered; a link
/// references the link ink instead.
#[test]
fn text_without_a_color_resolves_the_typography_ink() {
  reset();

  let owner = Owner::new();
  let (themed, bare, link) = owner.enter(|| {
    let themed = Theme::new(TestTypography::with_ink(red()))
      .set_child_builder(|| Text::new("themed"))
      .build();

    <TestTypography as ThemedIn<Headless>>::current();

    (
      themed,
      Text::new("bare").build(),
      Text::new("link").set_link("https://example.com").build(),
    )
  });
  let themed = children(themed)[0];
  let props = run_props(bare).unwrap();

  assert_eq!(
    props.style.color,
    Some(StaticValue::from(StaticLeaf::Token(typography::INK)))
  );
  assert_eq!(
    props.style.selection_color,
    Some(StaticValue::from(StaticLeaf::Token(
      typography::SELECTION_INK
    )))
  );
  assert_eq!(resolve_color(themed, TEXT_COLOR), Some(red()));
  assert_eq!(resolve_color(bare, TEXT_COLOR), Some(black()));
  assert_eq!(resolve_color(link, TEXT_COLOR), Some(blue()));
  assert_eq!(
    run_props(link).unwrap().link.as_deref(),
    Some("https://example.com")
  );
  assert_eq!(root_definitions().len(), 4);

  owner.dispose();
}

/// A group's color redefines the ink for the spans beneath it and no other: a sibling
/// keeps the theme's ink, a nested group redefines again, and a derived group color
/// follows its signal.
#[test]
fn rich_text_group_redefines_ink_for_nested_spans_only() {
  reset();

  let owner = Owner::new();
  let accent = Signal::new(red());
  let root = owner.enter(|| {
    Theme::new(TestTypography::default())
      .set_child_builder(move || {
        RichText::new().set_spans([
          Span::new("a"),
          Span::group([
            Span::new("b"),
            Span::group([Span::new("c")]).set_color(green()),
            Span::new("d").set_link("/d"),
          ])
          .derive_color(move || accent.get()),
        ])
      })
      .build()
  });
  let paragraph = children(root)[0];

  assert_eq!(
    outline(paragraph),
    "Box\n  Run \"a\"\n  Run \"\"\n    Run \"b\"\n    Run \"\"\n      Run \"c\"\n    Run \"d\"\n"
  );

  let spans = children(paragraph);
  let group = spans[1];
  let inner = children(group);
  let nested = inner[1];

  assert!(matches!(
    run_props(spans[0]).unwrap().display,
    RunDisplay::Inline
  ));
  assert_eq!(resolve_color(spans[0], TEXT_COLOR), Some(black()));
  assert_eq!(resolve_color(group, TEXT_COLOR), Some(red()));
  assert_eq!(resolve_color(inner[0], TEXT_COLOR), Some(red()));
  assert_eq!(resolve_color(nested, TEXT_COLOR), Some(green()));
  assert_eq!(
    resolve_color(children(nested)[0], TEXT_COLOR),
    Some(green())
  );
  assert_eq!(resolve_color(inner[2], TEXT_COLOR), Some(blue()));
  assert_eq!(
    definitions(group).map(|definitions| definitions.len()),
    Some(1)
  );
  assert_eq!(definitions(inner[0]), None);

  accent.set(blue());

  assert_eq!(resolve_color(group, TEXT_COLOR), Some(blue()));
  assert_eq!(resolve_color(inner[0], TEXT_COLOR), Some(blue()));
  assert_eq!(resolve_color(nested, TEXT_COLOR), Some(green()));

  owner.dispose();
}

/// A view span is an inline-level run holding the view's node, and a derived span
/// rewrites its own run's text.
#[test]
fn span_embeds_a_view_inline_and_derives_its_text() {
  reset();

  let owner = Owner::new();
  let label = Signal::new("one".to_owned());
  let paragraph = owner.enter(|| {
    RichText::new()
      .set_spans([
        Span::derive(move || label.get()),
        Span::view(Container::new().set_child(Text::new("boxed"))),
      ])
      .build()
  });
  let spans = children(paragraph);

  assert_eq!(
    outline(paragraph),
    "Box\n  Run \"one\"\n  Run \"\"\n    Box\n      Run \"boxed\"\n"
  );
  assert!(matches!(
    run_props(spans[1]).unwrap().display,
    RunDisplay::InlineBox
  ));

  label.set("two".to_owned());

  assert_eq!(
    outline(paragraph),
    "Box\n  Run \"two\"\n  Run \"\"\n    Box\n      Run \"boxed\"\n"
  );

  owner.dispose();
}
