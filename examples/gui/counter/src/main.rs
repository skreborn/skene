//! The server-render command: `counter-server /path` prints the page.

fn main() {
  #[cfg(skene_platform = "server")]
  counter::__skene_main();
}
