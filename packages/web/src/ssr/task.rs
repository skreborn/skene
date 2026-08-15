//! The SSR executor and its WASI reactor.
//!
//! Off a JS host there is no `spawn_local`, so this leaf drives futures itself. [`drive`] fills
//! the platform executor seam; [`stabilize`] polls every ready task, and when none are ready but
//! host operations are outstanding it blocks the runner on the WASI `poll` until one completes—
//! so an async load (a [`sleep`], an HTTP call registered with the reactor) resolves and the
//! render settles on its *loaded* tree rather than a placeholder.

use std::{
  cell::{Cell, RefCell},
  future::Future,
  pin::Pin,
  sync::Arc,
  task::{Context, Poll, Wake, Waker},
  time::Duration,
};

use rustc_hash::FxHashMap;
use wasi::{clocks::monotonic_clock, io::poll::Pollable};

/// The upper bound on polling rounds, so a task that always wakes itself (an unbounded resource
/// waterfall) fails the render rather than hanging the runner forever.
const MAX_ROUNDS: u32 = 1024;

/// The default time budget a render may spend settling before it is cut off.
const DEFAULT_DEADLINE: Duration = Duration::from_secs(10);

thread_local! {
  static EXECUTOR: RefCell<Executor> = RefCell::new(Executor::new());
  static REACTOR: RefCell<Reactor> = const { RefCell::new(Reactor::new()) };
  static BUDGET: Cell<Duration> = const { Cell::new(DEFAULT_DEADLINE) };
}

/// Sets the time budget every following render may spend settling its async work.
///
/// A render that exceeds it stops waiting, reports what failed to settle, and
/// serializes the tree as it stands—placeholders included—rather than hanging its
/// caller. Ten seconds by default.
pub fn set_render_deadline(budget: Duration) {
  BUDGET.with(|current| current.set(budget));
}

/// The set of live tasks and the ids ready to be polled.
struct Executor {
  tasks: FxHashMap<u64, Pin<Box<dyn Future<Output = ()>>>>,
  ready: Vec<u64>,
  next: u64,
}

impl Executor {
  fn new() -> Self {
    Self {
      tasks: FxHashMap::default(),
      ready: Vec::new(),
      next: 0,
    }
  }
}

/// A task's waker: pushing its id back onto the ready list re-polls it next round.
struct TaskWaker(u64);

impl Wake for TaskWaker {
  fn wake(self: Arc<Self>) {
    self.wake_by_ref();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    EXECUTOR.with(|executor| executor.borrow_mut().ready.push(self.0));
  }
}

/// The outstanding host operations to wait on: a WASI [`Pollable`] paired with the task waker
/// that fires when it is ready.
struct Reactor {
  waiters: Vec<(Pollable, Waker)>,
}

impl Reactor {
  const fn new() -> Self {
    Self {
      waiters: Vec::new(),
    }
  }

  /// Blocks the runner on the outstanding operations until at least one is ready—or the
  /// deadline instant passes—waking the ready tasks; returns whether there was anything
  /// to wait on.
  fn block(&mut self, deadline: u64) -> bool {
    if self.waiters.is_empty() {
      return false;
    }

    let cutoff = monotonic_clock::subscribe_instant(deadline);
    let mut pollables: Vec<&Pollable> = self.waiters.iter().map(|(pollable, _)| pollable).collect();

    pollables.push(&cutoff);

    let mut ready = wasi::io::poll::poll(&pollables);

    drop(pollables);

    // The cutoff's own index is not a waiter; passing the deadline wakes nothing.
    ready.retain(|index| (*index as usize) < self.waiters.len());

    // Remove highest-index first so earlier removals do not shift the later ones.
    ready.sort_unstable_by(|a, b| b.cmp(a));

    for index in ready {
      let (_pollable, waker) = self.waiters.remove(index as usize);

      waker.wake();
    }

    true
  }
}

/// Registers a host operation's pollable and the waker to fire when it completes. A future
/// returning [`Poll::Pending`] on a host operation calls this so [`stabilize`] can wait on it.
pub(super) fn register(pollable: Pollable, waker: Waker) {
  REACTOR.with(|reactor| reactor.borrow_mut().waiters.push((pollable, waker)));
}

/// A future that completes after `duration`, backed by the WASI monotonic clock.
///
/// The SSR runner's one timing primitive, and how an async load is driven and tested: it
/// registers a clock pollable with the reactor, so the render blocks on it and resumes when the
/// deadline passes.
pub fn sleep(duration: Duration) -> impl Future<Output = ()> {
  Sleep {
    duration,
    registered: false,
  }
}

