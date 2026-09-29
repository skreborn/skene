//! The server-render command: `router-server /path` prints the page.

#[cfg(skene_platform = "server")]
fn main() {
  router::__skene_main();
}

#[cfg(not(skene_platform = "server"))]
const fn main() {}
