//! The show: its child is built the first time it shows, hidden and shown again
//! without a rebuild, and paused while hidden.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{AnyView, NodeId, Scene, View};
use skene_headless::{Headless, children, end_turn, is_live, reset};
use skene_reactive::{Effect, Get, Owner, Set, signal};

type Container = skene_foundation::Container<Headless>;
type Visibility = skene_foundation::Visibility<Headless>;

/// A box with the segment `start` begins attached beneath it.
fn parent_of(start: NodeId) -> NodeId {
  let parent = Container::new().build();

  Scene::<Headless>::with(|scene| scene.attach(parent, start, None));

  parent
}

/// A child hidden from the start is built the first time it shows, and while hidden
/// its effects pause, catching up once, when it shows again.
#[test]
fn a_hidden_child_builds_when_first_shown_and_pauses_while_hidden() {
  reset();

  let owner = Owner::new();
  let (shown, set_shown) = signal(false);
  let (source, set_source) = signal(0);
  let builds = Rc::new(Cell::new(0));
  let runs = Rc::new(Cell::new(0));

  let _ = owner.enter(|| {
    let builds = Rc::clone(&builds);
    let runs = Rc::clone(&runs);

    Visibility::new()
      .derive_shown(move || shown.get())
      .set_child(AnyView::new(move || {
        let runs = Rc::clone(&runs);

        builds.set(builds.get() + 1);
        Effect::render(move |_: Option<()>| {
          let _ = source.get();

          runs.set(runs.get() + 1);
        });

        Container::new().build()
      }))
      .build()
  });

  end_turn();

  assert_eq!(builds.get(), 0, "a hidden child is not built");

  set_shown.set(true);
  end_turn();

  assert_eq!((builds.get(), runs.get()), (1, 1));

  set_shown.set(false);
  end_turn();
  set_source.set(1);
  set_source.set(2);
  end_turn();

  assert_eq!(runs.get(), 1, "a hidden child's effects pause");

  set_shown.set(true);
  end_turn();

  assert_eq!(
    (builds.get(), runs.get()),
    (1, 2),
    "and catch up once, when it shows"
  );

  owner.dispose();
}

/// A re-run that leaves the condition as it was keeps the child where it stands.
#[test]
fn an_unchanged_condition_keeps_the_child_where_it_stands() {
  reset();

  let owner = Owner::new();
  let (count, set_count) = signal(1);
  let start = owner.enter(|| {
    Visibility::new()
      .derive_shown(move || count.get() > 0)
      .set_child(Container::new())
      .build()
  });
  let parent = parent_of(start);
  let shown = children(parent);

  set_count.set(2);

  assert_eq!(children(parent), shown);

  owner.dispose();
}

/// Hiding detaches the child and showing reattaches the same node, never rebuilding;
/// the child is destroyed with the show's owner.
#[test]
fn show_detaches_and_reattaches_the_same_node() {
  reset();

  let owner = Owner::new();
  let (shown, set_shown) = signal(true);
  let start = owner.enter(|| {
    Visibility::new()
      .derive_shown(move || shown.get())
      .set_child(Container::new())
      .build()
  });
  let parent = parent_of(start);
  let [_, child, end] = children(parent)[..]
  else {
    panic!("a shown child sits between its show's anchors");
  };

  set_shown.set(false);

  assert_eq!(children(parent), [start, end]);
  assert!(is_live(child));

  set_shown.set(true);

  assert_eq!(children(parent), [start, child, end]);

  set_shown.set(false);
  owner.dispose();

  assert!(!is_live(child));
}
