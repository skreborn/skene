#[cfg(debug_assertions)]
use std::panic::Location;
use std::{
  cell::{Cell, RefCell, RefMut},
  ops::{Deref, DerefMut},
  rc::Rc,
  thread,
};

use crate::{graph::Graph, id::Id};

thread_local! {
  pub(crate) static RUNTIME: Runtime = Runtime::new();
}

/// The scheduler seam: receives one flush to run later.
type Scheduler = dyn Fn(Box<dyn FnOnce()>);

/// How many times one effect may run in one flush before a debug build panics.
#[cfg(debug_assertions)]
const MAX_RUNS_PER_FLUSH: u32 = 10_000;

/// Clears the flushing flag when the flush unwinds.
struct FlushGuard<'a>(&'a Cell<bool>);

impl Drop for FlushGuard<'_> {
  fn drop(&mut self) {
    self.0.set(false);
  }
}

/// An exclusive borrow of the graph that, in debug builds, remembers who holds it.
pub(crate) struct GraphGuard<'a> {
  graph: RefMut<'a, Graph>,
  #[cfg(debug_assertions)]
  holder: &'a Cell<Option<&'static Location<'static>>>,
}

impl Deref for GraphGuard<'_> {
  type Target = Graph;

  #[inline(always)]
  fn deref(&self) -> &Graph {
    &self.graph
  }
}

impl DerefMut for GraphGuard<'_> {
  #[inline(always)]
  fn deref_mut(&mut self) -> &mut Graph {
    &mut self.graph
  }
}

#[cfg(debug_assertions)]
impl Drop for GraphGuard<'_> {
  fn drop(&mut self) {
    self.holder.set(None);
  }
}

/// The reactive runtime: one per thread, holding that thread's graph and the
/// scheduling state around it.
pub struct Runtime {
  graph: RefCell<Graph>,
  #[cfg(debug_assertions)]
  graph_holder: Cell<Option<&'static Location<'static>>>,
  pub(crate) tracking: Cell<Option<Id>>,
  pub(crate) current_owner: Cell<Id>,
  batch_depth: Cell<u32>,
  flushing: Cell<bool>,
  scheduler: RefCell<Option<Rc<Scheduler>>>,
  scheduled: Cell<bool>,
}

impl Runtime {
  fn new() -> Self {
    let mut graph = Graph::new();
    let root = graph.allocate_root_owner();

    Self {
      graph: RefCell::new(graph),
      #[cfg(debug_assertions)]
      graph_holder: Cell::new(None),
      tracking: Cell::new(None),
      current_owner: Cell::new(root),
      batch_depth: Cell::new(0),
      flushing: Cell::new(false),
      scheduler: RefCell::new(None),
      scheduled: Cell::new(false),
    }
  }

  /// Borrows the graph exclusively.
  ///
  /// # Panics
  ///
  /// Panics when the graph is already borrowed: user code ran under a runtime borrow.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn graph(&self) -> GraphGuard<'_> {
    #[cfg(debug_assertions)]
    {
      let caller = Location::caller();

      let Ok(graph) = self.graph.try_borrow_mut()
      else {
        let held = self.graph_holder.get().map_or_else(String::new, |held| {
          format!(" (held at {}:{})", held.file(), held.line())
        });

        panic!(
          "reactive graph re-entered from user code{held}; attempted at {}:{}",
          caller.file(),
          caller.line()
        );
      };

      self.graph_holder.set(Some(caller));

      GraphGuard {
        graph,
        holder: &self.graph_holder,
      }
    }

