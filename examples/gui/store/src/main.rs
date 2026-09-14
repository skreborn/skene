//! The server-render command: `wasmtime store-server.wasm /path` prints the page.

fn main() {
  #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
  store::__skene_main();
}
