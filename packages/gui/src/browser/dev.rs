//! Dev-loop state: take the snapshot the page kept before boot, expose `skene_snapshot`.
//!
//! The handshake pairs with the `skene` CLI's dev server: the served page carries a
//! `skene-dev` meta marker and the reload script, which keeps `skene_snapshot()`'s
//! bytes in the page's own storage before it reloads on the server's say-so and hands
//! them back through `skene_state()`; the wasm boot awaits that and stages it, and the
//! focused node and the scroll offsets ride the snapshot and are adopted once the tree
//! is mounted. Without the marker (a production page) boot runs immediately.

use js_sys::{Reflect, Uint8Array};
use skene_dev::{DEV_META, SNAPSHOT_GLOBAL, STATE_GLOBAL};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

/// Boots through the dev-state handshake when the page carries the dev marker, else
/// runs `boot` directly.
///
/// The handshake takes the snapshot the page kept, stages it into the reactive runtime,
/// exposes the `skene_snapshot` global, then runs `boot`.
pub fn boot_with_dev_state(boot: impl FnOnce() + 'static) {
  if !dev_marker_present() {
    boot();

    return;
  }

  skene_reactive::record_snapshots();

  wasm_bindgen_futures::spawn_local(async move {
    if let Some(bytes) = fetch_state().await {
      skene_reactive::restore(&bytes);
    }

    expose_snapshot();
    boot();
    super::restore::adopt_restoration();
  });
}

/// Whether the served page carries the dev server's `skene-dev` meta marker.
fn dev_marker_present() -> bool {
  web_sys::window()
    .and_then(|window| window.document())
    .and_then(|document| {
      document
        .query_selector(&format!("meta[name={DEV_META}]"))
        .ok()
    })
    .flatten()
    .is_some()
}

/// The snapshot the page kept across its reload, through the dev script's
/// `skene_state()`, or `None` when there is none.
async fn fetch_state() -> Option<Vec<u8>> {
  let window = web_sys::window()?;
  let state = Reflect::get(&window, &JsValue::from_str(STATE_GLOBAL)).ok()?;
  let state: js_sys::Function = state.dyn_into().ok()?;
  let promise: js_sys::Promise = state.call0(&window).ok()?.dyn_into().ok()?;
  let bytes = JsFuture::from(promise).await.ok()?;
  let bytes: Uint8Array = bytes.dyn_into().ok()?;

  Some(bytes.to_vec())
}

/// Exposes `window.skene_snapshot`, returning the current tagged-state bytes; the name
/// is the framework's own, reserved by the build it controls.
fn expose_snapshot() {
  let Some(window) = web_sys::window()
  else {
    return;
  };

  let closure = Closure::wrap(
    Box::new(|| Uint8Array::from(skene_reactive::snapshot().as_slice()))
      as Box<dyn Fn() -> Uint8Array>,
  );

  let _ = Reflect::set(
    &window,
    &JsValue::from_str(SNAPSHOT_GLOBAL),
    closure.as_ref(),
  );

  closure.forget();
}
