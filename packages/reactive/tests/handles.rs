//! The satellite handles: watchers, reactions, slices, owned signals, triggers.

use std::{cell::Cell, rc::Rc};

use skene_reactive::{
  Effect,
  Get,
  OwnedSignal,
  Owner,
  Reaction,
  Set,
  Signal,
  Slice,
  Trigger,
  Update,
  untrack,
};

#[test]
fn watch_computes_now_and_reports_changes() {
  let source = Signal::new(1);
  let seen = Rc::new(Cell::new(0));
  let initial = Effect::watch(move || source.get() * 10, {
    let seen = seen.clone();

    move |value| seen.set(value)
  });

  assert_eq!(initial, 10, "the initial compute returns its value");
  assert_eq!(
    seen.get(),
    0,
    "on_change does not run for the initial compute"
  );

  source.set(2);

  assert_eq!(
    seen.get(),
    20,
    "a dependency change reports the fresh value"
  );
}

#[test]
fn untrack_reads_without_subscribing() {
  let tracked = Signal::new(0);
  let untracked = Signal::new(0);
  let runs = Rc::new(Cell::new(0));

  {
    let runs = runs.clone();

    Effect::new(move |_| {
      tracked.get();
      untrack(|| untracked.get());
      runs.set(runs.get() + 1);
    });
  }

  assert_eq!(runs.get(), 1);

  untracked.set(1);
  assert_eq!(runs.get(), 1, "an untracked read does not subscribe");

  tracked.set(1);
  assert_eq!(runs.get(), 2);
}

#[test]
fn reaction_invalidates_once_per_track() {
  let source = Signal::new(0);
  let invalidations = Rc::new(Cell::new(0));
  let reaction = Reaction::new({
    let invalidations = invalidations.clone();

    move || invalidations.set(invalidations.get() + 1)
  });
  let value = reaction.track(|| source.get());

  assert_eq!(value, 0);
  assert_eq!(invalidations.get(), 0);

  source.set(1);
  assert_eq!(invalidations.get(), 1, "a tracked change invalidates");

  let value = reaction.track(|| source.get());

  assert_eq!(value, 1);

  source.set(2);
  assert_eq!(invalidations.get(), 2, "re-tracking re-subscribes");

  drop(reaction);
  source.set(3);
  assert_eq!(invalidations.get(), 2, "a dropped reaction is silent");
}

#[test]
fn slice_projects_reads_and_writes() {
  let pair = Signal::new((1, 2));
  let first = Slice::new(
    pair,
    |pair: &(i32, i32)| pair.0,
    |pair: &mut (i32, i32), value: &i32| pair.0 = *value,
  );

  assert_eq!(first.get_untracked(), 1);

  first.set(10);
  assert_eq!(pair.get_untracked(), (10, 2));

  first.update(|value| *value += 1);
  assert_eq!(first.get_untracked(), 11);

  let runs = Rc::new(Cell::new(0));

  {
    let runs = runs.clone();

    Effect::new(move |_| {
      first.get();
      runs.set(runs.get() + 1);
    });
  }

  assert_eq!(runs.get(), 1);

  first.set(12);
  assert_eq!(runs.get(), 2, "a slice write notifies the signal's readers");
}

#[test]
fn owned_signal_dies_with_its_handle() {
  let owner = Owner::new();
  let handle = owner.enter(|| OwnedSignal::new(5));
  let signal = handle.signal();

  // An owned signal ignores the owner tree entirely.
  owner.dispose();
  assert_eq!(signal.try_get(), Some(5));

  drop(handle);
  assert_eq!(signal.try_get(), None);
}

#[test]
fn trigger_wakes_trackers() {
  let trigger = Trigger::new();
  let runs = Rc::new(Cell::new(0));

  {
    let runs = runs.clone();

    Effect::new(move |_| {
      trigger.track();
      runs.set(runs.get() + 1);
    });
  }

  assert_eq!(runs.get(), 1);

  trigger.notify();
  assert_eq!(runs.get(), 2);
}
