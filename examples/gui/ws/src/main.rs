//! The server-render command: `ws-server /path` prints the page.

fn main() {
  #[cfg(skene_platform = "server")]
  gui_ws::__skene_main();
}
