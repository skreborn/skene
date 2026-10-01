//! The single-thread executor a host platform installs: tasks queued on the platform's
//! thread and run when that thread pumps them.
//!
//! [`install`] makes it the reactive engine's spawner; [`start`] runs a future under a
//! cancelling handle and [`drive`] runs one detached; [`pump`] runs the woken tasks,
//! from an event loop between events or from a test. A task may be woken from any
//! thread—the run queue is shared, a task is queued once however often it is woken,
//! and the notifier [`set_notifier`] installs hears of each—while the futures
//! themselves stay on the installing thread.

use std::{
  cell::{Cell, RefCell},
  collections::VecDeque,
  future::Future,
  sync::{Arc, Condvar, Mutex, PoisonError},
  time::Duration,
};

use async_task::{Runnable, Task};
use rustc_hash::FxHashMap;

/// The upper bound on pump rounds, so a task that always wakes itself cannot starve
/// the thread.
const MAX_ROUNDS: u32 = 1024;

/// What a waker, on any thread, shares with the executor: the tasks woken and not yet
/// run, and who hears of a wake.
struct Shared {
  queue: Mutex<VecDeque<Runnable>>,
  woken: Condvar,
  notifier: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl Shared {
  /// Queues a woken task, wakes a thread waiting for one and tells the notifier.
  fn schedule(&self, runnable: Runnable) {
    self
      .queue
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .push_back(runnable);
    self.woken.notify_all();

    let notifier = self
      .notifier
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .clone();

    if let Some(notify) = notifier {
      notify();
    }
  }

  /// Takes every task woken so far.
  fn take(&self) -> VecDeque<Runnable> {
    std::mem::take(&mut *self.queue.lock().unwrap_or_else(PoisonError::into_inner))
  }
}

thread_local! {
  static SHARED: Arc<Shared> = Arc::new(Shared {
    queue: Mutex::new(VecDeque::new()),
    woken: Condvar::new(),
    notifier: Mutex::new(None),
  });
  /// How many tasks are alive on this thread.
  static ALIVE: Cell<usize> = const { Cell::new(0) };
  /// The detached tasks, by id, so a clear can cancel them.
  static DETACHED: RefCell<FxHashMap<u64, Task<()>>> = RefCell::new(FxHashMap::default());
  /// The next detached task's id.
  static NEXT_DETACHED: Cell<u64> = const { Cell::new(0) };
}

/// Counts a task alive for as long as its future lives.
struct Alive;

impl Alive {
  fn new() -> Self {
    ALIVE.with(|alive| alive.set(alive.get() + 1));

    Self
  }
}

impl Drop for Alive {
  fn drop(&mut self) {
    let _ = ALIVE.try_with(|alive| alive.set(alive.get() - 1));
  }
}

/// Queues `future` on this thread's executor, to be run by the next [`pump`]; dropping
/// the returned task cancels it, and its future is dropped unpolled at the pump after.
pub fn start(future: impl Future<Output = ()> + 'static) -> Task<()> {
  let shared = SHARED.with(Arc::clone);
  let alive = Alive::new();
  let (runnable, task) = async_task::spawn_local(
    async move {
      let _alive = alive;

      future.await;
    },
    move |runnable| shared.schedule(runnable),
  );

  runnable.schedule();

  task
}

/// Queues `future` on this thread's executor, detached: it runs until it finishes or a
/// [`clear`] drops it.
pub fn drive(future: impl Future<Output = ()> + 'static) {
  let id = NEXT_DETACHED.with(|next| next.replace(next.get() + 1));
  let task = start(async move {
    future.await;

    let finished = DETACHED.with(|detached| detached.borrow_mut().remove(&id));

    if let Some(finished) = finished {
      finished.detach();
    }
  });

  DETACHED.with(|detached| detached.borrow_mut().insert(id, task));
}

/// Blocks until a task is woken or `timeout` passes; `true` when one is ready.
///
/// A platform whose thread has nothing else to do between pumps waits here, so a
/// waker fired from another thread resumes it at once.
#[must_use]
pub fn wait(timeout: Duration) -> bool {
  let shared = SHARED.with(Arc::clone);
  let queue = shared.queue.lock().unwrap_or_else(PoisonError::into_inner);
  let (queue, _) = shared
    .woken
    .wait_timeout_while(queue, timeout, |queue| queue.is_empty())
    .unwrap_or_else(PoisonError::into_inner);

  !queue.is_empty()
}

/// Installs `notify` to hear of every wake, on whatever thread it fires: an event loop
/// wires it to its own wake source, so a task woken elsewhere runs at once.
pub fn set_notifier(notify: impl Fn() + Send + Sync + 'static) {
  let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(notify);

  SHARED.with(|shared| {
    *shared
      .notifier
      .lock()
      .unwrap_or_else(PoisonError::into_inner) = Some(notify);
  });
}

/// The number of tasks alive on this thread, pending or woken.
#[must_use]
pub fn task_count() -> usize {
  ALIVE.with(Cell::get)
}

/// Runs every woken task until none is woken; `true` when any ran.
#[must_use]
pub fn pump() -> bool {
  let shared = SHARED.with(Arc::clone);
  let mut ran = false;

  for _ in 0..MAX_ROUNDS {
    let woken = shared.take();

    if woken.is_empty() {
      break;
    }

    ran = true;

    for runnable in woken {
      runnable.run();
    }
  }

  ran
}

/// Drops every detached task and every woken one, readying the executor for an
/// independent mount.
///
/// A task may spawn as it drops; what it spawns is queued afresh.
pub fn clear() {
  let detached = DETACHED.with(|detached| std::mem::take(&mut *detached.borrow_mut()));

  drop(detached);

  let woken = SHARED.with(|shared| shared.take());

  drop(woken);
}

/// Installs this executor as the reactive engine's spawner: every spawned task counts
/// toward its boundary and is queued here.
pub fn install() {
  skene_reactive::set_spawner(|task| crate::spine::count_pending(Box::new(start(task))));
}

#[cfg(test)]
mod tests {
  use std::future::pending;

  use super::{clear, drive, task_count};

  /// Spawns a task as it drops.
  struct SpawnOnDrop;

  impl Drop for SpawnOnDrop {
    fn drop(&mut self) {
      drive(async {});
    }
  }

  #[test]
  fn a_task_spawning_as_it_drops_survives_a_clear() {
    let spawner = SpawnOnDrop;

    drive(async move {
      let _spawner = spawner;

      pending::<()>().await;
    });
    clear();

    assert_eq!(task_count(), 1);

    clear();

    assert_eq!(task_count(), 0);
  }
}
