//! A WebSocket in the terminal: `tui-ws [url]` opens the URL (a public echo service
//! without one), sends a greeting and lists what comes back. `Ctrl-Q` quits.

fn main() {
  #[cfg(not(target_arch = "wasm32"))]
  skene_tui::launch(|| tui_ws::app(std::env::args().nth(1)));
}
