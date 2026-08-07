//! Delivery of an off-thread signal write to a UI-thread effect.
//!
//! One test per file: `Runtime::init_on_ui_thread` sets a process-global thread id, so two
//! tests calling it from different threads in one binary would fight over it.

use std::{
  sync::{Arc, Mutex},
  thread,
};

use skene_reactive::{Effect, Get, Runtime, Set, SyncSignal};

/// A sync signal written from another thread queues its subscribed effect for the UI thread,
/// which runs it on the next drain. This is the whole path a background task needs to
/// deliver a result into the view.
#[test]
fn an_off_thread_write_reaches_a_ui_effect_on_drain() {
  Runtime::init_on_ui_thread();

  let signal: SyncSignal<i32> = SyncSignal::new_sync(0);
  let seen = Arc::new(Mutex::new(Vec::new()));

  Effect::new({
    let seen = Arc::clone(&seen);

    move |_| seen.lock().unwrap().push(signal.get())
  });

  assert_eq!(
    &*seen.lock().unwrap(),
    &[0],
    "the effect runs once when created",
  );

  thread::spawn(move || signal.set(42))
    .join()
    .expect("the writer thread");

  assert!(
    Runtime::has_pending_work(),
    "the off-thread write queued work for the UI thread",
  );

  assert_eq!(
    &*seen.lock().unwrap(),
    &[0],
    "and did not run the effect off the UI thread",
  );

  Runtime::drain_pending_work();

  assert_eq!(
    &*seen.lock().unwrap(),
    &[0, 42],
    "draining runs the effect on the UI thread, with the new value",
  );
}
