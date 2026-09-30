//! The R2 gate: the built-in views realized against the headless platform, built and
//! driven without a platform.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_common::gui::GuiProperty;
use skene_core::{
  BoxValue,
  Bubbling,
  Color,
  Condition,
  ContainerName,
  ContainerQuery,
  Definition,
  Dimension,
  Edge,
  EventKind,
  Expr,
  Flexibility,
  HasChild,
  HasChildren,
  Inset,
  Leaf,
  Length,
  Media,
  NoChild,
  NodeId,
  Platform,
  PointerPrecision,
  Prevented,
  Property,
  PropertyKey,
  Scene,
  Size,
  Slot,
  State,
  StaticLeaf,
  StaticValue,
  Themed,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  Value,
  View,
  When,
};
use skene_headless::{
  Headless,
  SlotValue,
  TokenValue,
  box_props,
  children,
  full_outline,
  is_focused,
  is_hovered,
  listened,
  outline,
  reset,
  resolve_color,
  resolve_length,
  slot_leaf,
  text,
  trigger_change,
  trigger_click,
  trigger_color_scheme,
  trigger_container_size,
  trigger_focus,
  trigger_frame,
  trigger_high_contrast,
  trigger_pointer_enter,
  trigger_pointer_hovers,
  trigger_pointer_leave,
  trigger_pointer_precision,
  trigger_reduced_transparency,
  trigger_resize,
  trigger_safe_area,
  trigger_size,
  value,
};
use skene_reactive::{
  Get,
  IndexedField,
  KeyedField,
  Owner,
  Set,
  Signal,
  Store,
  StoreField,
  Track,
  signal,
};

type Container = skene_common::Container<Headless>;
type Column = skene_common::Column<Headless>;
type Text = skene_common::Text<Headless>;
type Flexible<C = NoChild> = skene_common::Flexible<Headless, C>;
type Theme<S, C = NoChild> = skene_common::Theme<Headless, S, C>;

const COLOR: PropertyKey<GuiProperty> = PropertyKey::Shared(Property::Color);
const WIDTH: PropertyKey<GuiProperty> = PropertyKey::Shared(Property::Width);
const PADDING: PropertyKey<GuiProperty> = PropertyKey::Shared(Property::Padding);
const SIDEBAR: ContainerName = ContainerName("sidebar");

const WHITE: Color = Color::TRANSPARENT;

fn red() -> Color {
  Color::srgb(1., 0., 0.)
}

fn green() -> Color {
  Color::srgb(0., 1., 0.)
}

fn blue() -> Color {
  Color::srgb(0., 0., 1.)
}

/// A component library's palette: two tokens, defined by the library's defaults.
#[derive(Debug, Clone, PartialEq)]
struct TestPalette {
  card: Token<Color, f32>,
  gutter: Token<f32, f32>,
}

impl TestPalette {
  const CARD: TokenRef<Color> = TokenRef::new("card");
  const GUTTER: TokenRef<f32> = TokenRef::new("gutter");

  fn with_card(card: Color) -> Self {
    Self {
      card: Token::new("card", card),
      ..Self::default()
    }
  }
}

impl Default for TestPalette {
  fn default() -> Self {
    Self {
      card: Token::new("card", WHITE).define_when(Media::Dark, Color::srgb(0., 0., 0.)),
      gutter: Token::new("gutter", 8.),
    }
  }
}

impl TokenSet<TokenValue> for TestPalette {
  fn definitions(&self) -> Vec<(TokenName, TokenValue)> {
    vec![self.card.define(), self.gutter.define()]
  }
}

fn hovered() -> Condition<f32> {
  Condition::from(State::Hovered)
}

/// The content between a container's anchors: its one child.
fn only_child(node: NodeId) -> NodeId {
  children(node)[1]
}

/// The host platform takes the pixel media's own properties: a box's corner radius and
/// a run's font size resolve as set, the size an `em` measures against.
#[test]
fn the_pixel_properties_resolve_on_the_host() {
  reset();

  let owner = Owner::new();
  let node = owner.enter(|| {
    Container::new()
      .set_corner_radius(8.)
      .set_child(Text::new("big").set_font_size(24.))
      .build()
  });
  let run = only_child(node);

  assert_eq!(
    resolve_length(node, PropertyKey::Extension(GuiProperty::CornerRadius)),
    Some(8.)
  );
  assert_eq!(
    resolve_length(run, PropertyKey::Extension(GuiProperty::FontSize)),
    Some(24.)
  );

  owner.dispose();
}

