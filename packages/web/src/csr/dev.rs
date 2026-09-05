//! Dev-loop state: fetch the CLI-held snapshot before boot, expose `skene_snapshot`.
//!
//! The handshake pairs with the `skene` CLI's dev server: the served page carries a
//! `skene-dev` meta marker and the reload script, the wasm boot fetches `/skene:state`
//! and stages it, and the reload script beacons `skene_snapshot()` back before every
//! reload; the focused node and the scroll offsets ride the snapshot and are adopted
//! once the tree is mounted. Without the marker (a production page) boot runs
//! immediately, fetch-free.

use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

/// Boots through the dev-state handshake when the page carries the dev marker: fetch
/// `/skene:state`, stage it into the reactive runtime, expose the `skene_snapshot`
/// global, then run `boot`. Without the marker, runs `boot` directly.
pub fn boot_with_dev_state(boot: impl FnOnce() + 'static) {
  if !dev_marker_present() {
    boot();

    return;
  }

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
    .and_then(|document| document.query_selector("meta[name=skene-dev]").ok())
    .flatten()
    .is_some()
}

/// The CLI-held snapshot, or `None` when there is none yet.
async fn fetch_state() -> Option<Vec<u8>> {
  let window = web_sys::window()?;
  let response = JsFuture::from(window.fetch_with_str("/skene:state"))
    .await
    .ok()?;
  let response: web_sys::Response = response.dyn_into().ok()?;

  if !response.ok() {
    return None;
  }

  let buffer = JsFuture::from(response.array_buffer().ok()?).await.ok()?;

  Some(Uint8Array::new(&buffer).to_vec())
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
    &JsValue::from_str("skene_snapshot"),
    closure.as_ref(),
  );

  closure.forget();
}
