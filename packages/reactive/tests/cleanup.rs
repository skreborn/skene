//! Owner cleanup: the hook anything the runtime cannot free itself hangs on—a spawned
//! task, a timer, a subscription.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_reactive::{Effect, Get, Memo, Owner, Set, Signal, on_cleanup};

/// Records what ran, in order.
fn log() -> (
  Rc<RefCell<Vec<&'static str>>>,
  impl Fn(&'static str) + Clone,
) {
  let entries = Rc::new(RefCell::new(Vec::new()));
  let record = {
    let entries = Rc::clone(&entries);

    move |what: &'static str| entries.borrow_mut().push(what)
  };

  (entries, record)
}

#[test]
fn a_cleanup_runs_when_its_scope_is_disposed() {
  let (entries, record) = log();
  let scope = Owner::new();

  scope.on_cleanup({
    let record = record.clone();

    move || record("cleaned")
  });

  assert!(entries.borrow().is_empty(), "not until disposal");

  scope.dispose();

  assert_eq!(&*entries.borrow(), &["cleaned"]);
}

/// The case tasks depend on: a task spawned deep in a view tree must die when an ancestor
/// goes away, not just when its own scope is disposed directly.
#[test]
fn disposing_a_parent_runs_a_childs_cleanup() {
  let (entries, record) = log();
  let parent = Owner::new();
  let child = parent.child();
  let grandchild = child.child();

  grandchild.on_cleanup({
    let record = record.clone();

    move || record("grandchild")
  });

  child.on_cleanup({
    let record = record.clone();

    move || record("child")
  });

  parent.dispose();

  assert_eq!(
    &*entries.borrow(),
    &["grandchild", "child"],
    "descendants clean up before their ancestors",
  );
}

#[test]
fn cleanups_run_in_reverse_registration_order() {
  let (entries, record) = log();
  let scope = Owner::new();

  for what in ["first", "second", "third"] {
    scope.on_cleanup({
      let record = record.clone();

      move || record(what)
    });
  }

  scope.dispose();

  assert_eq!(&*entries.borrow(), &["third", "second", "first"]);
}

#[test]
fn a_cleanup_runs_at_most_once() {
  let (entries, record) = log();
  let scope = Owner::new();

  scope.on_cleanup(move || record("cleaned"));

  scope.dispose();
  scope.dispose();

  assert_eq!(
    &*entries.borrow(),
    &["cleaned"],
    "a second dispose is a no-op"
  );
}

#[test]
fn an_effect_runs_its_cleanups_before_each_rerun() {
  let source = Signal::new(0);
  let cleaned = Rc::new(Cell::new(0));
  let owner = Owner::new();

  owner.enter(|| {
    let cleaned = cleaned.clone();

    Effect::new(move |_: Option<()>| {
      source.get();

      let cleaned = cleaned.clone();

      on_cleanup(move || cleaned.set(cleaned.get() + 1));
    });
  });

  for value in 1..=5 {
    source.set(value);
  }

  assert_eq!(cleaned.get(), 5, "one per re-run");

  owner.dispose();

  assert_eq!(cleaned.get(), 6, "and the last run's at disposal");
}

#[test]
fn a_cleanup_on_a_disposed_owner_runs_at_once() {
  let (entries, record) = log();
  let owner = Owner::new();

  owner.dispose();
  owner.on_cleanup(move || record("cleaned"));

  assert_eq!(&*entries.borrow(), &["cleaned"]);
}

/// A memo pulled during a reader's run tears down its last run without the reader
/// subscribing to what the teardown reads.
#[test]
fn a_teardown_during_a_pull_subscribes_no_reader() {
  let source = Signal::new(0);
  let read_in_cleanup = Signal::new(0);
  let runs = Rc::new(Cell::new(0));
  let memo = Memo::new(move |_| {
    let value = source.get();

    Owner::current().child().on_cleanup(move || {
      read_in_cleanup.get();
    });

    value
  });

  {
    let runs = runs.clone();

    Effect::new(move |_: Option<()>| {
      source.get();
      memo.get();
      runs.set(runs.get() + 1);
    });
  }

  source.set(1);

  assert_eq!(runs.get(), 2);

  read_in_cleanup.set(1);

  assert_eq!(runs.get(), 2);
}