/// A container with a derived child realizes a box and a run and nothing else, and a
/// change rebuilds the run's text in place of the old run.
#[test]
fn derived_child_realizes_only_a_box_and_a_run() {
  reset();

  let owner = Owner::new();
  let (label, set_label) = signal("one".to_owned());
  let root = owner.enter(|| {
    Container::new()
      .derive_child(move || Text::new(label.get()))
      .build()
  });

  assert_eq!(outline(root), "Box\n  Run \"one\"\n");
  assert_eq!(
    full_outline(root),
    "Box\n  Anchor\n  Anchor\n  Run \"one\"\n  Anchor\n  Anchor\n"
  );

  let first = children(root)[2];

  set_label.set("two".to_owned());

  assert_eq!(outline(root), "Box\n  Run \"two\"\n");
  assert_ne!(children(root)[2], first);
  assert_eq!(text(first), None);

  owner.dispose();
}

/// A derived text rewrites its run's text without replacing the run.
#[test]
fn derived_text_rewrites_the_run_in_place() {
  reset();

  let owner = Owner::new();
  let (label, set_label) = signal("one".to_owned());
  let run = owner.enter(|| Text::derive(move || label.get()).build());

  assert_eq!(text(run), Some("one".to_owned()));

  set_label.set("two".to_owned());

  assert_eq!(text(run), Some("two".to_owned()));

  owner.dispose();
}

/// A flexible wrapper around a derived subtree lands its annotation on the content
/// box's props, on every rebuild, and on no other node.
#[test]
fn flexible_annotates_the_content_of_a_derived_subtree() {
  reset();

  let owner = Owner::new();
  let (version, set_version) = signal(0);
  let root = owner.enter(|| {
    Column::new()
      .derive_child(move || {
        version.track();

        Flexible::new()
          .set_grow_factor(2.)
          .set_child(Container::new().set_child(Text::new("x")))
      })
      .build()
  });
  let flex = |node: NodeId| box_props(node).and_then(|props| props.style.flex);
  let expected = Some(StaticValue::from(Flexibility::grow(2.)));

  assert_eq!(outline(root), "Box\n  Box\n    Run \"x\"\n");

  let content = children(root)[2];

  assert_eq!(flex(content), expected);
  assert_eq!(flex(root), None);
  assert!(box_props(only_child(content)).is_none());

  set_version.set(1);

  let rebuilt = children(root)[2];

  assert_ne!(rebuilt, content);
  assert_eq!(flex(rebuilt), expected);
  assert_eq!(flex(root), None);

  owner.dispose();
}

/// A keyed item that leaves has its owner disposed, so a signal created while building
/// it is dead; kept items keep their node ids, and the platform's mirror follows the
/// reorder.
#[test]
fn derived_children_dispose_a_removed_items_owner_and_keep_the_rest() {
  reset();

  let owner = Owner::new();
  let (items, set_items) = signal(vec![1, 2, 3]);
  let inner = Rc::new(Cell::new(None::<Signal<i32>>));
  let root = owner.enter(|| {
    let inner = Rc::clone(&inner);

    Column::new()
      .derive_children(
        move || items.get(),
        |item| *item,
        move |item| {
          if item == 2 {
            inner.set(Some(Signal::new(item)));
          }

          Text::new(item.to_string())
        },
      )
      .build()
  });

  assert_eq!(
    outline(root),
    "Box\n  Run \"1\"\n  Run \"2\"\n  Run \"3\"\n"
  );

  let before = children(root);
  let inner = inner.get().expect("the second item built");

  assert_eq!(before.len(), 5);
  assert_eq!(inner.try_get(), Some(2));

  set_items.set(vec![3, 1]);

  let after = children(root);

  assert_eq!(outline(root), "Box\n  Run \"3\"\n  Run \"1\"\n");
  assert_eq!(after, [before[0], before[3], before[1], before[4]]);
  assert_eq!(inner.try_get(), None);
  assert_eq!(text(before[2]), None);

  owner.dispose();
}

