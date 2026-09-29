//! The server-render command: `enhance-server /path` prints the page.

#[cfg(skene_platform = "server")]
fn main() {
  enhance::__skene_main();
}

#[cfg(not(skene_platform = "server"))]
const fn main() {}
