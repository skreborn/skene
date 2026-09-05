//! The lazy column: only the window plus its overscan builds, scrolling rebuilds the
//! window reusing the items whose keys stay, and a resize widens it.

mod common;

use std::{cell::Cell, rc::Rc};

use skene_core::{EventKind, NoProperty, NodeId, Property, PropertyKey, ScrollEvent, Size, View};
use skene_headless::{
  Headless,
  children,
  listened,
  reset,
  resolve_length,
  text,
  trigger_scroll,
  trigger_size,
};
use skene_reactive::{Get, Owner, Set, Signal};

const HEIGHT: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Height);
const ITEM_HEIGHT: f32 = 10.;
const OVERSCAN: usize = 3;
const COUNT: usize = 10_000;

type LazyColumn = skene_views::LazyColumn<Headless>;
type Text = skene_views::Text<Headless>;

/// The column's children: the leading spacer, the built items in order, and the
/// trailing spacer.
fn slices(node: NodeId) -> (NodeId, Vec<NodeId>, NodeId) {
  let nodes = children(node);
  let leading = nodes[1];
  let trailing = nodes[nodes.len() - 2];
  let items = nodes[4..nodes.len() - 4].to_vec();

  (leading, items, trailing)
}

fn label(item: NodeId) -> String {
  text(children(item)[1]).expect("the item's text")
}

/// The height `items` items take up.
#[expect(clippy::cast_precision_loss)]
fn extent(items: usize) -> f32 {
  items as f32 * ITEM_HEIGHT
}

/// Ten thousand items build as the overscan alone until the column is measured, then
/// as the slice in view plus the overscan, with spacers standing in for the rest.
#[test]
fn only_the_window_plus_the_overscan_builds() {
  reset();

  let owner = Owner::new();
  let items = Signal::new((0..COUNT).collect::<Vec<usize>>());
  let built = Rc::new(Cell::new(0));
  let node = owner.enter({
    let built = Rc::clone(&built);

    move || {
      LazyColumn::new()
        .set_item_height(ITEM_HEIGHT)
        .set_overscan(OVERSCAN)
        .derive_items(
          move || items.get(),
          |index| *index,
          move |index| {
            built.set(built.get() + 1);

            Text::new(index.to_string())
          },
        )
        .build()
    }
  });

  assert!(listened(node).contains(&EventKind::Scroll));

  let (leading, window, trailing) = slices(node);

  assert_eq!(window.len(), OVERSCAN);
  assert_eq!(built.get(), OVERSCAN);
  assert_eq!(resolve_length(leading, HEIGHT), Some(0.));
  assert_eq!(
    resolve_length(trailing, HEIGHT),
    Some(extent(COUNT - OVERSCAN))
  );

  trigger_size(node, Size::new(300., 100.));

  let (_, window, trailing) = slices(node);

  assert_eq!(window.len(), 13);
  assert_eq!(built.get(), 13);
  assert_eq!(label(window[0]), "0");
  assert_eq!(label(window[12]), "12");
  assert_eq!(resolve_length(window[0], HEIGHT), Some(ITEM_HEIGHT));
  assert_eq!(resolve_length(trailing, HEIGHT), Some(extent(COUNT - 13)));

  owner.dispose();
}

/// Scrolling moves the window: the items whose keys stay keep their nodes, only the
/// newcomers build, and the spacers take up the difference.
#[test]
fn scrolling_rebuilds_the_window_with_keyed_reuse() {
  reset();

  let owner = Owner::new();
  let built = Rc::new(Cell::new(0));
  let node = owner.enter({
    let built = Rc::clone(&built);

    move || {
      LazyColumn::new()
        .set_item_height(ITEM_HEIGHT)
        .set_overscan(OVERSCAN)
        .derive_items(
          || 0..COUNT,
          |index| *index,
          move |index| {
            built.set(built.get() + 1);

            Text::new(index.to_string())
          },
        )
        .build()
    }
  });

  trigger_size(node, Size::new(300., 100.));
  trigger_scroll(node, &ScrollEvent { x: 0., y: 500. });

  let (leading, before, _) = slices(node);

  assert_eq!(before.len(), 16);
  assert_eq!(label(before[0]), "47");
  assert_eq!(label(before[15]), "62");
  assert_eq!(built.get(), 13 + 16);
  assert_eq!(resolve_length(leading, HEIGHT), Some(extent(47)));

  trigger_scroll(node, &ScrollEvent { x: 0., y: 510. });

  let (leading, after, trailing) = slices(node);

  assert_eq!(after.len(), 16);
  assert_eq!(label(after[0]), "48");
  assert_eq!(label(after[15]), "63");
  assert_eq!(after[..15], before[1..]);
  assert_eq!(built.get(), 13 + 16 + 1);
  assert_eq!(resolve_length(leading, HEIGHT), Some(extent(48)));
  assert_eq!(resolve_length(trailing, HEIGHT), Some(extent(COUNT - 64)));

  owner.dispose();
}

/// A taller viewport widens the window in place, and a shorter list shrinks it.
#[test]
fn a_resize_widens_the_window() {
  reset();

  let owner = Owner::new();
  let count = Signal::new(COUNT);
  let node = owner.enter(move || {
    LazyColumn::new()
      .set_item_height(ITEM_HEIGHT)
      .set_overscan(OVERSCAN)
      .derive_items(
        move || 0..count.get(),
        |index| *index,
        |index| Text::new(index.to_string()),
      )
      .build()
  });

  trigger_size(node, Size::new(300., 100.));

  let (_, narrow, _) = slices(node);

  assert_eq!(narrow.len(), 13);

  trigger_size(node, Size::new(300., 200.));

  let (_, wide, _) = slices(node);

  assert_eq!(wide.len(), 23);
  assert_eq!(wide[..13], narrow[..]);
  assert_eq!(label(wide[22]), "22");

  count.set(5);

  let (leading, few, trailing) = slices(node);

  assert_eq!(few.len(), 5);
  assert_eq!(few[..], wide[..5]);
  assert_eq!(resolve_length(leading, HEIGHT), Some(0.));
  assert_eq!(resolve_length(trailing, HEIGHT), Some(0.));

  owner.dispose();
}