/// A list derived from a store's entries updates an item in place when its entry is
/// written, and follows a reorder of the `Vec` by the elements' keys.
#[test]
fn derived_entries_update_in_place_and_follow_a_reorder() {
  #[derive(Store, Clone)]
  struct Row {
    id: u32,
    label: String,
  }

  reset();

  let owner = Owner::new();
  let rows = Store::opaque(vec![
    Row {
      id: 1,
      label: "a".to_owned(),
    },
    Row {
      id: 2,
      label: "b".to_owned(),
    },
  ]);
  let root = owner.enter(|| {
    Column::new()
      .derive_entries(rows.keyed_by(|row| row.id), |row| {
        Text::derive(move || row.label().get())
      })
      .build()
  });
  let before = children(root);

  assert_eq!(outline(root), "Box\n  Run \"a\"\n  Run \"b\"\n");

  rows
    .keyed_by(|row| row.id)
    .key(1)
    .label()
    .set("A".to_owned());

  assert_eq!(outline(root), "Box\n  Run \"A\"\n  Run \"b\"\n");
  assert_eq!(children(root), before);

  rows.update(|rows| rows.swap(0, 1));

  assert_eq!(outline(root), "Box\n  Run \"b\"\n  Run \"A\"\n");
  assert_eq!(children(root), [before[0], before[2], before[1], before[3]]);

  rows.update(|rows| rows.retain(|row| row.id != 1));

  assert_eq!(outline(root), "Box\n  Run \"b\"\n");
  assert_eq!(children(root), [before[0], before[2], before[3]]);

  owner.dispose();
}

/// A click on an inner container reaches the outer container's handler, unless the
/// inner one stops it.
#[test]
fn click_bubbles_to_the_outer_container_until_stopped() {
  reset();

  let owner = Owner::new();
  let log = Rc::new(RefCell::new(Vec::new()));
  let tree = |stop: bool| {
    let outer_log = Rc::clone(&log);
    let inner_log = Rc::clone(&log);

    Container::new()
      .handle_click(move || outer_log.borrow_mut().push("outer"))
      .set_child(Container::new().handle_click(move || {
        inner_log.borrow_mut().push("inner");

        if stop {
          Bubbling::Stop
        }
        else {
          Bubbling::Continue
        }
      }))
      .build()
  };
  let bubbling = owner.enter(|| tree(false));
  let inner = only_child(bubbling);

  assert_eq!(listened(inner), [EventKind::Click]);
  assert_eq!(trigger_click(inner), Prevented::default());
  assert_eq!(*log.borrow(), ["inner", "outer"]);

  log.borrow_mut().clear();

  let stopping = owner.enter(|| tree(true));

  assert_eq!(
    trigger_click(only_child(stopping)),
    Prevented {
      platform: false,
      bubbling: true,
    }
  );
  assert_eq!(*log.borrow(), ["inner"]);

  owner.dispose();
}

/// A box under a nested theme resolves the inner palette's token; one under the outer
/// theme alone resolves the outer's, and the context reads the palette in force.
#[test]
fn nested_themes_resolve_the_nearest_palette() {
  reset();

  let owner = Owner::new();
  let seen = Rc::new(Cell::new(None::<TestPalette>));
  let root = owner.enter(|| {
    let seen = Rc::clone(&seen);

    Theme::new(TestPalette::default())
      .set_child_builder(move || {
        Column::new().set_children((
          Container::new().set_color(TestPalette::CARD),
          Theme::new(TestPalette::with_card(red())).set_child_builder(move || {
            seen.set(Some(<TestPalette as Themed<TokenValue>>::current()));

            Container::new().set_color(TestPalette::CARD)
          }),
        ))
      })
      .build()
  });

  assert_eq!(outline(root), "Box\n  Box\n  Box\n");
  assert_eq!(
    full_outline(root),
    "Scope\n  Box\n    Anchor\n    Box\n    Scope\n      Box\n    Anchor\n"
  );

  let column = children(root)[0];
  let outer_box = children(column)[1];
  let inner_scope = children(column)[2];
  let inner_box = children(inner_scope)[0];

  assert_eq!(resolve_color(outer_box, COLOR), Some(WHITE));
  assert_eq!(resolve_color(inner_box, COLOR), Some(red()));
  assert_eq!(seen.take(), Some(TestPalette::with_card(red())));
  assert_eq!(resolve_length(outer_box, WIDTH), None);

  owner.dispose();
}

