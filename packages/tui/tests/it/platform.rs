//! The build's platform, as a const.

use skene_tui::Platform;

/// The terminal's build names the terminal, as a const.
#[test]
fn the_build_names_the_terminal() {
  const _: () = assert!(matches!(Platform::CURRENT, Platform::Terminal));

  assert_eq!(Platform::CURRENT, Platform::Terminal);
  assert_eq!(Platform::CURRENT.name(), "terminal");
}
