//! Driving futures on the browser's event loop.

use std::{cell::Cell, future::Future};

use wasm_bindgen_futures::spawn_local;

/// Runs `future` to completion on the browser's event loop.
///
/// The one executor primitive a web leaf provides: hand a `!Send` future to the event loop.
/// Owner-bound cancellation is layered on top in the platform-independent
/// `spawn`, so a leaf implements only this. A custom in-WASM executor, or a
/// worker-backed multi-threaded one, would replace this single function without touching
/// `spawn` or its callers.
#[inline(always)]
pub fn drive(future: impl Future<Output = ()> + 'static) {
  spawn_local(future);
}

thread_local! {
  static EXECUTOR_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// Installs this leaf's executor as the engine's task seam, once per thread: every
/// [`skene_reactive::spawn`] then runs its future through [`drive`], bound to the
/// spawning owner.
pub fn install_executor() {
  if EXECUTOR_INSTALLED.replace(true) {
    return;
  }

  skene_reactive::set_spawner(|task| drive(skene_core::scope_bound(task)));
}
