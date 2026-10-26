//! Tasks on the web platform's event loop, their scope-bound cancellation, and one-shot
//! actions on its clock.
//!
//! Run in a headless browser:
//! `cargo test-web --test browser_task`.
#![cfg(all(skene_platform = "web", not(skene_server)))]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  time::Duration,
};

use skene_gui::{
  cancel,
  reactive::{Get, Owner, Set, Signal},
  schedule,
  spawn,
  testing::{fresh_root, tick, wait},
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

/// A task parked on a pending await, so a test can cancel it mid-flight.
async fn parked() {
  for _ in 0..8 {
    tick().await;
  }
}

#[wasm_bindgen_test]
async fn a_spawned_task_runs() {
  let scope = Owner::new();
  let ran = Rc::new(Cell::new(false));

  scope.enter({
    let ran = Rc::clone(&ran);

    || {
      spawn(async move {
        ran.set(true);
      });
    }
  });

  assert!(!ran.get(), "not synchronously");

  tick().await;

  assert!(ran.get(), "on the event loop");
}

/// A task may write a signal when it finishes—the delivery path a loaded resource needs.
#[wasm_bindgen_test]
async fn a_task_can_deliver_into_a_signal() {
  let scope = Owner::new();
  let value = Signal::new(0);

  scope.enter(move || {
    spawn(async move {
      tick().await;
      value.set(7);
    });
  });

  assert_eq!(value.get(), 0);

  parked().await;

  assert_eq!(value.get(), 7, "the task's result reached the signal");
}

/// The point of scope-binding: disposing the scope stops a task that is parked mid-flight,
/// so it never resumes into a torn-down tree.
#[wasm_bindgen_test]
async fn disposing_the_scope_cancels_a_parked_task() {
  let scope = Owner::new();
  let finished = Rc::new(Cell::new(false));

  scope.enter({
    let finished = Rc::clone(&finished);

    || {
      spawn(async move {
        parked().await;
        finished.set(true);
      });
    }
  });

  tick().await;

  scope.dispose();

  parked().await;
  parked().await;

  assert!(
    !finished.get(),
    "a disposed scope's task must not resume and run its body",
  );
}

/// Cancellation reaches a task spawned deeper in the tree, not just one on the disposed
/// scope itself.
#[wasm_bindgen_test]
async fn disposing_an_ancestor_cancels_a_task() {
  let parent = Owner::new();
  let child = parent.child();
  let finished = Rc::new(Cell::new(false));

  child.enter({
    let finished = Rc::clone(&finished);

    || {
      spawn(async move {
        parked().await;
        finished.set(true);
      });
    }
  });

  tick().await;

  parent.dispose();

  parked().await;
  parked().await;

  assert!(!finished.get(), "an ancestor's disposal cancels it too");
}

/// A scheduled action runs once its delay has passed on the browser's clock, the
/// earliest due first, and a cancelled one never runs.
#[wasm_bindgen_test]
async fn scheduled_actions_run_earliest_first_unless_cancelled() {
  let (_guard, _root) = fresh_root();
  let ran = Rc::new(RefCell::new(Vec::new()));
  let action = |label: &'static str| {
    let ran = Rc::clone(&ran);

    move || ran.borrow_mut().push(label)
  };
  let _ = schedule(Duration::from_millis(40), action("late"));
  let _ = schedule(Duration::from_millis(10), action("early"));
  let cancelled = schedule(Duration::from_millis(10), action("cancelled"));

  cancel(cancelled);

  assert!(ran.borrow().is_empty(), "not synchronously");

  wait(Duration::from_millis(80)).await;

  assert_eq!(*ran.borrow(), ["early", "late"]);
}
