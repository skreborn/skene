//! The enhance example rendered headlessly: the clock's caption stands until the
//! first tick, the picture's fallback stands in for the pixels, and the stripes paint
//! once the terminal proves its graphics.

#![cfg(not(target_arch = "wasm32"))]

use skene_terminal::{
  self as terminal,
  prelude::*,
  testing::{self, with_platform},
};

#[test]
fn the_forms_the_terminal_carries_render() {
  let frame = with_platform(|| {
    let _ = terminal::mount(|| terminal_enhance::app().build());

    testing::render_to_text(60, 30)
  });

  for expected in [
    "ENHANCE",
    "Clock",
    "Waiting for the first tick.",
    "Picture",
    "Eight blue stripes",
  ] {
    assert!(
      frame.contains(expected),
      "{expected} did not paint:\n{frame}"
    );
  }

  let frame = with_platform(|| {
    let _ = terminal::mount(|| terminal_enhance::app().build());
    let _ = testing::render_to_text(60, 30);

    testing::trigger_frame(std::time::Duration::from_secs(1));

    testing::render_to_text(60, 30)
  });

  assert!(frame.contains("0 s since the first tick."), "{frame}");
  assert!(!frame.contains("Waiting for the first tick"), "{frame}");
}

#[test]
fn the_picture_paints_where_the_terminal_proves_graphics() {
  with_platform(|| {
    let _ = terminal::mount(|| terminal_enhance::app().build());

    assert!(testing::render_to_pictures(60, 30).is_empty());

    testing::trigger_graphics(true);

    let frame = testing::render_to_text(60, 30);
    let pictures: Vec<_> = testing::render_to_pictures(60, 30)
      .into_iter()
      .filter(|picture| picture.layer == 0)
      .collect();

    assert!(!frame.contains("Eight blue stripes"), "{frame}");
    assert_eq!(
      pictures.len(),
      1,
      "the cards' backdrops lie on their own layer"
    );
    assert_eq!(pictures[0].rows, 3);
    assert_eq!(
      &pictures[0].pixels[..4],
      &[51, 89, 229, 255],
      "the first stripe's blue"
    );
  });
}
