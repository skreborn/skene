//! The terminal gallery: every built-in view and control, live, in cells.
//! `Tab` moves focus, arrows adjust, `Enter` clicks, `Ctrl-Q` quits.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  skene_tui::launch(tui_gallery::app);
}

#[cfg(target_arch = "wasm32")]
const fn main() {}
