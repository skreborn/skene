//! The engine's scheduling contract: lazy memos, panic safety, effect handles, render
//! ordering, selectors, and the scheduler seam.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_reactive::{Effect, Get, Memo, Owner, Runtime, Selector, Set, Signal, Track, batch};

/// The flushes a test scheduler was handed.
type Flushes = Rc<RefCell<Vec<Box<dyn FnOnce()>>>>;

/// A memo read inside a batch computes once; the flush that follows reuses it.
#[test]
fn memo_read_inside_batch_computes_once() {
  let source = Signal::new(1);
  let computes = Rc::new(Cell::new(0));
  let memo = Memo::new({
    let computes = computes.clone();

    move |_| {
      computes.set(computes.get() + 1);

      source.get() * 10
    }
  });
  let seen = Rc::new(Cell::new(0));

  {
    let seen = seen.clone();

    Effect::new(move |_| seen.set(memo.get()));
  }

  assert_eq!(computes.get(), 1);

  batch(|| {
    source.set(2);

    assert_eq!(memo.get(), 20, "the read pulls the fresh value");
  });

  assert_eq!(seen.get(), 20, "the effect saw the change");
  assert_eq!(computes.get(), 2, "read plus flush computed exactly once");
}

/// An unobserved memo does not recompute on a source write.
#[test]
fn unobserved_memo_ignores_writes() {
  let source = Signal::new(1);
  let computes = Rc::new(Cell::new(0));
  let memo = Memo::new({
    let computes = computes.clone();

    move |_| {
      computes.set(computes.get() + 1);

      source.get()
    }
  });

  assert_eq!(computes.get(), 1);

  for value in 2..10 {
    source.set(value);
  }

  assert_eq!(computes.get(), 1, "no reader, no recompute");
  assert_eq!(memo.get_untracked(), 9);
  assert_eq!(computes.get(), 2, "one recompute for the read");
}

/// A comparator other than equality decides what counts as a change.
#[test]
fn memo_new_with_uses_its_comparator() {
  let source = Signal::new(1.0f64);
  let rounded = Memo::new_with(
    |a: &f64, b: &f64| (a - b).abs() < 0.5,
    move |_| source.get(),
  );
  let runs = Rc::new(Cell::new(0));

  {
    let runs = runs.clone();

    Effect::new(move |_| {
      rounded.get();
      runs.set(runs.get() + 1);
    });
  }

  source.set(1.2);
  assert_eq!(runs.get(), 1, "within tolerance: nobody notified");

  source.set(2.0);
  assert_eq!(runs.get(), 2, "beyond tolerance: notified");
}

/// A panic inside `batch` leaves the runtime flushing on the next write.
#[test]
fn panic_inside_batch_does_not_poison_flushing() {
  let source = Signal::new(0);
  let seen = Rc::new(Cell::new(0));

  {
    let seen = seen.clone();

    Effect::new(move |_| seen.set(source.get()));
  }

  let result = std::panic::catch_unwind(|| {
    batch(|| {
      source.set(1);
      panic!("inside the batch");
    });
  });

  assert!(result.is_err());

  source.set(2);

  assert_eq!(
    seen.get(),
    2,
    "a write after the panic flushes synchronously"
  );
}

/// A panic inside `Owner::enter` restores the previous owner.
#[test]
fn panic_inside_enter_restores_the_owner() {
  let before = Owner::current();
  let scope = Owner::new();
  let result = std::panic::catch_unwind(|| {
    scope.enter(|| panic!("inside the scope"));
  });

  assert!(result.is_err());
  assert_eq!(Owner::current(), before);
}

/// A handle disposes its effect, and pauses and resumes it.
#[test]
fn effect_handle_disposes_pauses_and_resumes() {
  let source = Signal::new(0);
  let runs = Rc::new(Cell::new(0));
  let handle = {
    let runs = runs.clone();

    Effect::new(move |_| {
      source.track();
      runs.set(runs.get() + 1);
    })
  };

  assert_eq!(runs.get(), 1);

  handle.pause();
  source.set(1);
  source.set(2);
  assert_eq!(runs.get(), 1, "a paused effect does not run");

  handle.resume();
  assert_eq!(runs.get(), 2, "resuming runs once for the missed writes");

  handle.resume();
  assert_eq!(runs.get(), 2, "resuming a fresh effect is a no-op");

  handle.dispose();
  source.set(3);
  assert_eq!(runs.get(), 2, "a disposed effect never runs again");
}

