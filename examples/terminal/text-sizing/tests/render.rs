//! The text sizing example rendered headlessly: every section paints at one cell
//! until the terminal proves the protocol, and the title takes three rows after it.

#![cfg(not(target_arch = "wasm32"))]

use skene_terminal::{
  self as terminal,
  CellScale,
  prelude::*,
  testing::{self, with_platform},
};

#[test]
fn the_sections_render_at_one_cell_and_size_up_when_proven() {
  with_platform(|| {
    let _ = terminal::mount(|| terminal_text_sizing::app().build());

    let frame = testing::render_to_text(70, 40);

    for expected in ["Text sizing", "Scales", "Baseline", "Themed at two."] {
      assert!(
        frame.contains(expected),
        "{expected} did not paint:\n{frame}"
      );
    }

    assert!(
      testing::render_to_scales(70, 40)
        .iter()
        .all(|scale| *scale == CellScale::ONE),
      "unproven, every run is one cell"
    );

    testing::trigger_text_sizing(true);

    let frame = testing::render_to_text(70, 40);

    assert!(frame.contains("T  e  x  t"), "{frame}");
    assert!(
      testing::render_to_scales(70, 40).contains(&CellScale::Whole(3)),
      "the title draws at three"
    );
    assert!(
      testing::render_to_scales(70, 40).contains(&CellScale::Fraction(1, 2)),
      "the fraction draws at a half"
    );
  });
}
