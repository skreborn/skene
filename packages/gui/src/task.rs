//! Spawning scope-bound tasks.

use std::future::Future;

/// Runs `future` as a task, until it finishes or the current
/// [`Owner`](skene_reactive::Owner) is disposed.
///
/// Owner-bound on purpose: a task started while building a view ends with that view—or
/// with any ancestor of it—rather than waking later to write into a tree that is gone.
///
/// The future runs on the target's platform executor: on the web that is the event
/// loop, so it resumes on the UI thread and may touch signals and views directly.
/// Cancellation takes effect at the future's next poll, so a task parked on a pending
/// await stops there rather than resuming into a disposed tree.
#[inline(always)]
pub fn spawn(future: impl Future<Output = ()> + 'static) {
  skene_web::drive(skene_core::scope_bound(future));
}
