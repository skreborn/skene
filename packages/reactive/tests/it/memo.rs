//! Memos: computed once, recomputed only when read after what they read changes, and
//! notifying only on a changed value.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_reactive::{Effect, Get, Memo, Owner, Set, Signal, Track, Update};

#[test]
fn unobserved_memo_recomputes_only_when_read() {
  let source = Signal::new(0);
  let calls = Rc::new(Cell::new(0));

  let memo = Memo::new({
    let calls = calls.clone();
    move |_| {
      calls.set(calls.get() + 1);
      source.get() * 2
    }
  });

  assert_eq!(calls.get(), 0, "nothing computes before the first read");

  // Changing the source leaves an unobserved memo alone.
  source.set(1);
  assert_eq!(calls.get(), 0, "no reader, no compute");
  assert_eq!(memo.get(), 2, "a read pulls the fresh value");
  assert_eq!(calls.get(), 1, "the read computed once");
  assert_eq!(memo.get(), 2);
  assert_eq!(calls.get(), 1, "get does not recompute when fresh");

  source.set(2);
  assert_eq!(calls.get(), 1, "still no recompute without a read");
  assert_eq!(memo.get(), 4);
  assert_eq!(calls.get(), 2, "recomputed once more for the read");
}

#[test]
fn memo_notifies_dependents_on_change_when_recomputed() {
  let source = Signal::new(0);
  let memo = Memo::new(move |_| source.get());
  let runs = Rc::new(Cell::new(0));

  Effect::new({
    let runs = runs.clone();
    move |_| {
      memo.track();
      memo.get();
      runs.set(runs.get() + 1);
    }
  });

  assert_eq!(runs.get(), 1, "effect runs once initially");

  source.set(1);
  assert_eq!(runs.get(), 2, "effect reruns after memo recompute");

  source.set(2);
  assert_eq!(runs.get(), 3, "effect reruns on each source change");
}

#[test]
fn memo_skips_notifications_when_value_unchanged() {
  let source = Signal::new(0);
  let memo = Memo::new(move |_| source.get());
  let runs = Rc::new(Cell::new(0));

  Effect::new({
    let runs = runs.clone();
    move |_| {
      memo.track();
      memo.get();
      runs.set(runs.get() + 1);
    }
  });

  assert_eq!(runs.get(), 1);

  source.set(0);
  assert_eq!(runs.get(), 1, "unchanged value does not notify dependents");
}

#[test]
fn memo_skips_when_derived_value_equal_after_signal_update() {
  let source = Signal::new(0);
  let memo = Memo::new(move |_| source.get() % 2 == 0);
  let runs = Rc::new(Cell::new(0));

  Effect::new({
    let runs = runs.clone();
    move |_| {
      memo.track();
      memo.get();
      runs.set(runs.get() + 1);
    }
  });

  assert_eq!(runs.get(), 1);

  source.set(2); // signal changed, derived parity is still even
  assert_eq!(
    runs.get(),
    1,
    "memo should not notify when derived value stays equal"
  );
}

#[test]
fn memo_notifies_when_derived_value_changes_after_signal_update() {
  let source = Signal::new(0);
  let memo = Memo::new(move |_| source.get() % 2 == 0);
  let runs = Rc::new(Cell::new(0));

  Effect::new({
    let runs = runs.clone();
    move |_| {
      memo.track();
      memo.get();
      runs.set(runs.get() + 1);
    }
  });

  assert_eq!(runs.get(), 1);

  source.set(1); // parity flips to odd
  assert_eq!(
    runs.get(),
    2,
    "memo should notify dependents when derived value changes"
  );
}

#[test]
fn memo_recomputes_before_dependents_use_value() {
  let source = Signal::new(0);
  let memo = Memo::new(move |_| source.get());
  let seen = Rc::new(RefCell::new(Vec::new()));

  Effect::new({
    let seen = seen.clone();
    move |_| {
      let value = source.get();
      let memo_value = memo.get();
      seen.borrow_mut().push((value, memo_value));
    }
  });

  assert_eq!(*seen.borrow(), vec![(0, 0)]);

  source.set(1);
  assert_eq!(seen.borrow().last(), Some(&(1, 1)));

  source.set(2);
  assert_eq!(seen.borrow().last(), Some(&(2, 2)));
  assert_eq!(seen.borrow().len(), 3);
}

