//! The server-render command: `enhance-server /path` prints the page.

fn main() {
  #[cfg(skene_platform = "server")]
  enhance::__skene_main();
}
