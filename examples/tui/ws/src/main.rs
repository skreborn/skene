//! A WebSocket in the terminal: `tui-ws [url]` opens the URL (a public echo service
//! without one), sends a greeting and lists what comes back. `Ctrl-Q` quits.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  skene_tui::launch(|| tui_ws::app(std::env::args().nth(1)));
}

#[cfg(target_arch = "wasm32")]
const fn main() {}
