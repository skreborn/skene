//! The browser transport against the test runner's own server: the page comes back
//! with its status, headers and body, and a missing path is a `404` response.

#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use skene_transport::http::Request;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn the_page_comes_back_whole() {
  let response = skene_transport::http::get("/").await.expect("a response");

  assert_eq!(response.status(), 200);
  assert!(response.is_success());
  assert!(
    response
      .header("content-type")
      .is_some_and(|value| value.contains("html")),
    "{:?}",
    response.headers()
  );
  assert!(response.text().expect("text").contains("<"));
}

#[wasm_bindgen_test]
async fn a_missing_path_is_a_response() {
  let response = Request::get("/definitely-not-served")
    .set_header("X-Test", "yes")
    .send()
    .await
    .expect("a response");

  assert_eq!(response.status(), 404);
  assert!(!response.is_success());
}