/// Pausing an owner pauses every effect beneath it, nested owners and effects created
/// by an effect's run included, and resuming runs each that missed a write once; an
/// effect outside the owner runs throughout.
#[test]
fn an_owner_pauses_and_resumes_every_effect_beneath_it() {
  let source = Signal::new(0);
  let runs = Rc::new(RefCell::new(Vec::new()));
  let root = Owner::new();
  let scope = root.child();
  let nested = scope.child();
  let note = |label: &'static str| {
    let runs = runs.clone();

    move |_: Option<()>| {
      source.track();
      runs.borrow_mut().push(label);
    }
  };

  scope.enter(|| {
    let inner = note("inner");

    Effect::new(move |_: Option<()>| {
      Effect::new(inner.clone());
    });
  });
  scope.enter(|| Effect::new(note("scope")));
  nested.enter(|| Effect::render(note("nested")));
  root.enter(|| Effect::new(note("outside")));
  runs.borrow_mut().clear();

  scope.pause();
  source.set(1);
  source.set(2);

  assert_eq!(
    *runs.borrow(),
    ["outside", "outside"],
    "only the effect outside runs"
  );

  runs.borrow_mut().clear();
  scope.resume();

  let mut resumed = runs.borrow().clone();

  resumed.sort_unstable();

  assert_eq!(
    resumed,
    ["inner", "nested", "scope"],
    "each paused effect runs once"
  );

  runs.borrow_mut().clear();
  scope.resume();

  assert!(
    runs.borrow().is_empty(),
    "resuming a fresh owner runs nothing"
  );
}

/// Render effects run before ordinary effects in one flush, in owner-depth order.
#[test]
fn render_effects_run_first_by_depth() {
  let source = Signal::new(0);
  let log = Rc::new(RefCell::new(Vec::new()));
  let root = Owner::new();
  let child = root.child();
  let push = |label: &'static str| {
    let log = log.clone();

    move |_: Option<()>| {
      source.track();
      log.borrow_mut().push(label);
    }
  };

  root.enter(|| Effect::new(push("ordinary")));
  child.enter(|| Effect::render(push("render-deep")));
  root.enter(|| Effect::render(push("render-shallow")));

  log.borrow_mut().clear();
  source.set(1);

  assert_eq!(*log.borrow(), ["render-shallow", "render-deep", "ordinary"]);
}

/// A selector change re-runs only the readers of the key left and the key entered.
#[test]
fn selector_reruns_only_the_affected_readers() {
  let selected = Signal::new(1usize);
  let selector = Selector::new(move || selected.get());
  let runs: Vec<Rc<Cell<u32>>> = (0..5).map(|_| Rc::new(Cell::new(0))).collect();

  for (key, runs) in runs.iter().enumerate() {
    let runs = runs.clone();

    Effect::new(move |_| {
      let _ = selector.is(&key);
      runs.set(runs.get() + 1);
    });
  }

  let counts = || runs.iter().map(|runs| runs.get()).collect::<Vec<_>>();

  assert_eq!(counts(), [1, 1, 1, 1, 1]);
  assert!(selector.is(&1));

  selected.set(3);

  assert_eq!(counts(), [1, 2, 1, 2, 1], "only keys 1 and 3 re-ran");
  assert!(selector.is(&3));
  assert!(!selector.is(&1));

  selected.set(3);

  assert_eq!(counts(), [1, 2, 1, 2, 1], "an unchanged key re-runs nobody");
}

/// A selector answers with the source's current key even inside a batch, before any
/// effect of the batch has run.
#[test]
fn a_selector_answers_the_current_key_inside_a_batch() {
  let selected = Signal::new(1usize);
  let selector = Selector::new(move || selected.get());

  batch(|| {
    selected.set(2);

    assert!(selector.is(&2));
    assert!(!selector.is(&1));
  });
}

/// A scheduler receives one flush for any number of writes; `flush_sync` drains now.
#[test]
fn scheduler_coalesces_writes_into_one_flush() {
  let scheduled: Flushes = Rc::new(RefCell::new(Vec::new()));

  Runtime::set_scheduler({
    let scheduled = scheduled.clone();

    move |flush| scheduled.borrow_mut().push(flush)
  });

  let source = Signal::new(0);
  let seen = Rc::new(Cell::new(0));

  {
    let seen = seen.clone();

    Effect::new(move |_| seen.set(source.get()));
  }

  for value in 1..=5 {
    source.set(value);
  }

  assert_eq!(seen.get(), 0, "nothing ran inline");
  assert_eq!(
    scheduled.borrow().len(),
    1,
    "five writes, one scheduled flush"
  );

  let flush = scheduled.borrow_mut().pop().expect("the flush");

  flush();

  assert_eq!(seen.get(), 5);

  source.set(6);

  assert_eq!(
    scheduled.borrow().len(),
    1,
    "the next write schedules again"
  );

  Runtime::flush_sync();

  assert_eq!(seen.get(), 6, "flush_sync drains without the scheduler");

  scheduled.borrow_mut().pop().expect("the stale flush")();

  assert_eq!(seen.get(), 6, "the stale flush finds nothing to do");
}
