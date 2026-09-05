//! Headless view tests: the GUI facade's views realized against the headless driver,
//! built and driven without a browser.

#![cfg(not(target_arch = "wasm32"))]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_gui::{
  Bubbling,
  Column,
  Container,
  HasChildren,
  LayoutBox,
  Overlay,
  RadioMember,
  RawRadio,
  RawRadioGroup,
  RawTextField,
  Role,
  Roving,
  Row,
  Text,
  TextEntry,
  View,
  core::{Color, Dimension, FlexChild, FlexDirection, Layout, NoProperty, Property, PropertyKey},
  headless::{
    box_props,
    children,
    is_open,
    is_tab_stop,
    outline,
    reset,
    resolve_color,
    resolve_length,
    text,
    trigger_change,
    trigger_click,
    trigger_dismiss,
  },
  prelude::{Get, Owner, Set, Signal},
};

const WIDTH: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Width);
const HEIGHT: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Height);
const COLOR: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Color);

/// The nodes between a container's anchors: its content, in order.
fn content(node: NodeId) -> Vec<NodeId> {
  let nodes = children(node);

  nodes[1..nodes.len() - 1].to_vec()
}

use skene_gui::NodeId;

/// A static tree records its structure: kinds, order, and text content; a wrapper's
/// anchors take no place in the layout's outline.
#[test]
fn tree_structure_is_recorded() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Container::new()
      .set_role(Role::Navigation)
      .set_child(Column::new().set_children((
        Text::new("first"),
        Row::new().set_children((Text::new("second"),)),
      )))
      .build()
  });

  assert_eq!(
    outline(root),
    "Box\n  Box\n    Run \"first\"\n    Box\n      Run \"second\"\n"
  );
  assert_eq!(
    box_props(root).and_then(|props| props.role),
    Some(Role::Navigation)
  );

  owner.dispose();
}

/// A static property lands in the compiled props; a derived one writes its slot and
/// updates it on change.
#[test]
fn derived_property_updates_the_resolved_style() {
  reset();

  let owner = Owner::new();
  let width = Signal::new(100.0f32);
  let root = owner.enter(|| {
    Container::new()
      .set_height(40.)
      .derive_width(move || Dimension::Length(width.get()))
      .build()
  });

  assert_eq!(resolve_length(root, HEIGHT), Some(40.));
  assert_eq!(resolve_length(root, WIDTH), Some(100.));

  width.set(160.);

  assert_eq!(resolve_length(root, WIDTH), Some(160.));

  owner.dispose();
}

/// Derived text rewrites the run in place on change.
#[test]
fn derived_text_updates_the_run() {
  reset();

  let owner = Owner::new();
  let count = Signal::new(1);
  let root = owner.enter(|| {
    Container::new()
      .set_child(Text::derive(move || format!("count: {}", count.get())))
      .build()
  });
  let run = content(root)[0];

  assert_eq!(text(run).as_deref(), Some("count: 1"));

  count.set(2);

  assert_eq!(text(run).as_deref(), Some("count: 2"));
  assert_eq!(content(root)[0], run, "the same run, rewritten");

  owner.dispose();
}

/// A click runs the handlers deepest first; a box that stops bubbling keeps the event
/// from its ancestors, and no box states anything about the platform.
#[test]
fn click_bubbles_until_a_box_stops_it() {
  reset();

  let owner = Owner::new();
  let log = Rc::new(RefCell::new(Vec::new()));
  let note = |name: &'static str| {
    let log = Rc::clone(&log);

    move || {
      log.borrow_mut().push(name);
    }
  };
  let root = owner.enter(|| {
    Container::new()
      .handle_click(note("outer"))
      .set_child(
        Container::new()
          .handle_click({
            let note = note("middle");

            move || {
              note();

              Bubbling::Stop
            }
          })
          .set_child(Container::new().handle_click(note("inner"))),
      )
      .build()
  });
  let middle = content(root)[0];
  let inner = content(middle)[0];
  let verdict = trigger_click(inner);

  assert_eq!(*log.borrow(), ["inner", "middle"]);
  assert!(verdict.bubbling, "the middle box kept it");
  assert!(!verdict.platform, "and stated nothing about the platform");

  owner.dispose();
}

/// A change trigger commits the value into a text field's binding.
#[test]
fn change_trigger_commits_into_the_binding() {
  reset();

  let owner = Owner::new();
  let value = Signal::new(String::new());
  let field = owner.enter(|| RawTextField::new(TextEntry::new().bind_value(value)).build());

  trigger_change(field, "typed");

  assert_eq!(value.get(), "typed");

  owner.dispose();
}

