//! The server-render command: `context-menu-server /path` prints the page.

fn main() {
  #[cfg(skene_platform = "server")]
  context_menu::__skene_main();
}