/// A palette write redefines the theme's tokens on the same nodes: the box keeps its
/// id, its resolved color changes, and state inside the child survives.
#[test]
fn derived_theme_redefines_without_rebuilding() {
  reset();

  let owner = Owner::new();
  let palette = Signal::opaque(TestPalette::default());
  let local = Rc::new(Cell::new(None::<Signal<i32>>));
  let root = owner.enter(|| {
    let local = Rc::clone(&local);

    Theme::derive(move || palette.get())
      .set_child_builder(move || {
        local.set(Some(Signal::new(7)));

        Container::new().set_color(TestPalette::CARD)
      })
      .build()
  });
  let card = children(root)[0];
  let local = local.get().expect("the child built");

  local.set(9);

  assert_eq!(resolve_color(card, COLOR), Some(WHITE));

  palette.set(TestPalette::with_card(red()));

  assert_eq!(children(root), [card]);
  assert_eq!(resolve_color(card, COLOR), Some(red()));
  assert_eq!(local.try_get(), Some(9));

  owner.dispose();
}

/// An expression over a token resolves through the token's definition, and a length
/// expression follows the viewport the seams set.
#[test]
fn expressions_resolve_over_tokens_and_the_environment() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Theme::new(TestPalette::default())
      .set_child_builder(|| {
        Container::new()
          .set_color(Expr::from(TestPalette::CARD).with_alpha(0.5))
          .set_width(Expr::<Length>::vw(50.))
          .set_padding(Inset::all(4.))
      })
      .build()
  });
  let card = children(root)[0];

  assert_eq!(resolve_color(card, COLOR), Some(WHITE.with_alpha(0.5)));
  assert_eq!(resolve_length(card, WIDTH), Some(512.));
  assert_eq!(resolve_length(card, PADDING), Some(4.));

  trigger_resize(Size::new(800., 600.));

  assert_eq!(resolve_length(card, WIDTH), Some(400.));
  assert_eq!(Headless::media().viewport.get(), Size::new(800., 600.));

  owner.dispose();
}

/// A size observation reports each measured size until it is stopped, and nothing after.
#[test]
fn a_stopped_size_observation_hears_nothing_more() {
  reset();

  let owner = Owner::new();
  let node = owner.enter(|| Container::new().build());
  let heard = Rc::new(RefCell::new(Vec::new()));
  let observation = {
    let heard = Rc::clone(&heard);

    Headless::observe_size(node, Rc::new(move |size| heard.borrow_mut().push(size)))
  };

  trigger_size(node, Size::new(10., 20.));
  observation.stop();
  trigger_size(node, Size::new(30., 40.));

  assert_eq!(*heard.borrow(), [Size::new(10., 20.)]);

  owner.dispose();
}

/// A container condition resolves against the named container's size, whether the
/// seam sets it or the box reports it as its measured size.
#[test]
fn container_condition_resolves_against_the_named_containers_size() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Container::new()
      .set_container_name(SIDEBAR)
      .set_child(Container::new().set_color(WHITE.when(
        Condition::from(Media::Container {
          name: Some(SIDEBAR),
          query: ContainerQuery::MinWidth(400.),
        }),
        red(),
      )))
      .build()
  });
  let child = only_child(root);

  assert_eq!(
    box_props(root).and_then(|props| props.container),
    Some(SIDEBAR)
  );
  assert_eq!(resolve_color(child, COLOR), Some(WHITE));

  trigger_container_size(SIDEBAR, Size::new(300., 100.));

  assert_eq!(resolve_color(child, COLOR), Some(WHITE));

  trigger_container_size(SIDEBAR, Size::new(500., 100.));

  assert_eq!(resolve_color(child, COLOR), Some(red()));

  trigger_size(root, Size::new(300., 100.));

  assert_eq!(resolve_color(child, COLOR), Some(WHITE));
  assert_eq!(
    Headless::media().container_sizes.get(SIDEBAR),
    Some(Size::new(300., 100.))
  );

  owner.dispose();
}

