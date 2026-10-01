//! The server-render command: `drag-and-drop-server /path` prints the page.

#[cfg(skene_platform = "server")]
fn main() {
  drag_and_drop::__skene_main();
}

#[cfg(not(skene_platform = "server"))]
const fn main() {}
