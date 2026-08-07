//! Tagged-signal snapshots: serialize on one run, stage and adopt on the next.
//!
//! Each test thread carries its own runtime, so a spawned thread stands in for the
//! next run of the application.

use skene_reactive::{Get, Owner, Set, Signal, restore, snapshot};

/// A snapshot restores tagged state across runs: staged values are adopted, tags with
/// no staged value fall back to their defaults, and untagged signals stay out of it.
#[test]
fn snapshot_round_trips_tagged_state() {
  let bytes = std::thread::spawn(|| {
    let count = Signal::tagged("count", 1u32);
    let name = Signal::tagged("name", String::from("fresh"));
    let _untagged = Signal::new(9u8);

    count.set(41);
    name.set("skene".to_owned());

    snapshot()
  })
  .join()
  .expect("the first run");

  let (count, name, missing) = std::thread::spawn(move || {
    restore(&bytes);

    (
      Signal::tagged("count", 1u32).get(),
      Signal::tagged("name", String::from("fresh")).get(),
      Signal::tagged("missing", 7u8).get(),
    )
  })
  .join()
  .expect("the second run");

  assert_eq!(count, 41);
  assert_eq!(name, "skene");
  assert_eq!(missing, 7);
}

/// A staged value whose type changed fails to decode and degrades to the default.
#[test]
fn changed_type_degrades_to_the_default() {
  let bytes = std::thread::spawn(|| {
    let value = Signal::tagged("value", 0u32);

    value.set(300);

    snapshot()
  })
  .join()
  .expect("the first run");

  let value = std::thread::spawn(move || {
    restore(&bytes);

    Signal::tagged("value", String::from("fallback")).get()
  })
  .join()
  .expect("the second run");

  assert_eq!(value, "fallback");
}

/// A staged value applies exactly once: a recreation of the same tag starts from its
/// default, and a disposed tag may be re-registered.
#[test]
fn staged_value_applies_exactly_once() {
  let bytes = std::thread::spawn(|| {
    Signal::tagged("once", 0u32).set(5);

    snapshot()
  })
  .join()
  .expect("the first run");

  let (first, second) = std::thread::spawn(move || {
    restore(&bytes);

    let scope = Owner::new();
    let first = scope.enter(|| Signal::tagged("once", 1u32).get());

    scope.dispose();

    (first, Signal::tagged("once", 1u32).get())
  })
  .join()
  .expect("the second run");

  assert_eq!(first, 5);
  assert_eq!(second, 1);
}

/// Garbage bytes stage nothing and every tag falls back to its default.
#[test]
fn corrupt_snapshot_stages_nothing() {
  let value = std::thread::spawn(|| {
    restore(b"not a snapshot");

    Signal::tagged("value", 3u16).get()
  })
  .join()
  .expect("the run");

  assert_eq!(value, 3);
}

/// Two live signals cannot share a tag.
#[test]
#[should_panic(expected = "duplicate snapshot tag: dup")]
fn duplicate_live_tag_panics() {
  let _first = Signal::tagged("dup", 1u32);
  let _second = Signal::tagged("dup", 2u32);
}