/// A derived slot is rewritten when its signal changes, and a hover variant on the same
/// property still wins over it while the node is hovered.
#[test]
fn derived_slot_updates_and_a_hover_variant_still_wins() {
  reset();

  let owner = Owner::new();
  let color = Signal::new(red());
  let node = owner.enter(|| {
    Container::new()
      .set_color(Value::new(Leaf::derive(move || color.get())).when(hovered(), blue()))
      .build()
  });
  let slot = |node| slot_leaf(node, Slot(0));

  assert_eq!(
    slot(node),
    Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::Color(red()))))
  );
  assert_eq!(resolve_color(node, COLOR), Some(red()));

  color.set(green());

  assert_eq!(
    slot(node),
    Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::Color(green()))))
  );
  assert_eq!(resolve_color(node, COLOR), Some(green()));

  trigger_pointer_enter(node);

  assert_eq!(resolve_color(node, COLOR), Some(blue()));

  trigger_pointer_leave(node);

  assert!(!is_hovered(node));
  assert_eq!(resolve_color(node, COLOR), Some(green()));

  trigger_pointer_enter(node);

  assert!(is_hovered(node));
  assert_eq!(resolve_color(node, COLOR), Some(blue()));

  owner.dispose();
}

/// A derived leaf may name a token or an expression: the slot holds the leaf, the
/// property resolves it through the theme in force, and a write switches between them.
#[test]
fn a_derived_token_resolves_through_the_theme() {
  reset();

  let owner = Owner::new();
  let muted = Signal::new(false);
  let root = owner.enter(|| {
    Theme::new(TestPalette::default())
      .set_child_builder(move || {
        Container::new().derive_color(move || {
          if muted.get() {
            StaticLeaf::from(Expr::from(TestPalette::CARD).with_alpha(0.5))
          }
          else {
            StaticLeaf::from(TestPalette::CARD)
          }
        })
      })
      .build()
  });
  let card = children(root)[0];

  assert_eq!(
    slot_leaf(card, Slot(0)),
    Some(StaticLeaf::Token(TokenName::new("card")))
  );
  assert_eq!(resolve_color(card, COLOR), Some(WHITE));

  muted.set(true);

  assert!(matches!(
    slot_leaf(card, Slot(0)),
    Some(StaticLeaf::Expr(_))
  ));
  assert_eq!(resolve_color(card, COLOR), Some(WHITE.with_alpha(0.5)));

  trigger_color_scheme(skene_core::ColorScheme::Dark);

  assert_eq!(
    resolve_color(card, COLOR),
    Some(Color::srgb(0., 0., 0.).with_alpha(0.5)),
    "the token answers the scheme where the slot is read"
  );

  owner.dispose();
}
/// The scheme seam moves the scheme signal and picks a token's dark definition.
#[test]
fn scheme_seam_drives_the_dark_definition_and_the_signal() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Theme::new(TestPalette::default())
      .set_child_builder(|| Container::new().set_color(TestPalette::CARD))
      .build()
  });
  let card = children(root)[0];

  assert_eq!(resolve_color(card, COLOR), Some(WHITE));
  assert_eq!(
    Headless::media().scheme.get(),
    skene_core::ColorScheme::Light
  );

  trigger_color_scheme(skene_core::ColorScheme::Dark);

  assert_eq!(resolve_color(card, COLOR), Some(Color::srgb(0., 0., 0.)));
  assert_eq!(
    Headless::media().scheme.get(),
    skene_core::ColorScheme::Dark
  );
  assert_eq!(
    TestPalette::default().card.definition(),
    &Definition::from(WHITE).when(Media::Dark, StaticLeaf::Fixed(Color::srgb(0., 0., 0.)))
  );
  assert_eq!(
    TestPalette::GUTTER.name(),
    TestPalette::default().gutter.name()
  );

  owner.dispose();
}

/// A run's text color resolves like a box's, tokens included.
#[test]
fn run_color_resolves_through_the_theme() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Theme::new(TestPalette::with_card(green()))
      .set_child_builder(|| Text::new("t").set_color(TestPalette::CARD))
      .build()
  });
  let run = children(root)[0];

  assert_eq!(
    resolve_color(run, PropertyKey::Shared(Property::TextColor)),
    Some(green())
  );
  assert_eq!(resolve_color(run, COLOR), None);

  owner.dispose();
}

