//! The server-render command: `store-server /path` prints the page.

#[cfg(skene_platform = "server")]
fn main() {
  store::__skene_main();
}

#[cfg(not(skene_platform = "server"))]
const fn main() {}
