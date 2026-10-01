//! Drag and drop in the terminal: cards moving between two lists. Press a card and move
//! off its cell to drag it, release over the other list to drop it, `Escape` to abandon
//! the drag, `Ctrl-Q` to quit.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  skene_terminal::launch(terminal_drag_and_drop::app);
}

#[cfg(target_arch = "wasm32")]
const fn main() {}
