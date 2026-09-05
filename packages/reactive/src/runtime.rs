#[cfg(debug_assertions)]
use std::panic::Location;
#[cfg(debug_assertions)]
use std::sync::OnceLock;
use std::{
  cell::{Cell, RefCell, RefMut},
  ops::{Deref, DerefMut},
  rc::Rc,
  sync::atomic::{AtomicBool, AtomicU64, Ordering},
  thread,
};

use crate::{dispose::dispose_sync_local, graph::Graph, id::Id, sync::SYNC};

thread_local! {
  pub(crate) static RUNTIME: Runtime = Runtime::new();
  /// This thread's token: the process-unique surrogate for its id.
  static THREAD_TOKEN: u64 = NEXT_THREAD_TOKEN.fetch_add(1, Ordering::Relaxed);
}

static NEXT_THREAD_TOKEN: AtomicU64 = AtomicU64::new(1);
/// The UI thread's token; `0` while none is designated.
static UI_THREAD: AtomicU64 = AtomicU64::new(0);
#[cfg(debug_assertions)]
static UI_THREAD_SET_LOCATION: OnceLock<&'static Location<'static>> = OnceLock::new();
static ENFORCE_UI_THREAD: AtomicBool = AtomicBool::new(false);

/// The calling thread's token.
fn thread_token() -> u64 {
  THREAD_TOKEN.with(|token| *token)
}

/// Designates the calling thread as the UI thread; returns the existing designation
/// when one is already set to a different thread.
fn set_ui_thread() -> Result<(), u64> {
  let current = thread_token();

  match UI_THREAD.compare_exchange(0, current, Ordering::AcqRel, Ordering::Acquire) {
    Ok(_) => Ok(()),
    Err(existing) if existing == current => Ok(()),
    Err(existing) => Err(existing),
  }
}

