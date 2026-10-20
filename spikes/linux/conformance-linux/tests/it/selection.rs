//! A text field's selection under the pointer with the keyboard's modifiers, on the real
//! platform, through the driver: a click with Shift held extends the selection from the caret,
//! and once Shift is released, alone or after a chord, a click places the caret again.

use skene_gui::{
  prelude::*,
  testing::{self, Role, Screen},
};

/// One field, "Words", holding "alpha beta gamma".
fn page() -> impl View {
  Column::new()
    .set_padding(16)
    .add_child(TextField::new("Words").set_text("alpha beta gamma"))
}

/// The field's selection as a reader reads it, its ends in order; the caret alone where
/// nothing is selected.
fn selection(screen: &Screen) -> (usize, usize) {
  let read = screen.find_by_role(Role::TextField, "Words").read();
  let selection = read.selection.expect("the field reports its selection");

  (
    selection.start.min(selection.end),
    selection.start.max(selection.end),
  )
}

#[skene_gui::test]
fn a_shift_click_extends_and_a_released_shift_lets_a_click_place_the_caret() {
  let screen = testing::render(page);
  let field = screen.find_by_role(Role::TextField, "Words");
  let at = |offset| {
    screen
      .driver()
      .character_point(field.element(), offset, 0.05)
  };

  let point = at(2);

  screen.driver().click_point(point);
  assert_eq!(selection(&screen), (2, 2), "a click places the caret");

  let point = at(8);

  screen.driver().shift_click_point(point);
  assert_eq!(
    selection(&screen),
    (2, 8),
    "a click with Shift held extends from the caret"
  );

  let point = at(12);

  screen.driver().click_point(point);
  assert_eq!(
    selection(&screen),
    (12, 12),
    "once the Shift of the click is released, a click places the caret"
  );

  screen.press("Shift_L");

  let point = at(4);

  screen.driver().click_point(point);
  assert_eq!(
    selection(&screen),
    (4, 4),
    "a Shift pressed and released alone holds nothing for the next click"
  );

  screen.press("shift+Left");

  let point = at(10);

  screen.driver().click_point(point);
  assert_eq!(
    selection(&screen),
    (10, 10),
    "nor does the Shift of a chord"
  );
}
