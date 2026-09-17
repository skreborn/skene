//! The single-thread executor a host platform installs: scope-bound tasks queued on the
//! platform's thread and polled when that thread pumps them.
//!
//! [`install`] makes it the reactive engine's spawner; [`drive`] queues a future;
//! [`pump`] polls the ready ones, from an event loop between events or from a test.
//! Wakers may fire from any thread—the readiness queue is shared—while the futures
//! themselves stay on the installing thread.

use std::{
  cell::RefCell,
  future::Future,
  pin::Pin,
  sync::{Arc, Mutex},
  task::{Context, Wake, Waker},
};

use rustc_hash::FxHashMap;

/// The upper bound on pump rounds, so a task that always wakes itself cannot starve
/// the thread.
const MAX_ROUNDS: u32 = 1024;

/// The cross-thread readiness queue: ids of tasks whose wakers fired.
struct Shared {
  ready: Mutex<Vec<u64>>,
}

/// The thread's tasks and their shared readiness queue.
struct Executor {
  tasks: FxHashMap<u64, Pin<Box<dyn Future<Output = ()>>>>,
  shared: Arc<Shared>,
  next: u64,
}

impl Executor {
  fn new() -> Self {
    Self {
      tasks: FxHashMap::default(),
      shared: Arc::new(Shared {
        ready: Mutex::new(Vec::new()),
      }),
      next: 0,
    }
  }
}

thread_local! {
  static EXECUTOR: RefCell<Executor> = RefCell::new(Executor::new());
}

/// A task's waker: pushing its id onto the shared queue re-polls it at the next pump.
struct TaskWaker {
  id: u64,
  shared: Arc<Shared>,
}

impl Wake for TaskWaker {
  fn wake(self: Arc<Self>) {
    self.wake_by_ref();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self
      .shared
      .ready
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .push(self.id);
  }
}

/// Queues `future` on this thread's executor; the next [`pump`] polls it.
pub fn drive(future: impl Future<Output = ()> + 'static) {
  EXECUTOR.with(|executor| {
    let mut executor = executor.borrow_mut();
    let id = executor.next;

    executor.next += 1;
    executor.tasks.insert(id, Box::pin(future));
    executor
      .shared
      .ready
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .push(id);
  });
}

/// Polls every ready task until none remain ready; `true` when any ran.
#[must_use]
pub fn pump() -> bool {
  let mut ran = false;

  for _ in 0..MAX_ROUNDS {
    let ready = EXECUTOR.with(|executor| {
      let executor = executor.borrow();
      let mut ready = executor
        .shared
        .ready
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

      std::mem::take(&mut *ready)
    });

    if ready.is_empty() {
      break;
    }

    ran = true;

    for id in ready {
      // Take the future out before polling: its body may re-enter `drive`/`wake`,
      // which borrow the executor.
      let (future, shared) = EXECUTOR.with(|executor| {
        let mut executor = executor.borrow_mut();
        let shared = Arc::clone(&executor.shared);

        (executor.tasks.remove(&id), shared)
      });

      let Some(mut future) = future
      else {
        continue;
      };

      let waker = Waker::from(Arc::new(TaskWaker { id, shared }));
      let mut context = Context::from_waker(&waker);

      if future.as_mut().poll(&mut context).is_pending() {
        EXECUTOR.with(|executor| executor.borrow_mut().tasks.insert(id, future));
      }
    }
  }

  ran
}

/// Drops every task and queued wake, readying the executor for an independent mount.
pub fn clear() {
  EXECUTOR.with(|executor| {
    let mut executor = executor.borrow_mut();

    executor.tasks.clear();
    executor
      .shared
      .ready
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .clear();
  });
}

/// Installs this executor as the reactive engine's spawner: every spawned task is
/// scope-bound and queued here.
pub fn install() {
  skene_reactive::set_spawner(|task| drive(crate::scope_bound(task)));
}
