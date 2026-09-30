//! State snapshots: serialize on one run, stage and adopt on the next, by position.
//!
//! Each test thread carries its own runtime, so a spawned thread stands in for the
//! next run of the application.

use skene_reactive::{Effect, Get, Key, Owner, Set, Signal, restore, snapshot};

/// Runs `first`, then `second` on a fresh runtime with `first`'s snapshot staged.
fn across_runs<R: Send + 'static>(
  first: impl FnOnce() + Send + 'static,
  second: impl FnOnce() -> R + Send + 'static,
) -> R {
  let bytes = std::thread::spawn(move || {
    first();

    snapshot()
  })
  .join()
  .expect("the first run");

  std::thread::spawn(move || {
    restore(&bytes);

    second()
  })
  .join()
  .expect("the second run")
}

/// Untagged signals restore by position: the same program adopts every value, and
/// a transient signal neither serializes nor occupies a position.
#[test]
fn positional_state_round_trips() {
  let (count, name, flag) = across_runs(
    || {
      let count = Signal::new(1u32);
      let _transient = Signal::transient(vec![Box::new(9u8)]);
      let name = Signal::new(String::from("fresh"));
      let flag = Signal::new(false);

      count.set(41);
      name.set("skene".to_owned());
      flag.set(true);
    },
    || {
      let count = Signal::new(1u32);
      let name = Signal::new(String::from("fresh"));
      let flag = Signal::new(false);

      (count.get(), name.get(), flag.get())
    },
  );

  assert_eq!((count, name, flag), (41, "skene".to_owned(), true));
}

/// A program edit that adds a signal before existing ones: tagged signals restore by
/// tag, the new one takes its default, and the moved ones match by shape.
#[test]
fn an_added_signal_shifts_nothing_it_can_be_told_apart_from() {
  let (added, tagged, moved, trailing) = across_runs(
    || {
      let tagged = Signal::tagged("chosen", 0u32);
      let moved = Signal::new(String::new());
      let trailing = Signal::new(0u64);

      tagged.set(7);
      moved.set("moved".to_owned());
      trailing.set(99);
    },
    || {
      let added = Signal::new(false);
      let tagged = Signal::tagged("chosen", 0u32);
      let moved = Signal::new(String::new());
      let trailing = Signal::new(0u64);

      (added.get(), tagged.get(), moved.get(), trailing.get())
    },
  );

  assert!(!added, "the added signal starts from its default");
  assert_eq!(tagged, 7, "the tagged signal restores by tag");
  assert_eq!(moved, "moved", "the moved signal matches by shape");
  assert_eq!(trailing, 99);
}

/// Content rebuilt in place—a `derived` branch flipping away and back—keeps its
/// identity: the rebuilt scope takes the previous one's place.
#[test]
fn a_branch_flip_keeps_identity() {
  fn build(scope: Owner, dark: bool) -> Signal<u32> {
    scope.enter(|| {
      if dark {
        let _other = Signal::new(String::from("dark"));
      }

      Signal::new(0u32)
    })
  }

  let restored = across_runs(
    || {
      let parent = Owner::new();
      let mut scope = parent.child();
      let mut count = build(scope, false);

      count.set(5);

      for dark in [true, false] {
        let next = scope.replacement();

        scope.dispose();
        scope = next;
        count = build(scope, dark);
      }

      count.set(11);
    },
    || {
      let parent = Owner::new();
      let scope = parent.child();

      build(scope, false).get()
    },
  );

  assert_eq!(restored, 11);
}

/// A signal created inside an effect keeps its position across the effect's re-runs.
#[test]
fn an_effect_rerun_keeps_positions() {
  let restored = across_runs(
    || {
      let trigger = Signal::new(0u8);
      let latest = std::rc::Rc::new(std::cell::Cell::new(None::<Signal<u32>>));

      Effect::new({
        let latest = std::rc::Rc::clone(&latest);

        move |_| {
          let _ = trigger.get();

          latest.set(Some(Signal::new(0u32)));
        }
      });

      trigger.set(1);
      trigger.set(2);
      latest.get().expect("the effect ran").set(23);
    },
    || {
      let _trigger = Signal::new(0u8);
      let latest = std::rc::Rc::new(std::cell::Cell::new(None::<Signal<u32>>));

      Effect::new({
        let latest = std::rc::Rc::clone(&latest);

        move |_| latest.set(Some(Signal::new(0u32)))
      });

      latest.get().expect("the effect ran").get()
    },
  );

  assert_eq!(restored, 23);
}

/// A staged value whose type changed is not matched and the signal takes its default.
#[test]
fn changed_type_degrades_to_the_default() {
  let (positional, tagged) = across_runs(
    || {
      Signal::new(0u32).set(300);
      Signal::tagged("value", 0u32).set(301);
    },
    || {
      (
        Signal::new(String::from("fallback")).get(),
        Signal::tagged("value", String::from("fallback")).get(),
      )
    },
  );

  assert_eq!(positional, "fallback");
  assert_eq!(tagged, "fallback");
}

