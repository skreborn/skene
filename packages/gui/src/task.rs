//! Spawning scope-bound tasks.

use std::future::Future;

use skene_reactive::TaskHandle;

/// Runs `future` as a task of the current [`Owner`](skene_reactive::Owner) until it
/// finishes, the owner goes or the returned handle cancels it; once it ends it is never
/// polled again.
///
/// A task started while building a view ends with that view, or with any ancestor of
/// it. The future runs on the platform's executor: on the web the event loop, on the UI
/// thread, so it may touch signals and views directly; on the host the headless
/// platform's queue, drained by `pump_tasks`.
#[inline(always)]
pub fn spawn(future: impl Future<Output = ()> + 'static) -> TaskHandle {
  #[cfg(not(skene_leaf = "host"))]
  skene_web::install_executor();
  #[cfg(skene_leaf = "host")]
  skene_headless::install_executor();

  skene_reactive::spawn(future)
}
