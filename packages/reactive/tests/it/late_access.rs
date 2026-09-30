//! Late access: a debug build names where a read after disposal happened, where the
//! signal was created and what disposed it.

#![cfg(debug_assertions)]

use std::{
  cell::Cell,
  panic::{AssertUnwindSafe, catch_unwind},
  rc::Rc,
};

use skene_reactive::{Effect, Get, Owner, Set, Signal};

/// The message `read` panics with.
fn panic_message(read: impl FnOnce()) -> String {
  let payload = catch_unwind(AssertUnwindSafe(read)).expect_err("the read panics");

  payload
    .downcast_ref::<String>()
    .cloned()
    .or_else(|| {
      payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
    })
    .expect("a message")
}

/// A read of a signal an owner's disposal took names the read, the signal's creation
/// and the disposing call.
#[test]
fn a_late_read_names_the_read_the_creation_and_the_disposal() {
  let owner = Owner::new();
  let created = line!() + 1;
  let signal = owner.enter(|| Signal::new(1));
  let disposed = line!() + 1;
  owner.dispose();
  let read = line!() + 1;
  let message = panic_message(|| _ = signal.get());
  let file = file!();

  assert!(
    message.contains(&format!("read at {file}:{read}:")),
    "{message}"
  );
  assert!(
    message.contains(&format!("created at {file}:{created}:")),
    "{message}"
  );
  assert!(
    message.contains(&format!("disposed by the call at {file}:{disposed}:")),
    "{message}"
  );
}

/// A signal its owner's re-run disposed says so, beside where it was created; a write
/// to it is dropped.
#[test]
fn a_signal_its_owners_re_run_disposed_says_so() {
  let rerun = Signal::new(0);
  let held = Rc::new(Cell::new(None));
  let created = line!() + 7;

  Effect::new({
    let held = Rc::clone(&held);

    move |_| {
      rerun.get();
      held.set(Some(Signal::new(1)));
    }
  });

  let first = held.get().expect("the first run's signal");

  rerun.set(1);
  first.set(2);

  let message = panic_message(|| _ = first.get());

  assert!(
    message.contains("disposed with its owner's re-run or teardown"),
    "{message}"
  );
  assert!(
    message.contains(&format!("created at {}:{created}:", file!())),
    "{message}"
  );
}
