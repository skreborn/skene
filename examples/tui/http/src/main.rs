//! HTTP in the terminal: `tui-http [url]` fetches the URL (`https://example.com/`
//! without one) and shows what came back. `Ctrl-Q` quits.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  skene_tui::launch(|| tui_http::app(std::env::args().nth(1)));
}

#[cfg(target_arch = "wasm32")]
const fn main() {}
