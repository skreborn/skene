//! The server-render command: `store-server /path` prints the page.

fn main() {
  #[cfg(skene_platform = "server")]
  store::__skene_main();
}
