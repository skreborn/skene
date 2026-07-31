//! A counter for the terminal, targeting the TUI driver deliberately.
//!
//! The views are the same portable builders every driver realizes, but the values are
//! chosen for a terminal: lengths in cells, and only the styling a terminal renders
//! (color, border, bold, focus variants). `Tab` moves focus, `Enter` clicks, `Ctrl-C`
//! quits.

#[cfg(not(target_arch = "wasm32"))]
mod app;

fn main() {
  #[cfg(not(target_arch = "wasm32"))]
  skene::launch(app::app);
}
