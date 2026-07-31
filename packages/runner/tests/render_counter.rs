//! End-to-end: load the counter example's server build and render a route through the
//! embedded runtime.
//!
//! Requires the artifact from `cargo build -p counter --target wasm32-wasip2`; the test
//! is ignored by default so plain `cargo test` needs no wasm toolchain.

#![cfg(not(target_arch = "wasm32"))]

use skene_runner::Runner;

#[test]
#[ignore = "needs the counter wasm32-wasip2 artifact built first"]
fn renders_the_counter_page() {
  let artifact = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasm32-wasip2/debug/counter-server.wasm",
  );
  let runner = Runner::load(artifact).expect("load the counter component");

  let page = runner.render("/").expect("render the root route");

  assert!(page.starts_with("<!DOCTYPE html>"));
  assert!(page.contains("COUNTER"));
  assert!(page.contains("skene-state"));

  // A second render from the same loaded component is isolated and identical.
  assert_eq!(runner.render("/").expect("render again"), page);
}
