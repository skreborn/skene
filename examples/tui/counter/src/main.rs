//! A counter for the terminal: `skene-tui` is its single dependency, and its vocabulary
//! is the terminal's own—cells, colors, bold. `Tab` moves focus, `Enter` clicks,
//! `Ctrl-Q` quits.

#[cfg(not(target_arch = "wasm32"))]
mod app;

fn main() {
  #[cfg(not(target_arch = "wasm32"))]
  skene_tui::launch(app::app);
}
