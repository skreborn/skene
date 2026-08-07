//! Headless view tests: the GUI facade's views realized against the recording driver,
//! built and driven without a browser.

#![cfg(not(target_arch = "wasm32"))]

use std::{cell::Cell, rc::Rc};

use skene_gui::{
  Container,
  HasChildren,
  Input,
  Overlay,
  Text,
  View,
  core::{Color, Dimension, Prevent, Role},
  prelude::{Get, Set, Signal},
  record,
};

/// A static tree records its structure: kinds, order, and text content.
#[test]
fn tree_structure_is_recorded() {
  let root = Container::column()
    .set_role(Role::Navigation)
    .set_children((
      Text::new("first"),
      Container::row().set_children((Text::new("second"),)),
    ))
    .build();

  assert_eq!(
    record::outline(root),
    "Box\n  Run \"first\"\n  Box\n    Run \"second\"\n"
  );
  assert_eq!(record::role(root), Some(Role::Navigation));
}

/// A static style lands in the recorded props; a derived property updates it on
/// change.
#[test]
fn derived_property_updates_the_recorded_style() {
  let width = Signal::new(100.0f32);
  let root = Container::new()
    .set_height(40.)
    .derive_width(move || width.get())
    .build();
  let style = record::box_style(root).expect("a box style");

  assert_eq!(style.height, Some(Dimension::Length(40.)));
  assert_eq!(style.width, Some(Dimension::Length(100.)));

  width.set(160.);

  let style = record::box_style(root).expect("a box style");

  assert_eq!(style.width, Some(Dimension::Length(160.)));
}

/// Derived text rewrites the recorded run on change.
#[test]
fn derived_text_updates_the_recorded_run() {
  let count = Signal::new(1);
  let root = Container::new()
    .set_children((Text::derive(move || format!("count: {}", count.get())),))
    .build();
  let run = record::children(root)[0];

  assert_eq!(record::text(run).as_deref(), Some("count: 1"));

  count.set(2);

  assert_eq!(record::text(run).as_deref(), Some("count: 2"));
}

/// A click trigger runs the view's handler and returns its verdict.
#[test]
fn click_trigger_runs_the_handler() {
  let clicks = Rc::new(Cell::new(0));
  let root = Container::new()
    .handle_click({
      let clicks = Rc::clone(&clicks);

      move || {
        clicks.set(clicks.get() + 1);

        Prevent::Default
      }
    })
    .build();

  let prevention = record::trigger_click(root);

  assert_eq!(clicks.get(), 1);
  assert!(prevention.default);
  assert!(!prevention.bubbling);
}

/// A change trigger commits the value into an input's binding.
#[test]
fn change_trigger_commits_into_the_binding() {
  let value = Signal::new(String::new());
  let root = Input::new().bind_value(value).build();

  record::trigger_change(root, "typed");

  assert_eq!(value.get(), "typed");
}

/// An overlay opens and closes with its bound signal, and a dismiss trigger requests
/// closing through the handler.
#[test]
fn overlay_open_follows_its_signal() {
  let open = Signal::new(false);
  let root = Container::new()
    .set_children((Overlay::modal()
      .bind_open(open)
      .set_children((Text::new("dialog"),)),))
    .build();
  let overlay = record::children(root)[0];

  assert_eq!(record::overlay_open(overlay), Some(false));

  open.set(true);

  assert_eq!(record::overlay_open(overlay), Some(true));

  record::trigger_dismiss(overlay);

  assert!(!open.get());
  assert_eq!(record::overlay_open(overlay), Some(false));
}

/// A keyed reconcile keeps unkeyed-change nodes and reorders the recorded children.
#[test]
fn reconcile_reuses_and_reorders_recorded_children() {
  let items = Signal::new(vec![1, 2, 3]);
  let parent = record::create_group();

  skene_gui::reconcile(
    parent,
    move || items.get(),
    |item: &i32| *item,
    |item| Text::new(format!("{item}")).build(),
  );

  let before = record::children(parent);

  assert_eq!(before.len(), 3);

  items.set(vec![3, 1]);

  let after = record::children(parent);

  assert_eq!(after.len(), 2);
  assert_eq!(after[0], before[2], "the kept node moved, not rebuilt");
  assert_eq!(after[1], before[0]);
  assert_eq!(record::text(after[0]).as_deref(), Some("3"));
}

/// Colors and backgrounds record as given, uninterpreted.
#[test]
fn decoration_records_verbatim() {
  let color = Color::srgb(0.25, 0.5, 0.75);
  let root = Container::new().set_color(color).build();
  let style = record::box_style(root).expect("a box style");

  assert_eq!(style.color, Some(color));
}
