//! Spawning scope-bound tasks.

use std::future::Future;

use skene_reactive::TaskHandle;

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

/// Installs the active platform's executor as this thread's spawner.
#[inline(always)]
pub(crate) fn install_executor() {
  crate::platform::web::install_executor();
}
