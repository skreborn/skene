//! Tasks: the single-thread executor a host platform installs, and the boundaries a
//! subtree's async work counts toward.
//!
//! [`install`] makes the executor the reactive engine's spawner, each task counting as
//! pending toward the nearest enclosing [`Boundary`] ([`count_pending`]); [`start`] runs a
//! future under a cancelling handle and [`drive`] runs one detached; [`pump`] runs the
//! woken tasks, from an event loop between events or from a test. A task may be woken from
//! any thread—the run queue is shared, a task is queued once however often it is woken,
//! and the notifier [`set_notifier`] installs hears of each—while the futures themselves
//! stay on the installing thread.

use std::{
  cell::{Cell, RefCell},
  collections::VecDeque,
  future::Future,
  sync::{Arc, Condvar, Mutex, PoisonError},
  time::Duration,
};

use async_task::{Runnable, Task};
use rustc_hash::FxHashMap;
use skene_reactive::{Owner, Signal, TaskGuard, Update, With, use_context};

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
  /// Whether an event handler is running.
  static HANDLING: Cell<bool> = const { Cell::new(false) };
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

/// The outstanding async work inside a boundary.
///
/// A task spawned within the boundary increments the count; it decrements when the task
/// finishes or its scope is disposed. The subtree is settled at zero. Threaded through
/// the reactive context ([`use_context`]), so a task spawner reports to the nearest
/// enclosing boundary.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Boundary {
  pending: Signal<usize>,
}

impl Boundary {
  /// A fresh boundary with nothing pending.
  pub fn new() -> Self {
    Self {
      pending: Signal::new(0),
    }
  }

  /// The nearest enclosing boundary, if the current subtree is inside one.
  #[must_use]
  pub fn current() -> Option<Self> {
    use_context::<Self>()
  }

  /// Records a task starting inside the boundary.
  pub fn enter(self) {
    self.pending.update(|pending| *pending += 1);
  }

  /// Records a task inside the boundary finishing or being cancelled; nothing once the
  /// boundary is gone.
  pub fn leave(self) {
    let _ = self.pending.try_update(|pending| *pending -= 1);
  }

  /// Whether all the boundary's work has settled. Tracks, so a reactive reader—a
  /// suspense swap, a derived placeholder—re-runs as work starts and finishes.
  #[must_use]
  pub fn is_settled(self) -> bool {
    self.pending.with(|pending| *pending == 0)
  }
}

impl Default for Boundary {
  fn default() -> Self {
    Self::new()
  }
}

/// Runs `handler` as `owner`, as an event handler: a task it spawns counts toward no
/// boundary.
pub(in crate::spine) fn run_handler<T>(owner: Owner, handler: impl FnOnce() -> T) -> T {
  /// Restores the previous state when the handler returns or unwinds.
  struct Restore(bool);

  impl Drop for Restore {
    fn drop(&mut self) {
      HANDLING.set(self.0);
    }
  }

  let _restore = Restore(HANDLING.replace(true));

  owner.enter(handler)
}

/// Leaves its boundary when dropped.
struct Counted(Boundary);

impl Drop for Counted {
  fn drop(&mut self) {
    self.0.leave();
  }
}

/// Counts the task `guard` stands for as pending work toward the nearest enclosing
/// [`Boundary`].
///
/// It counts until the guard drops, as it does once the task finishes or is cancelled;
/// one an event handler starts counts toward no boundary. A facade's spawner wraps the
/// guard its executor hands back in it.
#[must_use = "dropping the guard cancels the task"]
pub fn count_pending(guard: TaskGuard) -> TaskGuard {
  let boundary = if HANDLING.get() {
    None
  }
  else {
    Boundary::current()
  };

  let Some(boundary) = boundary
  else {
    return guard;
  };

  boundary.enter();

  Box::new((guard, Counted(boundary)))
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
pub fn alive() -> usize {
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
  skene_reactive::set_spawner(|task| count_pending(Box::new(start(task))));
}

#[cfg(test)]
mod tests {
  use std::{
    cell::Cell,
    future::{pending, poll_fn},
    rc::Rc,
    task::Poll,
  };

  use skene_reactive::{Owner, provide_context, spawn};

  use super::{Boundary, alive, clear, drive, install, pump};

  /// Sets its flag when dropped.
  struct DropFlag(Rc<Cell<bool>>);

  impl Drop for DropFlag {
    fn drop(&mut self) {
      self.0.set(true);
    }
  }

  /// A task its owner's disposal cancels leaves its boundary at once, and its future is
  /// dropped without another poll.
  #[test]
  fn a_cancelled_task_leaves_its_boundary_and_is_never_polled_again() {
    install();

    let boundary = Boundary::new();
    let owner = Owner::new();
    let polls = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(false));

    owner.enter(|| {
      provide_context(boundary);

      let polls = Rc::clone(&polls);
      let flag = DropFlag(Rc::clone(&dropped));

      spawn(poll_fn(move |_| {
        let _flag = &flag;

        polls.set(polls.get() + 1);

        Poll::Pending
      }));
    });

    assert!(pump());
    assert_eq!(polls.get(), 1);
    assert!(!boundary.is_settled());

    owner.dispose();

    assert!(boundary.is_settled());

    let _ = pump();

    assert!(dropped.get());
    assert_eq!(polls.get(), 1);
    assert_eq!(alive(), 0);
  }

  /// A finished task leaves its boundary once and nothing of it stays alive.
  #[test]
  fn a_finished_task_leaves_its_boundary_once() {
    install();

    let boundary = Boundary::new();
    let owner = Owner::new();

    owner.enter(|| {
      provide_context(boundary);
      spawn(async {});
    });

    assert!(!boundary.is_settled());
    assert!(pump());
    assert!(boundary.is_settled());
    assert_eq!(alive(), 0);

    owner.dispose();

    assert!(boundary.is_settled());
  }

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

    assert_eq!(alive(), 1);

    clear();

    assert_eq!(alive(), 0);
  }
}
