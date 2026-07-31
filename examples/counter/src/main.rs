//! The server-render command: `wasmtime counter-server.wasm /path` prints the page.

fn main() {
  #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
  counter::__skene_main();
}
