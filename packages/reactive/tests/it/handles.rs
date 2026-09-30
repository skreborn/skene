//! The satellite handles: untracked reads and triggers.

use std::{cell::Cell, rc::Rc};

use skene_reactive::{Effect, Get, Set, Signal, Trigger, untrack};

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
