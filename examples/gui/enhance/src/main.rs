//! The server-render command: `wasmtime enhance-server.wasm /path` prints the page.

fn main() {
  #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
  enhance::__skene_main();
}
