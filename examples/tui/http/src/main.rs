//! HTTP in the terminal: `tui-http [url]` fetches the URL (`https://example.com/`
//! without one) and shows what came back. `Ctrl-Q` quits.

fn main() {
  #[cfg(not(target_arch = "wasm32"))]
  skene_tui::launch(|| tui_http::app(std::env::args().nth(1)));
}
