//! Theming for the terminal: a palette the reader switches, a nested theme, and a
//! scheme the reader chooses. `Tab` moves focus, arrows adjust, `Enter` clicks,
//! `Ctrl-C` quits.

fn main() {
  #[cfg(not(target_arch = "wasm32"))]
  skene_tui::launch(tui_theme::app);
}
