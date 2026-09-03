//! Delivery of an off-thread signal write through an installed scheduler.
//!
//! One test per file: `Runtime::init_on_ui_thread` sets a process-global thread id, so two
//! tests calling it from different threads in one binary would fight over it.

use std::{
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
  thread,
};

use skene_reactive::{Effect, Get, Runtime, Set, SyncSignal};

/// The flushes a test scheduler was handed.
type Flushes = Rc<RefCell<Vec<Box<dyn FnOnce()>>>>;

/// With a scheduler installed, draining the off-thread work propagates it and hands the
/// flush to the scheduler, so the effect runs when the scheduler says.
#[test]
fn an_off_thread_write_reaches_a_ui_effect_through_the_scheduler() {
  Runtime::init_on_ui_thread();

  let scheduled: Flushes = Rc::new(RefCell::new(Vec::new()));

  Runtime::set_scheduler({
    let scheduled = scheduled.clone();

    move |flush| scheduled.borrow_mut().push(flush)
  });

  let woken = Arc::new(Mutex::new(0));

  Runtime::set_sync_effect_waker({
    let woken = Arc::clone(&woken);

    move || *woken.lock().unwrap() += 1
  });

  let signal: SyncSignal<i32> = SyncSignal::new_sync(0);
  let seen = Arc::new(Mutex::new(Vec::new()));

  Effect::new({
    let seen = Arc::clone(&seen);

    move |_| seen.lock().unwrap().push(signal.get())
  });

  thread::spawn(move || signal.set(42))
    .join()
    .expect("the writer thread");

  assert_eq!(
    *woken.lock().unwrap(),
    1,
    "the off-thread write woke the UI loop"
  );
  assert!(Runtime::has_pending_work());
  assert!(
    scheduled.borrow().is_empty(),
    "nothing is scheduled before a drain"
  );

  Runtime::drain_pending_work();

  assert_eq!(&*seen.lock().unwrap(), &[0], "the drain scheduled, not ran");
  assert_eq!(scheduled.borrow().len(), 1);

  scheduled.borrow_mut().pop().expect("the flush")();

  assert_eq!(&*seen.lock().unwrap(), &[0, 42]);
}
