//! Eager staleness marking and non-reentrant flushing.

use std::{cell::RefCell, rc::Rc};

use skene_reactive::{Effect, Get, Memo, Set, Signal, batch};

/// A memo read inside a batch, after a write to its source, pulls the fresh value.
#[test]
fn memo_read_inside_batch_is_fresh() {
  let source = Signal::new(1);
  let memo = Memo::new(move |_| source.get() * 10);

  assert_eq!(memo.get(), 10);

  let seen = batch(|| {
    source.set(2);

    memo.get()
  });

  assert_eq!(seen, 20);
}

/// Staleness propagates through a memo chain: a read at the end pulls the whole chain.
#[test]
fn memo_chain_read_inside_batch_is_fresh() {
  let source = Signal::new(1);
  let doubled = Memo::new(move |_| source.get() * 2);
  let quadrupled = Memo::new(move |_| doubled.get() * 2);

  assert_eq!(quadrupled.get(), 4);

  let seen = batch(|| {
    source.set(3);

    quadrupled.get()
  });

  assert_eq!(seen, 12);
}

/// A memo whose recomputed value is unchanged notifies no dependent effect.
#[test]
fn unchanged_memo_filters_change() {
  let source = Signal::new(2);
  let is_even = Memo::new(move |_| source.get() % 2 == 0);
  let runs = Rc::new(RefCell::new(0));

  {
    let runs = Rc::clone(&runs);

    Effect::new(move |_| {
      is_even.get();

      *runs.borrow_mut() += 1;
    });
  }

  assert_eq!(*runs.borrow(), 1);

  source.set(4);
  assert_eq!(*runs.borrow(), 1);

  source.set(5);
  assert_eq!(*runs.borrow(), 2);
}

/// A write inside a running effect enqueues its dependents; they run after the writer
/// finishes, not re-entrantly inside it.
#[test]
fn write_during_flush_defers() {
  let first = Signal::new(0);
  let second = Signal::new(0);
  let log = Rc::new(RefCell::new(Vec::new()));

  {
    let log = Rc::clone(&log);

    Effect::new(move |_| {
      second.get();

      log.borrow_mut().push("second");
    });
  }

  {
    let log = Rc::clone(&log);

    Effect::new(move |_| {
      first.get();

      log.borrow_mut().push("first-start");
      second.set(second.get_untracked() + 1);
      log.borrow_mut().push("first-end");
    });
  }

  log.borrow_mut().clear();
  first.set(1);

  assert_eq!(*log.borrow(), ["first-start", "first-end", "second"]);
}

/// An effect reading both a signal and a memo of it runs once per write, with the two
/// values consistent.
#[test]
fn diamond_runs_once_consistently() {
  let source = Signal::new(1);
  let doubled = Memo::new(move |_| source.get() * 2);
  let runs = Rc::new(RefCell::new(0));

  {
    let runs = Rc::clone(&runs);

    Effect::new(move |_| {
      assert_eq!(doubled.get(), source.get() * 2);

      *runs.borrow_mut() += 1;
    });
  }

  assert_eq!(*runs.borrow(), 1);

  source.set(3);
  assert_eq!(*runs.borrow(), 2);
}
