//! A scroll on the real platform, through the driver: focus moving to a row below the fold
//! scrolls it into view by as little as shows it, and the keys scroll (`Home`, `PageDown`, an
//! arrow, `End`), as a reader reads the rows' places.

use skene_gui::{
  prelude::*,
  testing::{self, Role, Screen},
};

/// A scroll 100 pixels tall at the content's top, holding ten activatable rows 40 pixels tall:
/// "Row 0" to "Row 9".
fn page() -> impl View {
  let rows = (0..10).fold(Column::new(), |column, index| {
    column.add_child(
      Container::new()
        .set_label(format!("Row {index}"))
        .set_height(40)
        .handle_click(|| {})
        .set_child(Text::new(format!("Row {index}"))),
    )
  });

  Column::new().add_child(
    Scroll::new(ScrollAxes::Vertical)
      .set_height(100)
      .set_child(rows),
  )
}

/// Where the first row's top is, in pixels from the content's top: how far the scroll has
/// scrolled, negated.
fn first_top(screen: &Screen) -> f32 {
  screen.find_by_role(Role::Row, "Row 0").read().bounds.y
}

/// Whether `actual` is `wanted`, within the pixel a reader's whole-pixel extents round away.
fn near(actual: f32, wanted: f32) -> bool {
  (actual - wanted).abs() <= 1.
}

#[skene_gui::test]
fn focus_scrolls_into_view_and_the_keys_scroll() {
  let screen = testing::render(page);

  for _ in 0..6 {
    screen.press("Tab");
  }

  let focused = screen.focused();

  assert_eq!(focused.name(), "Row 5");

  let bounds = focused.read().bounds;

  assert!(
    bounds.y >= -1. && bounds.y + bounds.height <= 101.,
    "the sixth row shows within the scroll: {bounds:?}"
  );
  assert!(
    near(first_top(&screen), -140.),
    "scrolled by as little as shows it: the first row at {}",
    first_top(&screen)
  );

  screen.press("Escape");
  screen.press("Home");
  assert!(
    near(first_top(&screen), 0.),
    "Home scrolls to the start: {}",
    first_top(&screen)
  );

  screen.press("PageDown");
  assert!(
    near(first_top(&screen), -87.5),
    "PageDown scrolls most of the box: {}",
    first_top(&screen)
  );

  screen.press("Down");
  assert!(
    near(first_top(&screen), -127.5),
    "an arrow scrolls a line: {}",
    first_top(&screen)
  );

  screen.press("End");
  assert!(
    near(first_top(&screen), -300.),
    "End scrolls to the end: {}",
    first_top(&screen)
  );
}
