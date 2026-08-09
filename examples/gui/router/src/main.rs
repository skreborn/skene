//! The server-render command: `wasmtime router-server.wasm /path` prints the page.

fn main() {
  #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
  router::__skene_main();
}