    #[cfg(not(debug_assertions))]
    GraphGuard {
      graph: self.graph.borrow_mut(),
    }
  }

  /// Resets the calling thread's reactive runtime to a fresh, empty state.
  ///
  /// Clears all signals, effects, owners, pending work, snapshot registrations and the
  /// scheduler. Cleanups are discarded, not run. Inside a batch, the batch still ends
  /// where it would have, and flushes then.
  pub fn reset() {
    let (graph, scheduler) = RUNTIME.with(|runtime| {
      runtime.tracking.set(None);
      runtime.flushing.set(false);
      runtime.scheduled.set(false);

      let scheduler = runtime.scheduler.borrow_mut().take();
      let mut fresh = Graph::new();

      runtime.current_owner.set(fresh.allocate_root_owner());

      (std::mem::replace(&mut *runtime.graph(), fresh), scheduler)
    });

    drop(graph);
    drop(scheduler);
    crate::snapshot::clear();
  }

  /// Whether a computation is tracking what it reads.
  pub(crate) fn is_tracking() -> bool {
    RUNTIME.with(|runtime| runtime.tracking.get().is_some())
  }

  /// Attaches `id` to the current owner, so disposing the owner disposes it.
  pub(crate) fn adopt_current(id: Id) {
    RUNTIME.with(|runtime| {
      let owner = runtime.current_owner.get();

      runtime.graph().adopt(id, owner);
    });
  }

  /// Runs `operation` with `tracking` as the subscribing computation, restoring the previous
  /// one afterwards.
  pub(crate) fn with_tracking<T>(tracking: Option<Id>, operation: impl FnOnce() -> T) -> T {
    struct Restore(Option<Id>);

    impl Drop for Restore {
      fn drop(&mut self) {
        let _ = RUNTIME.try_with(|runtime| runtime.tracking.set(self.0));
      }
    }

    let _restore = RUNTIME.with(|runtime| Restore(runtime.tracking.replace(tracking)));

    operation()
  }

  /// Runs `operation` with `owner` as the current owner, restoring the previous one afterwards.
  pub(crate) fn with_owner<T>(owner: Id, operation: impl FnOnce() -> T) -> T {
    struct Restore(Id);

    impl Drop for Restore {
      fn drop(&mut self) {
        let _ = RUNTIME.try_with(|runtime| runtime.current_owner.set(self.0));
      }
    }

    let _restore = RUNTIME.with(|runtime| Restore(runtime.current_owner.replace(owner)));

    operation()
  }

  /// Opens a batch; the returned guard closes it and flushes when it was the outermost.
  pub(crate) fn enter_batch(&self) -> BatchGuard<'_> {
    self.batch_depth.set(self.batch_depth.get() + 1);

    BatchGuard(self)
  }

  /// Opens the batch a computation's run is, unless a flush is running, which holds
  /// what the run invalidates until the run ends all the same.
  pub(crate) fn enter_run_batch(&self) -> Option<BatchGuard<'_>> {
    (!self.flushing.get()).then(|| self.enter_batch())
  }

  /// Flushes now, or through the scheduler when one is installed; a no-op when nothing
  /// is queued, or inside a batch or a running flush, both of which flush on their own.
  pub(crate) fn request_flush(&self) {
    if self.batch_depth.get() > 0 || self.flushing.get() || !self.graph().has_queued() {
      return;
    }

    let scheduler = self.scheduler.borrow().clone();

    match scheduler {
      Some(schedule) => {
        if !self.scheduled.replace(true) {
          schedule(Box::new(|| {
            let _ = RUNTIME.try_with(|runtime| {
              runtime.scheduled.set(false);

              if runtime.batch_depth.get() == 0 {
                runtime.flush();
              }
            });
          }));
        }
      }
      None => self.flush(),
    }
  }

  /// Drains the queues until none remain: every queued effect, then, untracked, the
  /// hooks of the signals left without a subscriber. Re-entrant calls return
  /// immediately: the running flush picks up whatever a mid-flush write enqueued.
  ///
  /// # Panics
  ///
  /// In debug builds, panics when one effect runs [`MAX_RUNS_PER_FLUSH`] times in one
  /// flush: it keeps invalidating itself.
  pub(crate) fn flush(&self) {
    if self.flushing.get() {
      return;
    }

    self.flushing.set(true);

    let _guard = FlushGuard(&self.flushing);
    #[cfg(debug_assertions)]
    let mut runs = rustc_hash::FxHashMap::<Id, u32>::default();

    loop {
      let (next, unobserved) = {
        let mut graph = self.graph();
        let next = graph.pop_queued();

        (next, next.is_none() && graph.has_unobserved())
      };

      let Some(id) = next
      else {
        if unobserved {
          Self::run_unobserved();

          continue;
        }

        break;
      };

      #[cfg(debug_assertions)]
      {
        let count = runs.entry(id).or_default();

        *count += 1;

        assert!(
          *count < MAX_RUNS_PER_FLUSH,
          "effect {id:?} ran {MAX_RUNS_PER_FLUSH} times in one flush: it keeps invalidating \
           itself"
        );
      }

      crate::computation::run_queued(id);
    }
  }

  /// Runs, untracked, the hooks of the signals left without a subscriber.
  #[cold]
  #[inline(never)]
  fn run_unobserved() {
    let hooks = with_graph(Graph::take_unobserved);

    Self::with_tracking(None, || {
      for hook in hooks {
        hook();
      }
    });
  }

  /// Installs the scheduler a write outside a batch hands its flush to.
  ///
  /// Without one, such a write flushes synchronously. With one, the first write after
  /// a flush schedules exactly one call; further writes coalesce into it until it runs.
  pub fn set_scheduler(schedule: impl Fn(Box<dyn FnOnce()>) + 'static) {
    let previous =
      RUNTIME.with(|runtime| runtime.scheduler.borrow_mut().replace(Rc::new(schedule)));

    drop(previous);
  }

  /// Runs every queued effect now, whether or not a scheduled flush is outstanding.
  pub fn flush_sync() {
    RUNTIME.with(|runtime| {
      runtime.scheduled.set(false);
      runtime.flush();
    });
  }
}

/// Closes a batch on drop, flushing when it was the outermost and nothing is unwinding.
pub(crate) struct BatchGuard<'a>(&'a Runtime);

impl Drop for BatchGuard<'_> {
  fn drop(&mut self) {
    let depth = self.0.batch_depth.get().saturating_sub(1);

    self.0.batch_depth.set(depth);

    if depth == 0 && !thread::panicking() {
      self.0.request_flush();
    }
  }
}

/// Runs `operation` with the graph borrowed exclusively.
#[cfg_attr(debug_assertions, track_caller)]
#[inline(always)]
pub(crate) fn with_graph<T>(operation: impl FnOnce(&mut Graph) -> T) -> T {
  RUNTIME.with(|runtime| operation(&mut runtime.graph()))
}

/// Runs `operation` with the graph borrowed exclusively, or nothing when the runtime is gone.
#[cfg_attr(debug_assertions, track_caller)]
#[inline(always)]
pub(crate) fn try_with_graph<T>(operation: impl FnOnce(&mut Graph) -> T) -> Option<T> {
  RUNTIME
    .try_with(|runtime| operation(&mut runtime.graph()))
    .ok()
}
