//! Ownership: disposal reaches everything an owner created, and stale handles stay
//! dead.

use std::{cell::Cell, rc::Rc};

use skene_reactive::{Effect, Get, Owner, Runtime, Set, Signal, Track, signal};

#[test]
fn effect_disposed_when_dependency_signal_disposed() {
  let parent = Owner::new();
  let signal_owner = parent.child();
  let (source, setter) = signal_owner.enter(|| signal(0));
  let count = Rc::new(Cell::new(0));

  parent.enter(|| {
    let count = count.clone();

    Effect::new(move |_| {
      source.track();
      count.set(count.get() + 1);
    });
  });

  assert_eq!(count.get(), 1);

  // Disposing the signal's owner cleans up the subscribing effect.
  signal_owner.dispose();

  // Mutations after disposal must not rerun the effect.
  setter.set(1);
  Runtime::drain_pending_work();
  assert_eq!(count.get(), 1);
}

#[test]
fn signals_created_by_effect_are_disposed_with_effect() {
  let parent = Owner::new();
  let dep_owner = parent.child();
  let (dep, dep_setter) = dep_owner.enter(|| signal(0));
  let created = Rc::new(Cell::new(None::<Signal<i32>>));
  let runs = Rc::new(Cell::new(0));

  parent.enter(|| {
    let created = created.clone();
    let runs = runs.clone();

    Effect::new(move |_| {
      dep.track();
      runs.set(runs.get() + 1);

      if created.get().is_none() {
        created.set(Some(Signal::new(0)));
      }
    });
  });

  assert_eq!(runs.get(), 1);

  let inner = created.get().expect("signal created");

  assert_eq!(inner.try_get(), Some(0));

  // Dispose the dependency owner; the effect goes with it, and its created signal too.
  dep_owner.dispose();
  Runtime::drain_pending_work();

  dep_setter.set(1);
  Runtime::drain_pending_work();
  assert_eq!(runs.get(), 1);
  assert_eq!(inner.try_get(), None);
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
  let fresh: Vec<Signal<u32>> = (0..16).map(|n| Signal::new(n + 100)).collect();

  assert_eq!(source.try_get(), None);
  assert_eq!(source.try_get_untracked(), None);

  for (n, signal) in fresh.iter().enumerate() {
    assert_eq!(signal.try_get(), Some(n as u32 + 100));
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