/// The scheduler seam: receives one flush to run later.
type Scheduler = dyn Fn(Box<dyn FnOnce()>);

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

      match self.graph.try_borrow_mut() {
        Ok(graph) => {
          self.graph_holder.set(Some(caller));

          GraphGuard {
            graph,
            holder: &self.graph_holder,
          }
        }
        Err(_) => {
          let held = self.graph_holder.get().map_or_else(String::new, |held| {
            format!(" (held at {}:{})", held.file(), held.line())
          });

          panic!(
            "reactive graph re-entered from user code{held}; attempted at {}:{}",
            caller.file(),
            caller.line()
          );
        }
      }
    }

    #[cfg(not(debug_assertions))]
    GraphGuard {
      graph: self.graph.borrow_mut(),
    }
  }

  /// Records the calling thread as the UI thread; call once during initialization.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn init_on_ui_thread() {
    assert!(
      set_ui_thread().is_ok(),
      "UI thread id already set to a different thread"
    );

    #[cfg(debug_assertions)]
    {
      let caller = Location::caller();
      let _ = UI_THREAD_SET_LOCATION.set(caller);
    }

    ENFORCE_UI_THREAD.store(true, Ordering::Relaxed);
  }

  /// Panics if enforcement is on and the caller is not the UI thread.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn assert_ui_thread() {
    if !ENFORCE_UI_THREAD.load(Ordering::Relaxed) {
      return;
    }

    let current = thread_token();
    let ui = UI_THREAD.load(Ordering::Acquire);

    if ui == 0 {
      // Once enforcement is on, first access defines the UI thread.
      let _ = set_ui_thread();

      return;
    }

    if ui == current {
      return;
    }

    #[cfg(debug_assertions)]
    {
      let caller = Location::caller();
      let set_info = UI_THREAD_SET_LOCATION
        .get()
        .map(|loc| format!(" (set at {}:{})", loc.file(), loc.line()))
        .unwrap_or_default();

      panic!(
        "local runtime access from non-UI thread\n  expected UI thread: #{ui}{set_info}\n  current thread: #{current} {:?}\n  caller: {}:{}",
        thread::current().id(),
        caller.file(),
        caller.line(),
      );
    }

    #[cfg(not(debug_assertions))]
    panic!(
      "local runtime access from non-UI thread: expected #{ui}, got #{current} {:?}",
      thread::current().id()
    );
  }

  /// Whether a UI thread has been designated via
  /// [`init_on_ui_thread`](Self::init_on_ui_thread).
  pub(crate) fn ui_thread_designated() -> bool {
    ENFORCE_UI_THREAD.load(Ordering::Relaxed)
  }

  /// Whether the caller is the UI thread, or `true` when enforcement is off.
  pub fn is_ui_thread() -> bool {
    if !ENFORCE_UI_THREAD.load(Ordering::Relaxed) {
      return true;
    }

    UI_THREAD.load(Ordering::Acquire) == thread_token()
  }

  /// Resets the calling thread's reactive runtime to a fresh, empty state.
  ///
  /// Clears all signals, effects, owners, pending work, snapshot registrations and the
  /// scheduler, and releases the UI-thread designation. Cleanups are discarded, not
  /// run.
  pub fn reset() {
    let (graph, scheduler) = RUNTIME.with(|runtime| {
      runtime.tracking.set(None);
      runtime.batch_depth.set(0);
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

    UI_THREAD.store(0, Ordering::Release);
    ENFORCE_UI_THREAD.store(false, Ordering::Relaxed);
  }

  /// Attaches `id` to the current owner, so disposing the owner disposes it.
  pub(crate) fn adopt_current(id: Id) {
    RUNTIME.with(|runtime| {
      let owner = runtime.current_owner.get();

      runtime.graph().adopt(id, owner);
    });
  }

  /// Runs `f` with `tracking` as the subscribing computation, restoring the previous
  /// one afterwards.
  pub(crate) fn with_tracking<T>(tracking: Option<Id>, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Id>);

    impl Drop for Restore {
      fn drop(&mut self) {
        let _ = RUNTIME.try_with(|runtime| runtime.tracking.set(self.0));
      }
    }

    let _restore = RUNTIME.with(|runtime| Restore(runtime.tracking.replace(tracking)));

    f()
  }

  /// Runs `f` with `owner` as the current owner, restoring the previous one afterwards.
  pub(crate) fn with_owner<T>(owner: Id, f: impl FnOnce() -> T) -> T {
    struct Restore(Id);

    impl Drop for Restore {
      fn drop(&mut self) {
        let _ = RUNTIME.try_with(|runtime| runtime.current_owner.set(self.0));
      }
    }

    let _restore = RUNTIME.with(|runtime| Restore(runtime.current_owner.replace(owner)));

    f()
  }

  /// Opens a batch; the returned guard closes it and flushes when it was the outermost.
  pub(crate) fn enter_batch() -> BatchGuard {
    RUNTIME.with(|runtime| runtime.batch_depth.set(runtime.batch_depth.get() + 1));

    BatchGuard
  }

  /// Flushes now, or through the scheduler when one is installed; a no-op inside a
  /// batch or a running flush, both of which flush on their own.
  pub(crate) fn request_flush(&self) {
    if self.batch_depth.get() > 0 || self.flushing.get() {
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

  /// Drains the queues until none remain. Re-entrant calls return immediately: the
  /// running flush picks up whatever a mid-flush write enqueued.
  pub(crate) fn flush(&self) {
    if self.flushing.get() {
      return;
    }

    self.flushing.set(true);

    let _guard = FlushGuard(&self.flushing);

    loop {
      let next = self.graph().pop_queued();

      let Some(id) = next
      else {
        break;
      };

      crate::computation::run_queued(id);
    }
  }

  /// Installs the scheduler a write outside a batch hands its flush to.
  ///
  /// Without one, such a write flushes synchronously. With one, the first write after
  /// a flush schedules exactly one call; further writes coalesce into it until it runs.
  /// [`drain_pending_work`](Self::drain_pending_work) routes the same way.
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

  /// Takes the work other threads queued—sync-signal writes and disposals—into this
  /// runtime, then flushes as a write would: now, or through the scheduler.
  pub fn drain_pending_work() {
    Self::assert_ui_thread();

    let writes = SYNC.take_pending_writes();
    let disposals = SYNC.take_pending_disposals();

    RUNTIME.with(|runtime| {
      let mut graph = runtime.graph();

      for id in writes {
        graph.propagate(id);
      }
    });

    for (id, parent) in disposals {
      if id.is_sync() {
        dispose_sync_local(id, parent);
      }
      else {
        id.dispose();
      }
    }

    RUNTIME.with(Self::request_flush);
  }

  /// Whether there is queued work for this runtime or the sync registry.
  pub fn has_pending_work() -> bool {
    RUNTIME.with(|runtime| runtime.graph().has_queued()) || SYNC.has_pending_work()
  }

  /// Sets a waker called when a sync signal is written or disposed off the UI thread.
  /// The waker should nudge the UI event loop into calling
  /// [`drain_pending_work`](Self::drain_pending_work).
  pub fn set_sync_effect_waker(waker: impl Fn() + Send + Sync + 'static) {
    SYNC.set_waker(waker);
  }
}

/// Closes a batch on drop, flushing when it was the outermost and nothing is unwinding.
pub(crate) struct BatchGuard;

impl Drop for BatchGuard {
  fn drop(&mut self) {
    let _ = RUNTIME.try_with(|runtime| {
      let depth = runtime.batch_depth.get() - 1;

      runtime.batch_depth.set(depth);

      if depth == 0 && !thread::panicking() {
        runtime.request_flush();
      }
    });
  }
}

/// Runs `f` with the graph borrowed exclusively.
#[cfg_attr(debug_assertions, track_caller)]
#[inline(always)]
pub(crate) fn with_graph<T>(f: impl FnOnce(&mut Graph) -> T) -> T {
  RUNTIME.with(|runtime| f(&mut runtime.graph()))
}

/// Runs `f` with the graph borrowed exclusively, or nothing when the runtime is gone.
#[cfg_attr(debug_assertions, track_caller)]
#[inline(always)]
pub(crate) fn try_with_graph<T>(f: impl FnOnce(&mut Graph) -> T) -> Option<T> {
  RUNTIME.try_with(|runtime| f(&mut runtime.graph())).ok()
}
