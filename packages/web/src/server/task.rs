//! The server executor: the host executor driven to a fixpoint per render.
//!
//! Off a browser there is no event loop, so this leaf drives futures itself. [`drive`]
//! fills the platform executor seam; [`stabilize`] polls every ready task, and when none
//! is ready but tasks are alive it parks the render thread until a waker fires from
//! wherever the work runs—a thread around a plain client, a [`sleep`]'s timer—so an
//! async load resolves and the render settles on its *loaded* tree rather than a
//! placeholder.

use std::{
  cell::Cell,
  future::Future,
  pin::Pin,
  sync::{
    Arc,
    Mutex,
    PoisonError,
    atomic::{AtomicBool, Ordering},
  },
  task::{Context, Poll, Waker},
  thread,
  time::{Duration, Instant},
};

use skene_core::executor;

/// The upper bound on polling rounds, so a task that always wakes itself (an unbounded
/// resource waterfall) fails the render rather than hanging its caller forever.
const MAX_ROUNDS: u32 = 1024;

/// The default time budget a render may spend settling before it is cut off.
const DEFAULT_DEADLINE: Duration = Duration::from_secs(10);

thread_local! {
  static BUDGET: Cell<Duration> = const { Cell::new(DEFAULT_DEADLINE) };
  static EXECUTOR_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// Sets the time budget every following render may spend settling its async work.
///
/// A render that exceeds it stops waiting, reports what failed to settle, and
/// serializes the tree as it stands—placeholders included—rather than hanging its
/// caller. Ten seconds by default.
pub fn set_render_deadline(budget: Duration) {
  BUDGET.with(|current| current.set(budget));
}

/// A future that completes after `duration`, timed on a thread of its own.
///
/// The server leaf's one timing primitive, and how an async load is driven and tested:
/// the render parks until the timer wakes it, so a resource that waits on it settles
/// before the page serializes.
pub fn sleep(duration: Duration) -> impl Future<Output = ()> {
  Sleep {
    duration,
    timer: None,
  }
}

/// A sleeping future's timer: whether it fired, and the waker to fire it at.
struct Timer {
  done: AtomicBool,
  waker: Mutex<Option<Waker>>,
}

struct Sleep {
  duration: Duration,
  timer: Option<Arc<Timer>>,
}

impl Future for Sleep {
  type Output = ();

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
    let this = self.get_mut();
    let timer = this.timer.get_or_insert_with(|| {
      let timer = Arc::new(Timer {
        done: AtomicBool::new(false),
        waker: Mutex::new(None),
      });
      let duration = this.duration;
      let fired = Arc::clone(&timer);

      thread::spawn(move || {
        thread::sleep(duration);
        fired.done.store(true, Ordering::Release);

        if let Some(waker) = fired
          .waker
          .lock()
          .unwrap_or_else(PoisonError::into_inner)
          .take()
        {
          waker.wake();
        }
      });

      timer
    });

    *timer.waker.lock().unwrap_or_else(PoisonError::into_inner) = Some(context.waker().clone());

    if timer.done.load(Ordering::Acquire) {
      Poll::Ready(())
    }
    else {
      Poll::Pending
    }
  }
}

/// Runs `future` to completion on the render's executor; it makes progress only while
/// a render settles.
pub fn drive(future: impl Future<Output = ()> + 'static) {
  executor::drive(future);
}

/// Installs this leaf's executor as the engine's task seam, once per thread: every
/// [`skene_reactive::spawn`] then runs its future through [`drive`], bound to the
/// spawning owner.
pub fn install_executor() {
  if EXECUTOR_INSTALLED.replace(true) {
    return;
  }

  executor::install();
}

/// Polls every ready task, each round of polls one turn, parking until a waker fires
/// when none is ready but tasks are alive, until the render settles.
///
/// Returns once nothing is ready and nothing is alive, or after [`MAX_ROUNDS`] so a
/// self-waking task cannot hang the render.
pub(crate) fn stabilize() {
  stabilize_until(|| false);
}

/// Pumps the executor until `done` reports the render has progressed far enough, then
/// returns—leaving any work `done` does not depend on still outstanding.
///
/// The streaming counterpart to [`stabilize`]: the shell settles against its boundary,
/// then each deferred boundary settles against its own, so the render can flush what is
/// ready and block only on the next thing it needs. Like [`stabilize`] it also returns
/// once nothing is ready and nothing is alive, or after [`MAX_ROUNDS`].
pub(crate) fn stabilize_until(done: impl Fn() -> bool) {
  let budget = BUDGET.with(Cell::get);
  let deadline = Instant::now() + budget;

  for _ in 0..MAX_ROUNDS {
    if done() {
      return;
    }

    let now = Instant::now();

    if now >= deadline {
      report_unsettled("its deadline", budget);

      return;
    }

    if skene_reactive::batch(executor::pump) {
      continue;
    }

    if executor::task_count() == 0 {
      return;
    }

    let _ = executor::wait(deadline - now);
  }

  report_unsettled("its polling bound", budget);
}

/// Reports a render cut off before its async work settled: what stopped it and what is
/// still alive. The render serializes as it stands—placeholders included.
fn report_unsettled(cutoff: &str, budget: Duration) {
  let tasks = executor::task_count();

  tracing::error!("render hit {cutoff} (budget {budget:?}) before settling: {tasks} task(s) alive");
}

/// Drops every task, readying the executor for the next render.
pub(crate) fn clear() {
  executor::clear();
}
