//! Text sizing in the terminal: headings in cells where the terminal sizes text, one
//! cell tall elsewhere. `Ctrl-Q` quits.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  skene_terminal::launch(terminal_text_sizing::app);
}

#[cfg(target_arch = "wasm32")]
const fn main() {}
