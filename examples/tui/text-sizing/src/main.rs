//! Text sizing in the terminal: headings in cells where the terminal sizes text, one
//! cell tall elsewhere. `Ctrl-Q` quits.

fn main() {
  #[cfg(not(target_arch = "wasm32"))]
  skene_tui::launch(tui_text_sizing::app);
}
