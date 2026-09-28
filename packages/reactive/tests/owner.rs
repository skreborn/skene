//! Ownership: disposal reaches everything an owner created, and stale handles stay
//! dead.

use std::{cell::Cell, rc::Rc};

use skene_reactive::{Effect, Get, Owner, Runtime, Set, Signal, Track, signal};

/// Disposing a dependency ends the subscription, not the subscriber: the effect keeps
/// running on its other dependencies.
#[test]
fn an_effect_outlives_a_disposed_dependency() {
  let parent = Owner::new();
  let signal_owner = parent.child();
  let (source, setter) = signal_owner.enter(|| signal(0));
  let other = parent.enter(|| Signal::new(0));
  let count = Rc::new(Cell::new(0));

  parent.enter(|| {
    let count = count.clone();

    Effect::new(move |_| {
      source.try_track();
      other.track();
      count.set(count.get() + 1);
    });
  });

  assert_eq!(count.get(), 1);

  signal_owner.dispose();
  setter.set(1);

  assert_eq!(count.get(), 1);

  other.set(1);

  assert_eq!(count.get(), 2);
}

#[test]
fn signals_created_by_effect_are_disposed_with_effect() {
  let parent = Owner::new();
  let effect_owner = parent.child();
  let (dep, dep_setter) = parent.enter(|| signal(0));
  let created = Rc::new(Cell::new(None::<Signal<i32>>));
  let runs = Rc::new(Cell::new(0));

  effect_owner.enter(|| {
    let created = created.clone();
    let runs = runs.clone();

    Effect::new(move |_| {
      dep.track();
      runs.set(runs.get() + 1);
      created.set(Some(Signal::new(0)));
    });
  });

  assert_eq!(runs.get(), 1);

  let first = created.get().expect("signal created");

  dep_setter.set(1);

  // A re-run disposes what the last run created.
  assert_eq!(runs.get(), 2);
  assert_eq!(first.try_get(), None);

  let second = created.get().expect("signal created");

  assert_eq!(second.try_get(), Some(0));

  effect_owner.dispose();
  dep_setter.set(2);

  assert_eq!(runs.get(), 2);
  assert_eq!(second.try_get(), None);
}

#[test]
fn disposing_owner_drops_signals_and_effects() {
  let owner = Owner::new();
  let (source, setter) = owner.enter(|| signal(0));
  let runs = Rc::new(Cell::new(0));

  owner.enter(|| {
    let runs = runs.clone();

    Effect::new(move |_| {
      source.track();
      runs.set(runs.get() + 1);
    });
  });

  assert_eq!(runs.get(), 1);

  owner.dispose();
  Runtime::drain_pending_work();

  setter.set(1);
  Runtime::drain_pending_work();
  assert_eq!(runs.get(), 1);
  assert_eq!(source.try_get(), None);
}

#[test]
fn set_parent_disposes_with_new_parent() {
  let parent = Owner::new();
  let child = Owner::new();

  assert!(child.parent().is_none());

  let source = child.enter(|| Signal::new(42));

  child.set_parent(parent);

  assert_eq!(child.parent(), Some(parent));
  assert_eq!(source.try_get(), Some(42));

  parent.dispose();

  assert_eq!(source.try_get(), None);
}

#[test]
fn set_parent_removes_from_old_parent() {
  let old_parent = Owner::new();
  let new_parent = Owner::new();
  let child = old_parent.child();
  let source = child.enter(|| Signal::new(1));

  child.set_parent(new_parent);

  // Disposing the old parent no longer reaches the child.
  old_parent.dispose();

  assert_eq!(source.try_get(), Some(1));

  new_parent.dispose();

  assert_eq!(source.try_get(), None);
}

/// The arena reuses a disposed signal's slot without resurrecting its old handles.
#[test]
fn stale_handle_stays_dead_after_slot_reuse() {
  let owner = Owner::new();
  let source = owner.enter(|| Signal::new(7u32));

  owner.dispose();

  assert_eq!(source.try_get(), None);

  // New signals may land in the freed slot; the stale handle must not see them.
  let fresh: Vec<Signal<u32>> = (100..116).map(Signal::new).collect();

  assert_eq!(source.try_get(), None);
  assert_eq!(source.try_get_untracked(), None);

  for (signal, value) in fresh.iter().zip(100..) {
    assert_eq!(signal.try_get(), Some(value));
  }
}

