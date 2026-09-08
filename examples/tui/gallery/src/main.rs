//! The terminal gallery: every built-in view and control, live, in cells.
//! `Tab` moves focus, arrows adjust, `Enter` clicks, `Ctrl-Q` quits.

fn main() {
  #[cfg(not(target_arch = "wasm32"))]
  skene_tui::launch(tui_gallery::app);
}
