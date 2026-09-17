//! The server-render command: `router-server /path` prints the page.

fn main() {
  #[cfg(skene_platform = "server")]
  router::__skene_main();
}
