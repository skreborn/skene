//! The server-render command: `ws-server /path` prints the page.

#[cfg(skene_platform = "server")]
fn main() {
  gui_ws::__skene_main();
}

#[cfg(not(skene_platform = "server"))]
const fn main() {}
