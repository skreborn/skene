//! Owner cleanup: the hook anything the runtime cannot free itself hangs on—a spawned
//! task, a timer, a subscription.

use std::{cell::RefCell, rc::Rc};

use skene_reactive::Owner;

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
