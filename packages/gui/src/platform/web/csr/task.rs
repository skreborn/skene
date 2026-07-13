//! Driving futures on the browser's event loop.

use std::future::Future;

use wasm_bindgen_futures::spawn_local;

/// Runs `future` to completion on the browser's event loop.
///
/// The one executor primitive a web leaf provides: hand a `!Send` future to the event loop.
/// Scope-bound cancellation is layered on top in the platform-independent
/// [`spawn`](crate::spawn), so a leaf implements only this. A custom in-WASM executor, or a
/// worker-backed multi-threaded one, would replace this single function without touching
/// `spawn` or its callers.
#[inline(always)]
pub fn drive(future: impl Future<Output = ()> + 'static) {
  spawn_local(future);
}
