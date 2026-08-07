//! A `Sync` signal with subscribed effects, used without `Runtime::init_on_ui_thread`.
//!
//! One test per file: `init_on_ui_thread` sets a process-global thread id, and this test
//! deliberately never calls it—it must not share a binary with one that does.

use std::sync::{Arc, Mutex};

use skene_reactive::{Effect, Get, Set, SyncSignal};

/// Off the UI thread the hazard is real (an off-thread write's effects are dropped), which the
/// runtime now warns about. On the writing thread itself—here, the main thread with no UI
/// thread designated—the effect still runs: the warning is guidance, not a panic, and does not
/// break single-threaded use.
#[test]
fn effects_still_run_single_threaded_without_a_designated_ui_thread() {
  let signal: SyncSignal<i32> = SyncSignal::new_sync(0);
  let seen = Arc::new(Mutex::new(Vec::new()));

  Effect::new({
    let seen = Arc::clone(&seen);

    move |_| seen.lock().unwrap().push(signal.get())
  });

  signal.set(1);

  assert_eq!(
    &*seen.lock().unwrap(),
    &[0, 1],
    "the effect ran on the writing thread; the missing designation only warns",
  );
}