/// An overlay opens and closes with its bound signal, and a dismiss trigger requests
/// closing through the handler rather than closing on its own.
#[test]
fn overlay_open_follows_its_signal() {
  reset();

  let owner = Owner::new();
  let open = Signal::new(false);
  let root = owner.enter(|| {
    Container::new()
      .set_child(
        Overlay::modal()
          .bind_open(open)
          .set_child(Text::new("dialog")),
      )
      .build()
  });
  let overlay = content(root)[0];

  assert!(!is_open(overlay));

  open.set(true);

  assert!(is_open(overlay));

  trigger_dismiss(overlay);

  assert!(!open.get());
  assert!(!is_open(overlay));

  owner.dispose();
}

/// Keyed children keep the nodes whose keys stay and reorder them, rebuilding nothing.
#[test]
fn keyed_children_reuse_and_reorder() {
  reset();

  let owner = Owner::new();
  let items = Signal::new(vec![1, 2, 3]);
  let column = owner.enter(|| {
    Column::new()
      .derive_children(
        move || items.get(),
        |item: &i32| *item,
        |item| Text::new(item.to_string()),
      )
      .build()
  });
  let before = content(column);

  assert_eq!(before.len(), 3);

  items.set(vec![3, 1]);

  let after = content(column);

  assert_eq!(after.len(), 2);
  assert_eq!(after[0], before[2], "the kept node moved, not rebuilt");
  assert_eq!(after[1], before[0]);
  assert_eq!(text(after[0]).as_deref(), Some("3"));

  owner.dispose();
}

/// A radio group's roster orders its members by the tree, so the one tab stop follows
/// the choice and a member inserted first is first.
#[test]
fn roster_orders_members_in_tree_order() {
  reset();

  let owner = Owner::new();
  let chosen: Signal<Option<String>> = Signal::new(None);
  let items = Signal::opaque(vec!["a", "b"]);
  let group = owner.enter(|| {
    RawRadioGroup::new(Roving::new().bind_chosen(chosen))
      .set_child(Column::new().derive_children(
        move || items.get(),
        |item| *item,
        |item| RawRadio::new(RadioMember::new(item)).set_label(item),
      ))
      .build()
  });
  let column = children(group)[0];
  let member = |index: usize| content(column)[index];

  assert!(is_tab_stop(member(0)));
  assert!(!is_tab_stop(member(1)));

  let _ = trigger_click(member(1));

  assert_eq!(chosen.get().as_deref(), Some("b"));
  assert!(!is_tab_stop(member(0)));
  assert!(is_tab_stop(member(1)));

  chosen.set(None);
  items.set(vec!["z", "a", "b"]);

  assert!(is_tab_stop(member(0)));
  assert!(!is_tab_stop(member(1)));
  assert!(!is_tab_stop(member(2)));

  owner.dispose();
}

/// Decoration resolves as given.
#[test]
fn decoration_resolves_verbatim() {
  reset();

  let owner = Owner::new();
  let color = Color::srgb(0.25, 0.5, 0.75);
  let root = owner.enter(|| Container::new().set_color(color).build());

  assert_eq!(resolve_color(root, COLOR), Some(color));

  owner.dispose();
}

/// The equal-footing gate: a layout view written outside the facade, against its
/// public API alone, realizes the same box the built-in it parallels does.
#[test]
fn a_third_party_layout_view_matches_the_built_in() {
  #[must_use]
  struct Toolbar {
    base: LayoutBox<FlexChild>,
  }

  impl Toolbar {
    fn new() -> Self {
      Self {
        base: LayoutBox::new(Layout::Flex {
          direction: FlexDirection::Row,
        }),
      }
    }

    fn set_spacing(mut self, spacing: f32) -> Self {
      self.base.style_mut().spacing = Some(skene_gui::Spacing::from(spacing).into());

      self
    }
  }

  impl skene_gui::AnyKind for Toolbar {}

  impl skene_gui::core::View<skene_gui::ActiveDriver> for Toolbar {
    fn build(self) -> NodeId {
      skene_gui::View::build(self.base)
    }
  }

  reset();

  let owner = Owner::new();
  let clicks = Rc::new(Cell::new(0));
  let (toolbar, row) = owner.enter(|| {
    let _ = &clicks;

    (
      Toolbar::new().set_spacing(8.).build(),
      Row::new().set_spacing(8.).build(),
    )
  });
  let (toolbar, row) = (box_props(toolbar).unwrap(), box_props(row).unwrap());

  assert_eq!(toolbar.layout, row.layout);
  assert_eq!(toolbar.style, row.style);

  owner.dispose();
}