/// A disposed child leaves no path from its parent: disposing the parent afterwards
/// is a no-op for the child's already-freed slot.
#[test]
fn dispose_unlinks_from_parent_children() {
  let parent = Owner::new();
  let child = parent.child();
  let source = child.enter(|| Signal::new(3));

  child.dispose();

  assert_eq!(source.try_get(), None);

  // The freed slots may already be reused; parent disposal must not touch them.
  let bystander = Signal::new(9);

  parent.dispose();

  assert_eq!(bystander.try_get(), Some(9));
}

/// A rebuild in place, as `derived` does: each run builds under a scope taking the last
/// one's place and disposes the last one, while reading state the new build created.
#[test]
fn a_rebuild_reading_its_own_content_keeps_running() {
  let parent = Owner::new();
  let trigger = Signal::new(0);
  let current = Rc::new(Cell::new(None::<Owner>));
  let runs = Rc::new(Cell::new(0));

  parent.enter(|| {
    let runs = runs.clone();

    Effect::render(move |_: Option<()>| {
      runs.set(runs.get() + 1);
      trigger.get();

      let scope = current
        .get()
        .map_or_else(|| parent.child(), |previous| previous.replacement());

      scope.enter(|| Signal::new(0).get());

      if let Some(previous) = current.replace(Some(scope)) {
        previous.dispose();
      }
    });
  });

  for value in 1..=3 {
    trigger.set(value);
  }

  assert_eq!(runs.get(), 4);
}

/// An owner disposes its children newest first, so a child's cleanup runs before
/// those of the children created before it.
#[test]
fn children_are_disposed_newest_first() {
  let parent = Owner::new();
  let order = Rc::new(std::cell::RefCell::new(Vec::new()));

  for index in 0..3 {
    let order = Rc::clone(&order);

    parent
      .child()
      .on_cleanup(move || order.borrow_mut().push(index));
  }

  parent.dispose();

  assert_eq!(*order.borrow(), [2, 1, 0]);
}

/// A subtree moved under another owner takes the depths of its new place, so its render
/// effects run in the order of where they now sit.
#[test]
fn a_moved_subtree_takes_its_new_depths() {
  let root = Owner::new();
  let deep = root.child().child().child();
  let subtree = deep.child();
  let (source, setter) = root.enter(|| signal(0));
  let order = Rc::new(std::cell::RefCell::new(Vec::new()));

  subtree.enter(|| {
    let order = Rc::clone(&order);

    Effect::render(move |_: Option<()>| {
      source.track();
      order.borrow_mut().push("moved");
    });
  });
  root.child().enter(|| {
    let order = Rc::clone(&order);

    Effect::render(move |_: Option<()>| {
      source.track();
      order.borrow_mut().push("shallow");
    });
  });
  order.borrow_mut().clear();
  setter.set(1);

  assert_eq!(
    *order.borrow(),
    ["shallow", "moved"],
    "the deeper runs later"
  );

  subtree.set_parent(root);
  order.borrow_mut().clear();
  setter.set(2);

  assert_eq!(
    *order.borrow(),
    ["moved", "shallow"],
    "at one depth, the earlier created runs first"
  );

  root.dispose();
}

/// A reset inside a batch leaves the batch to end where it would have, and the fresh
/// runtime flushes as ever.
#[test]
fn a_reset_inside_a_batch_leaves_the_runtime_working() {
  skene_reactive::batch(Runtime::reset);

  let (source, setter) = signal(0);
  let runs = Rc::new(Cell::new(0));

  Effect::new({
    let runs = Rc::clone(&runs);

    move |_| {
      source.track();
      runs.set(runs.get() + 1);
    }
  });
  setter.set(1);

  assert_eq!(runs.get(), 2);
}
