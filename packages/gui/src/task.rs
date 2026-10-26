//! Spawning scope-bound tasks and scheduling one-shot actions on the platform's clock.

use std::{future::Future, time::Duration};

use skene_foundation::{Realization, Timer};
use skene_reactive::TaskHandle;

use crate::Active;

/// Runs `future` as a task of the current [`Owner`](skene_reactive::Owner) until it
/// finishes, the owner goes or the returned handle cancels it; once it ends it is never
/// polled again.
///
/// A task started while building a view ends with that view, or with any ancestor of
/// it. The future runs on the platform's executor: in the browser the event loop, on
/// the UI thread, so it may touch signals and views directly; on a server render the
/// render's own executor, which drives it until the render settles.
pub fn spawn(future: impl Future<Output = ()> + 'static) -> TaskHandle {
  install_executor();

  skene_reactive::spawn(future)
}

/// Runs `action` once `delay` has passed on the platform's clock, unless the returned
/// [`Timer`] is [`cancel`]led first.
///
/// The platform's clock as an app reaches it, the one a built-in's delay waits on (a
/// tooltip's hover delay): in the browser a timeout of the event loop, run on the UI
/// thread, so the action may touch signals and views directly. A server render has no
/// clock and never runs it.
pub fn schedule(delay: Duration, action: impl FnOnce() + 'static) -> Timer {
  Active::schedule(delay, Box::new(action))
}

/// Drops the action `timer` names, if it has not run.
#[inline(always)]
pub fn cancel(timer: Timer) {
  Active::cancel(timer);
}

/// Installs the active platform's executor as this thread's spawner.
#[inline(always)]
pub(crate) fn install_executor() {
  crate::platform::web::install_executor();
}
