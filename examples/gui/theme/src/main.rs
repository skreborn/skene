//! The server-render command: `theme-server /path` prints the page.

fn main() {
  #[cfg(skene_platform = "server")]
  theme::__skene_main();
}