/// The value and focus seams record what a control's platform would hold, and a
/// destroyed node holds nothing.
#[test]
fn state_seams_record_value_focus_and_hover() {
  reset();

  let owner = Owner::new();
  let node = owner.enter(|| Container::new().set_width(Dimension::Length(10.)).build());

  assert_eq!(value(node), Some(String::new()));
  assert!(!is_focused(node));

  trigger_change(node, "typed");
  trigger_focus(node);

  assert_eq!(value(node), Some("typed".to_owned()));
  assert!(is_focused(node));
  assert_eq!(resolve_length(node, WIDTH), Some(10.));

  Scene::<Headless>::with(|scene| scene.destroy(node));

  assert_eq!(value(node), None);
  assert!(!is_focused(node));

  owner.dispose();
}

/// A frame runs the actions due by its reading earliest due first, whatever order they
/// were scheduled in, and a cancelled one not at all.
#[test]
fn due_actions_run_earliest_first() {
  reset();

  let ran = Rc::new(RefCell::new(Vec::new()));
  let action = |label: &'static str| -> Box<dyn FnOnce()> {
    let ran = Rc::clone(&ran);

    Box::new(move || ran.borrow_mut().push(label))
  };
  let _ = Headless::schedule(std::time::Duration::from_secs(2), action("late"));
  let _ = Headless::schedule(std::time::Duration::from_secs(1), action("early"));
  let cancelled = Headless::schedule(std::time::Duration::from_secs(1), action("cancelled"));

  Headless::cancel(cancelled);
  trigger_frame(std::time::Duration::from_secs(3));

  assert_eq!(*ran.borrow(), ["early", "late"]);
}

/// Reduced transparency and more contrast answer as the platform reports them, in a
/// property's condition and through a watch.
#[test]
fn the_transparency_and_contrast_preferences_answer() {
  reset();

  let owner = Owner::new();
  let (node, contrast) = owner.enter(|| {
    let node = Container::new()
      .set_color(
        Value::from(red())
          .when(Media::ReducedTransparency, green())
          .when(Media::HighContrast, blue()),
      )
      .build();

    (
      node,
      skene_common::media::watch::<Headless>(&Media::HighContrast),
    )
  });

  assert_eq!(resolve_color(node, COLOR), Some(red()));
  assert!(!contrast.get());

  trigger_reduced_transparency(true);

  assert_eq!(resolve_color(node, COLOR), Some(green()));
  assert!(skene_common::media::reduced_transparency::<Headless>().get());

  trigger_high_contrast(true);

  assert_eq!(resolve_color(node, COLOR), Some(blue()));
  assert!(contrast.get());

  owner.dispose();
}

/// The primary pointer's precision and whether it hovers answer as the platform
/// reports them, in a property's condition and through a watch.
#[test]
fn the_pointer_precision_and_hover_answer() {
  reset();

  let owner = Owner::new();
  let (node, hovers) = owner.enter(|| {
    let node = Container::new()
      .set_color(
        Value::from(red())
          .when(Media::Pointer(PointerPrecision::Coarse), green())
          .when(Media::Pointer(PointerPrecision::Absent), blue()),
      )
      .build();

    (node, skene_common::media::watch::<Headless>(&Media::Hover))
  });

  assert_eq!(resolve_color(node, COLOR), Some(red()));
  assert!(hovers.get(), "a fine pointer that hovers by default");

  trigger_pointer_precision(PointerPrecision::Coarse);
  trigger_pointer_hovers(false);

  assert_eq!(resolve_color(node, COLOR), Some(green()));
  assert_eq!(
    skene_common::media::pointer::<Headless>().get(),
    PointerPrecision::Coarse
  );
  assert!(!hovers.get());

  trigger_pointer_precision(PointerPrecision::Absent);

  assert_eq!(resolve_color(node, COLOR), Some(blue()));

  owner.dispose();
}

/// A length reading the safe area follows the insets the platform reports, as the
/// signal does.
#[test]
fn a_length_reads_the_safe_area() {
  reset();

  let owner = Owner::new();
  let node = owner.enter(|| {
    Container::new()
      .set_padding(Expr::<Length>::safe_area(Edge::Top) + 4.)
      .build()
  });

  assert_eq!(resolve_length(node, PADDING), Some(4.));

  trigger_safe_area(Inset {
    top: 44.,
    right: 0.,
    bottom: 34.,
    left: 0.,
  });

  assert_eq!(resolve_length(node, PADDING), Some(48.));
  assert!((skene_common::media::safe_area::<Headless>().get().bottom - 34.).abs() < f32::EPSILON);

  owner.dispose();
}
