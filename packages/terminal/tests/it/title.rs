//! The window title: set, and derived from state as it changes.

use skene_terminal::{self as terminal, prelude::*, testing};

use crate::common::with_platform;

/// The terminal title follows `set_title`, and a derived title follows its signal.
#[test]
fn title_follows_state() {
  with_platform(|| {
    let name = Signal::new("one".to_owned());

    let _ = terminal::mount(|| {
      derive_title(move || format!("app: {}", name.get()));

      Text::new("body").build()
    });

    assert_eq!(testing::window_title().as_deref(), Some("app: one"));

    name.set("two".to_owned());

    assert_eq!(testing::window_title().as_deref(), Some("app: two"));

    set_title("fixed");

    assert_eq!(testing::window_title().as_deref(), Some("fixed"));
  });
}
