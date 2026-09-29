//! The lazy lists: only the window plus its overscan builds, scrolling rebuilds the
//! window reusing the items whose keys stay, a resize widens it, measured items reshape
//! it, and a row does the same along the other axis.

mod common;

use std::{cell::Cell, rc::Rc};

use skene_common::ItemExtent;
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
const WIDTH: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Width);
const ITEM_HEIGHT: f32 = 10.;
const OVERSCAN: usize = 3;
const COUNT: usize = 10_000;

type LazyColumn = skene_common::LazyColumn<Headless>;
type LazyRow = skene_common::LazyRow<Headless>;
type Text = skene_common::Text<Headless>;

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
#[expect(
  clippy::cast_precision_loss,
  reason = "the tests' item counts stay far below the integers f32 holds exactly"
)]
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

/// Estimated items build by the estimate, then each measure reshapes the window and the
/// spacers: the edges follow the measured items, and an item scrolled out keeps its
/// measure.
#[test]
fn measured_items_reshape_the_window() {
  reset();

  let owner = Owner::new();
  let node = owner.enter(|| {
    LazyColumn::new()
      .set_item_height(ItemExtent::Estimated(10.))
      .set_overscan(0)
      .derive_items(
        || 0..100,
        |index| *index,
        |index| Text::new(index.to_string()),
      )
      .build()
  });

  trigger_size(node, Size::new(300., 50.));

  let (_, estimated, trailing) = slices(node);

  assert_eq!(
    estimated.len(),
    5,
    "five items of the estimate fill the viewport"
  );
  assert_eq!(resolve_length(trailing, HEIGHT), Some(950.));
  assert_eq!(
    resolve_length(estimated[0], HEIGHT),
    None,
    "an item takes its own height"
  );

  for item in &estimated {
    trigger_size(*item, Size::new(300., 20.));
  }

  let (leading, measured, trailing) = slices(node);

  assert_eq!(measured.len(), 3, "three measured items fill the viewport");
  assert_eq!(measured[..], estimated[..3]);
  assert_eq!(resolve_length(leading, HEIGHT), Some(0.));
  assert_eq!(
    resolve_length(trailing, HEIGHT),
    Some(3. * 20. + 97. * 10. - 60.),
    "the trailing spacer counts the three measures; the two items the first measures \
     pushed out of the window were disposed before they could measure"
  );

  trigger_scroll(node, &ScrollEvent { x: 0., y: 40. });

  let (leading, scrolled, trailing) = slices(node);

  assert_eq!(label(scrolled[0]), "2");
  assert_eq!(
    scrolled.len(),
    4,
    "two measured and two estimated items fill it"
  );
  assert_eq!(
    resolve_length(leading, HEIGHT),
    Some(40.),
    "the two items scrolled out keep their measures"
  );
  assert_eq!(
    resolve_length(trailing, HEIGHT),
    Some(3. * 20. + 97. * 10. - 90.)
  );

  owner.dispose();
}

/// A lazy row windows along the horizontal axis: its width is the viewport, its scroll
/// offset the horizontal one, and its spacers take widths.
#[test]
fn a_lazy_row_windows_along_x() {
  reset();

  let owner = Owner::new();
  let node = owner.enter(|| {
    LazyRow::new()
      .set_item_width(ITEM_HEIGHT)
      .set_overscan(OVERSCAN)
      .derive_items(
        || 0..COUNT,
        |index| *index,
        |index| Text::new(index.to_string()),
      )
      .build()
  });

  trigger_size(node, Size::new(100., 300.));

  let (_, window, trailing) = slices(node);

  assert_eq!(window.len(), 13);
  assert_eq!(resolve_length(window[0], WIDTH), Some(ITEM_HEIGHT));
  assert_eq!(resolve_length(window[0], HEIGHT), None);
  assert_eq!(resolve_length(trailing, WIDTH), Some(extent(COUNT - 13)));

  trigger_scroll(node, &ScrollEvent { x: 500., y: 0. });

  let (leading, scrolled, _) = slices(node);

  assert_eq!(label(scrolled[0]), "47");
  assert_eq!(resolve_length(leading, WIDTH), Some(extent(47)));

  owner.dispose();
}
