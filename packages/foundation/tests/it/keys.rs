//! View keys: a keyed view keeps its state across a reorder among its siblings, and a
//! keyed list item keeps its own across a reorder of the list, on the snapshot.

use std::{cell::RefCell, rc::Rc};

use skene_foundation::{HasChild, HasChildren, View};
use skene_headless::{Headless, children, reset, text};
use skene_reactive::{Get, Owner, Set, Signal, record_snapshots, restore, snapshot};

type Column = skene_foundation::Column<Headless>;
type Container = skene_foundation::Container<Headless>;
type Text = skene_foundation::Text<Headless>;

/// Runs `first`, then `second` on a fresh runtime with `first`'s snapshot staged.
fn across_runs<R: Send + 'static>(
  first: impl FnOnce() + Send + 'static,
  second: impl FnOnce() -> R + Send + 'static,
) -> R {
  let bytes = std::thread::spawn(move || {
    reset();
    record_snapshots();
    first();

    snapshot()
  })
  .join()
  .expect("the first run");

  std::thread::spawn(move || {
    reset();
    restore(&bytes);

    second()
  })
  .join()
  .expect("the second run")
}

/// The text of every run under `node`, in tree order.
fn texts_under(node: skene_foundation::NodeId) -> Vec<String> {
  children(node)
    .into_iter()
    .flat_map(|child| text(child).map_or_else(|| texts_under(child), |text| vec![text]))
    .collect()
}

/// A container named by `name` whose content creates one signal, pushed to `signals`.
fn keyed_card(name: &'static str, signals: &Rc<RefCell<Vec<Signal<u32>>>>) -> impl View<Headless> {
  let signals = Rc::clone(signals);

  Container::new().set_key(name).derive_child(move || {
    let count = Signal::saved(0u32);

    signals.borrow_mut().push(count);

    Text::new(name)
  })
}

/// A keyed view restores by its key: reordered among its siblings, each finds its own
/// state, and the key adds no node.
#[test]
fn a_keyed_view_restores_by_key_among_its_siblings() {
  fn build(order: [&'static str; 3]) -> Vec<Signal<u32>> {
    let signals = Rc::new(RefCell::new(Vec::new()));
    let owner = Owner::new();
    let column = owner.enter(|| {
      Column::new()
        .set_children(order.map(|name| keyed_card(name, &signals)))
        .build()
    });
    let cards = children(column)
      .into_iter()
      .filter(|node| !texts_under(*node).is_empty())
      .count();

    assert_eq!(
      cards, 3,
      "the key added no node between the column and its cards"
    );
    assert_eq!(texts_under(column), order);

    signals.take()
  }

  let restored = across_runs(
    || {
      for (signal, value) in build(["a", "b", "c"]).into_iter().zip([10, 20, 30]) {
        signal.set(value);
      }
    },
    || {
      build(["c", "a", "d"])
        .into_iter()
        .map(|signal| signal.get())
        .collect::<Vec<_>>()
    },
  );

  assert_eq!(restored, [30, 10, 0]);
}

/// A keyed list item's state restores by the item's key: the list reordered, each
/// item finds its own.
#[test]
fn a_keyed_list_item_restores_by_its_key() {
  fn build(items: Vec<u8>) -> Vec<Signal<u32>> {
    let signals = Rc::new(RefCell::new(Vec::new()));
    let owner = Owner::new();
    let _column = owner.enter(|| {
      let signals = Rc::clone(&signals);

      Column::new()
        .derive_children(
          move || items.clone(),
          |item| *item,
          move |item| {
            let count = Signal::saved(0u32);

            signals.borrow_mut().push(count);

            Text::new(item.to_string())
          },
        )
        .build()
    });

    signals.take()
  }

  let restored = across_runs(
    || {
      for (signal, value) in build(vec![1, 2, 3]).into_iter().zip([10, 20, 30]) {
        signal.set(value);
      }
    },
    || {
      build(vec![3, 1, 4])
        .into_iter()
        .map(|signal| signal.get())
        .collect::<Vec<_>>()
    },
  );

  assert_eq!(restored, [30, 10, 0]);
}