#[test]
fn memo_stays_in_lockstep_with_signal_for_add_assign_updates() {
  let counter = Signal::new(0);
  let memo_runs = Rc::new(RefCell::new(Vec::new()));

  let memo = Memo::new({
    let memo_runs = memo_runs.clone();
    move |_| {
      let value = counter.get();
      memo_runs.borrow_mut().push(value);
      value
    }
  });

  let view_runs = Rc::new(RefCell::new(Vec::new()));
  Effect::new({
    let view_runs = view_runs.clone();
    move |_| {
      let value = counter.get();
      let memo_value = memo.get();
      view_runs.borrow_mut().push((value, memo_value));
    }
  });

  assert_eq!(*memo_runs.borrow(), vec![0]);
  assert_eq!(*view_runs.borrow(), vec![(0, 0)]);

  counter.update(|value| *value += 1);
  counter.update(|value| *value += 1);
  counter.update(|value| *value += 1);

  assert_eq!(*memo_runs.borrow(), vec![0, 1, 2, 3]);
  assert_eq!(*view_runs.borrow(), vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
}

#[test]
fn memo_high_priority_runs_before_normal_dependents() {
  let source = Signal::new(0);
  let log = Rc::new(RefCell::new(Vec::new()));

  let memo = Memo::new({
    let log = log.clone();
    move |_| {
      log.borrow_mut().push("memo");
      source.get()
    }
  });

  log.borrow_mut().clear();

  Effect::new({
    let log = log.clone();
    move |_| {
      let _ = source.get();
      let _ = memo.get();
      log.borrow_mut().push("effect");
    }
  });

  log.borrow_mut().clear();

  source.set(1);

  assert_eq!(*log.borrow(), vec!["memo", "effect"]);
}

/// A summary over rows keeps updating when a row's scope is disposed in the same flush
/// that removes the row, as a keyed list does.
#[test]
fn a_summary_outlives_the_rows_it_read() {
  use skene_reactive::Owner;

  let app = Owner::new();
  let rows = app.child();
  let row = rows.child();
  let item = row.enter(|| Signal::new(3));
  let items = app.enter(|| Signal::transient(vec![(1u32, item)]));
  let owners = Rc::new(RefCell::new(vec![(1u32, row)]));
  let total =
    app.enter(|| Memo::new(move |_| items.get().iter().map(|(_, item)| item.get()).sum::<i32>()));
  let shown = Rc::new(Cell::new(0));

  rows.enter(|| {
    let owners = owners.clone();

    Effect::render(move |_: Option<()>| {
      let keys: Vec<u32> = items.get().iter().map(|(key, _)| *key).collect();

      owners.borrow_mut().retain(|(key, owner)| {
        let keep = keys.contains(key);

        if !keep {
          owner.dispose();
        }

        keep
      });
    });
  });

  app.enter(|| {
    let shown = shown.clone();

    Effect::new(move |_: Option<()>| shown.set(total.get()));
  });

  assert_eq!(shown.get(), 3);

  items.set(vec![]);

  let fresh = app.enter(|| Signal::new(10));

  items.set(vec![(2, fresh)]);

  assert_eq!(total.try_get_untracked(), Some(10));
  assert_eq!(shown.get(), 10);
}

/// A change at the source of a long chain of memos reaches its end without a call per
/// memo on the stack.
#[test]
fn a_long_memo_chain_settles() {
  let owner = Owner::new();

  owner.enter(|| {
    let source = Signal::new(0u64);
    let mut last = Memo::new(move |_| source.get());

    for _ in 0..100_000 {
      let previous = last;

      last = Memo::new(move |_| previous.get() + 1);
      let _ = last.get_untracked();
    }

    assert_eq!(last.get(), 100_000);

    source.set(1);

    assert_eq!(last.get(), 100_001);
  });

  owner.dispose();
}
