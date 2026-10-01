//! Theming for the terminal: a palette the reader switches, a nested theme, and a
//! scheme the reader chooses. `Tab` moves focus, arrows adjust, `Enter` clicks,
//! `Ctrl-Q` quits.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  skene_terminal::launch(terminal_theme::app);
}

#[cfg(target_arch = "wasm32")]
const fn main() {}
