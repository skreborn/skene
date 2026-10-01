//! The server-render command: `theme-server /path` prints the page.

#[cfg(skene_platform = "server")]
fn main() {
  theme::__skene_main();
}

#[cfg(not(skene_platform = "server"))]
const fn main() {}
