//! Enhancement in the terminal: scripting is live at once, and a painted form's cell
//! fallback stands where the terminal draws no pixels. `Ctrl-Q` quits.

fn main() {
  #[cfg(not(target_arch = "wasm32"))]
  skene_tui::launch(tui_enhance::app);
}
