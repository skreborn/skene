//! The server-render command: `http-server /path` prints the page.

fn main() {
  #[cfg(skene_platform = "server")]
  gui_http::__skene_main();
}
