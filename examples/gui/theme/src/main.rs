//! The server-render command: `wasmtime theme-server.wasm /path` prints the page.

fn main() {
  #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
  theme::__skene_main();
}