struct Sleep {
  duration: Duration,
  registered: bool,
}

impl Future for Sleep {
  type Output = ();

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
    let this = self.get_mut();

    // Woken only when the clock pollable is ready, so a re-poll means the deadline has passed.
    if this.registered {
      return Poll::Ready(());
    }

    let nanoseconds = u64::try_from(this.duration.as_nanos()).unwrap_or(u64::MAX);

    register(
      monotonic_clock::subscribe_duration(nanoseconds),
      context.waker().clone(),
    );

    this.registered = true;

    Poll::Pending
  }
}

/// Runs `future` to completion on the runner's executor.
///
/// The one executor primitive the SSR leaf provides; scope-bound cancellation is layered on
/// top in the platform-independent `spawn`, so this leaf implements only the
/// raw drive. The task makes progress only while [`stabilize`] is pumping.
pub fn drive(future: impl Future<Output = ()> + 'static) {
  EXECUTOR.with(|executor| {
    let mut executor = executor.borrow_mut();
    let id = executor.next;
    executor.next += 1;

    executor.tasks.insert(id, Box::pin(future));
    executor.ready.push(id);
  });
}

/// Polls every ready task, blocking on the reactor when none are ready but host operations are
/// outstanding, until the render settles.
///
/// Returns once nothing is ready and nothing is outstanding, or after [`MAX_ROUNDS`] so a
/// self-waking task cannot hang the render.
pub(super) fn stabilize() {
  stabilize_until(|| false);
}

/// Pumps the executor until `done` reports the render has progressed far enough, then returns—
/// leaving any work `done` does not depend on still outstanding.
///
/// The streaming counterpart to [`stabilize`]: the shell settles against its boundary, then each
/// deferred boundary settles against its own, so the render can flush what is ready and block only
/// on the next thing it needs. Like [`stabilize`] it also returns once nothing is ready and
/// nothing is outstanding, or after [`MAX_ROUNDS`].
pub(super) fn stabilize_until(done: impl Fn() -> bool) {
  let budget = BUDGET.with(Cell::get);
  let deadline =
    monotonic_clock::now().saturating_add(u64::try_from(budget.as_nanos()).unwrap_or(u64::MAX));

  for _ in 0..MAX_ROUNDS {
    if done() {
      return;
    }

    if monotonic_clock::now() >= deadline {
      report_unsettled("its deadline", budget);

      return;
    }

    let ready = EXECUTOR.with(|executor| std::mem::take(&mut executor.borrow_mut().ready));

    if ready.is_empty() {
      // Nothing synchronously ready: block on any outstanding host operations, else settle.
      if REACTOR.with(|reactor| reactor.borrow_mut().block(deadline)) {
        continue;
      }

      return;
    }

    for id in ready {
      // Take the future out before polling: its body may re-enter `drive`/`wake`, which
      // borrows the executor.
      let Some(mut future) = EXECUTOR.with(|executor| executor.borrow_mut().tasks.remove(&id))
      else {
        continue;
      };

      let waker = Waker::from(Arc::new(TaskWaker(id)));
      let mut context = Context::from_waker(&waker);

      if future.as_mut().poll(&mut context).is_pending() {
        EXECUTOR.with(|executor| executor.borrow_mut().tasks.insert(id, future));
      }
    }
  }

  report_unsettled("its polling bound", BUDGET.with(Cell::get));
}

/// Reports a render cut off before its async work settled: what stopped it and what is
/// still alive. The render serializes as it stands—placeholders included.
fn report_unsettled(cutoff: &str, budget: Duration) {
  let tasks = EXECUTOR.with(|executor| executor.borrow().tasks.len());
  let waiting = REACTOR.with(|reactor| reactor.borrow().waiters.len());

  tracing::error!(
    "render hit {cutoff} (budget {budget:?}) before settling: \
     {tasks} task(s) alive, {waiting} host operation(s) outstanding",
  );
}

/// Drops every task and outstanding operation, readying the executor for the next render.
pub(super) fn clear() {
  EXECUTOR.with(|executor| {
    let mut executor = executor.borrow_mut();

    executor.tasks.clear();
    executor.ready.clear();
    executor.next = 0;
  });

  REACTOR.with(|reactor| reactor.borrow_mut().waiters.clear());
}
