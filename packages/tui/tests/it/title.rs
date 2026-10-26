//! The window title: set, and derived from state as it changes.

use skene_tui::{platform::terminal::mount, prelude::*, testing};

use crate::common::with_platform;

/// The terminal title follows the latest title set, and a derived title follows its
/// signal.
#[test]
fn title_follows_state() {
  with_platform(|| {
    let name = Signal::new("one".to_owned());

    let _ = mount(|| {
      window::derive_title(move || format!("app: {}", name.get()));

      Text::new("body").build()
    });

    assert_eq!(testing::window_title().as_deref(), Some("app: one"));

    name.set("two".to_owned());

    assert_eq!(testing::window_title().as_deref(), Some("app: two"));

    window::set_title("fixed");

    assert_eq!(testing::window_title().as_deref(), Some("fixed"));
  });
}
