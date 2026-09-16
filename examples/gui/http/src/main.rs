//! The server-render command: `wasmtime http-server.wasm /path` prints the page.

fn main() {
  #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
  gui_http::__skene_main();
}