/// A disposed signal leaves the snapshot, and its position and tag are free again.
#[test]
fn disposal_unregisters() {
  let (first, second) = across_runs(
    || {
      let scope = Owner::new();

      scope.enter(|| {
        Signal::new(0u32).set(5);
        Signal::tagged("once", 0u32).set(6);
      });
      scope.dispose();

      let scope = Owner::new();

      scope.enter(|| {
        Signal::new(0u32).set(7);
        Signal::tagged("once", 0u32).set(8);
      });
    },
    || {
      let scope = Owner::new();
      let scope_two = Owner::new();
      let first = scope.enter(|| (Signal::new(1u32).get(), Signal::tagged("once", 1u32).get()));
      let second =
        scope_two.enter(|| (Signal::new(1u32).get(), Signal::tagged("once", 1u32).get()));

      (first, second)
    },
  );

  assert_eq!(first, (1, 1), "the disposed scope's values are gone");
  assert_eq!(second, (7, 8), "the live scope's values restore");
}

/// A staged value applies exactly once: a recreation at the same position starts from
/// its default.
#[test]
fn staged_value_applies_exactly_once() {
  let (first, second) = across_runs(
    || {
      Signal::tagged("once", 0u32).set(5);
    },
    || {
      let scope = Owner::new();
      let first = scope.enter(|| Signal::tagged("once", 1u32).get());

      scope.dispose();

      (first, Signal::tagged("once", 1u32).get())
    },
  );

  assert_eq!(first, 1, "another owner has no staged value under the tag");
  assert_eq!(second, 5, "the root owner's staged value applies once");
}

/// Garbage bytes stage nothing and every signal falls back to its default.
#[test]
fn corrupt_snapshot_stages_nothing() {
  let value = std::thread::spawn(|| {
    restore(b"not a snapshot");

    Signal::new(3u16).get()
  })
  .join()
  .expect("the run");

  assert_eq!(value, 3);
}

/// Provided extras ride the snapshot beside the signals and are adopted by tag.
#[test]
fn provided_extras_ride_the_snapshot() {
  let staged = across_runs(
    || skene_reactive::provide("extra", || Some(vec![1, 2, 3])),
    || skene_reactive::staged("extra"),
  );

  assert_eq!(staged, Some(vec![1, 2, 3]));
}

/// Two live signals of one owner cannot share a tag; two owners may.
#[test]
fn tags_are_unique_per_owner() {
  let scope = Owner::new();
  let _first = Signal::tagged("dup", 1u32);
  let _second = scope.enter(|| Signal::tagged("dup", 2u32));
  let panicked = std::panic::catch_unwind(|| {
    let _third = Signal::tagged("dup", 3u32);
  });

  assert!(panicked.is_err(), "a duplicate tag in one owner panics");
}

/// A keyed scope restores by its key: state under it matches wherever the scope sits
/// among its siblings, so a reordered list finds each item's own state.
#[test]
fn a_keyed_scope_restores_by_key_across_a_reorder() {
  fn build(parent: Owner, order: &[u8]) -> Vec<Signal<u32>> {
    order
      .iter()
      .map(|item| {
        parent
          .keyed_child(Key::of(item))
          .enter(|| Signal::new(0u32))
      })
      .collect()
  }

  let restored = across_runs(
    || {
      let parent = Owner::new();

      for (signal, value) in build(parent, &[1, 2, 3]).into_iter().zip([10, 20, 30]) {
        signal.set(value);
      }
    },
    || {
      let parent = Owner::new();

      build(parent, &[3, 1, 4])
        .into_iter()
        .map(|signal| signal.get())
        .collect::<Vec<_>>()
    },
  );

  assert_eq!(restored, [30, 10, 0]);
}

/// A keyed child takes no position, so keying a sibling shifts none of the positional
/// ones, and a positional child before the keyed ones still restores by position.
#[test]
fn a_keyed_child_takes_no_position() {
  let (positional, keyed) = across_runs(
    || {
      let parent = Owner::new();

      parent.child().enter(|| Signal::new(0u32)).set(5);
      parent
        .keyed_child(Key::of(&"row"))
        .enter(|| Signal::new(0u32))
        .set(6);
    },
    || {
      let parent = Owner::new();
      let keyed = parent
        .keyed_child(Key::of(&"row"))
        .enter(|| Signal::new(0u32));
      let _added = parent.keyed_child(Key::of(&"added"));
      let positional = parent.child().enter(|| Signal::new(0u32));

      (positional.get(), keyed.get())
    },
  );

  assert_eq!((positional, keyed), (5, 6));
}

/// Rebuilding a scope in place takes no position of its own, so a later sibling keeps
/// its identity however often an earlier one is rebuilt.
#[test]
fn a_replacement_shifts_no_later_sibling() {
  let restored = across_runs(
    || {
      let page = Owner::new();
      let first = page.child();
      let second = first.replacement();

      first.dispose();

      let third = second.replacement();

      second.dispose();

      let _ = third;
      let later = page.child();

      later.enter(|| Signal::new(0u32)).set(42);
    },
    || {
      let page = Owner::new();
      let _first = page.child();
      let later = page.child();

      later.enter(|| Signal::new(0u32)).get()
    },
  );

  assert_eq!(restored, 42);
}
