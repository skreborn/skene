//! Context menus in the terminal: a menu each note opts into, a field whose menu adds
//! the clipboard's standard items, and the terminal's standard menu everywhere else.
//! `Tab` moves focus, a right click or `Shift-F10` opens a menu, `Ctrl-Q` quits.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  skene_terminal::launch(terminal_context_menu::app);
}

#[cfg(target_arch = "wasm32")]
const fn main() {}
