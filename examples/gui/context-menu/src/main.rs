//! The server-render command: `wasmtime context-menu-server.wasm /path` prints the page.

fn main() {
  #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
  context_menu::__skene_main();
}
