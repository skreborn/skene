//! The browser transport: a socket to a host that speaks no WebSocket fails to open,
//! and a URL of another scheme is refused before anything is tried.

#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use skene_ws::Error;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn a_host_without_websockets_fails_to_open() {
  let outcome = skene_ws::connect("ws://127.0.0.1:1/").await;

  assert!(matches!(outcome, Err(Error::Transport(_))), "{outcome:?}");
}

#[wasm_bindgen_test]
async fn a_url_of_another_scheme_is_refused() {
  let outcome = skene_ws::connect("http://not-a-socket/").await;

  assert!(matches!(outcome, Err(Error::Url(_))), "{outcome:?}");
}
