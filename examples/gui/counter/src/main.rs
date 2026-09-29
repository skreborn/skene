//! The server-render command: `counter-server /path` prints the page.

#[cfg(skene_platform = "server")]
fn main() {
  counter::__skene_main();
}

#[cfg(not(skene_platform = "server"))]
const fn main() {}
