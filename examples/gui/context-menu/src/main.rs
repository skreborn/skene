//! The server-render command: `context-menu-server /path` prints the page.

#[cfg(skene_platform = "server")]
fn main() {
  context_menu::__skene_main();
}

#[cfg(not(skene_platform = "server"))]
const fn main() {}
