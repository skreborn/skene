//! The server-render command: `http-server /path` prints the page.

#[cfg(skene_platform = "server")]
fn main() {
  web_http::__skene_main();
}

#[cfg(not(skene_platform = "server"))]
const fn main() {}
