//! Enhancement in the terminal: scripting is live at once, and a painted form's cell
//! fallback stands where the terminal draws no pixels. `Ctrl-Q` quits.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  skene_terminal::launch(terminal_enhance::app);
}

#[cfg(target_arch = "wasm32")]
const fn main() {}
