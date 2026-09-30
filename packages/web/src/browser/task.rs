//! Driving futures on the browser's event loop.

use std::{cell::Cell, future::Future};

use async_task::{Runnable, Task};

/// Queues `future` on the browser's event loop, each wake run from a microtask; dropping
/// the returned task cancels it, and its future is dropped unpolled.
fn start(future: impl Future<Output = ()> + 'static) -> Task<()> {
  let (runnable, task) = async_task::spawn_local(future, |runnable: Runnable| {
    wasm_bindgen_futures::spawn_local(async move {
      runnable.run();
    });
  });

  runnable.schedule();

  task
}

/// Runs `future` to completion on the browser's event loop, detached.
pub fn drive(future: impl Future<Output = ()> + 'static) {
  start(future).detach();
}

thread_local! {
  static EXECUTOR_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// Installs this leaf's executor as the engine's task seam, once per thread: every
/// [`skene_reactive::spawn`] then runs its future on the event loop, counted toward its
/// boundary.
pub fn install_executor() {
  if EXECUTOR_INSTALLED.replace(true) {
    return;
  }

  skene_reactive::set_spawner(|task| skene_core::count_pending(Box::new(start(task))));
}
