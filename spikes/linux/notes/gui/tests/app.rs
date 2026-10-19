//! The notes screen on the real platform: found as a reader finds it, acted on as a user acts.

use skene_gui::{
  Color,
  testing::{self, Role, Screen},
};

/// How far down the window the note titled `title` is.
fn top(screen: &Screen, title: &str) -> f32 {
  screen.find_by_role(Role::Row, title).read().bounds.y
}

#[skene_gui::test]
fn adding_selects_the_new_note() {
  let screen = testing::render(notes_gui::app);

  screen
    .find_by_role(Role::TextField, "New note")
    .type_text("Milk");
  screen.find_by_role(Role::Button, "Add").click();
  screen.find_by_role(Role::Row, "Milk");
  screen.find_by_text("Not pinned");

  assert_eq!(
    screen.find_by_role(Role::TextField, "New note").text(),
    "",
    "the draft is cleared"
  );
}

#[skene_gui::test]
fn the_field_has_focus_as_the_window_opens() {
  let screen = testing::render(notes_gui::app);

  assert!(
    screen
      .find_by_role(Role::TextField, "New note")
      .is_focused(),
    "the field asks for focus and takes it"
  );
}

#[skene_gui::test]
fn enter_in_the_field_adds_the_note() {
  let screen = testing::render(notes_gui::app);

  screen
    .find_by_role(Role::TextField, "New note")
    .type_text("Call Bo\n");
  screen.find_by_role(Role::Row, "Call Bo");
}

#[skene_gui::test]
fn clicking_a_note_shows_it_in_the_detail() {
  let screen = testing::render(notes_gui::app);

  screen.find_by_text("Select a note");
  screen.find_by_role(Role::Row, "Call Ada").click();
  screen.find_by_text("Not pinned");
  assert!(screen.query_by_text("Select a note").is_none());
}

#[skene_gui::test]
fn a_row_is_selected_from_the_keyboard() {
  let screen = testing::render(notes_gui::app);

  screen.find_by_role(Role::Row, "Groceries").press("Enter");
  screen.find_by_text("Pinned");
  assert!(screen.find_by_role(Role::Row, "Groceries").is_focused());
  assert!(screen.query_by_text("Select a note").is_none());
}

#[skene_gui::test]
fn the_pin_switch_owns_its_press_and_reorders() {
  let screen = testing::render(notes_gui::app);

  assert!(top(&screen, "Groceries") < top(&screen, "Call Ada"));

  let groceries = screen.find_by_role(Role::Switch, "Pin Groceries");

  assert!(groceries.is_on());
  groceries.click();
  assert!(!groceries.is_on());
  assert!(
    top(&screen, "Groceries") < top(&screen, "Call Ada"),
    "unpinned keep their order"
  );

  screen.find_by_role(Role::Switch, "Pin Call Ada").click();

  assert!(
    top(&screen, "Call Ada") < top(&screen, "Groceries"),
    "pinned notes come first"
  );

  screen.find_by_text("Select a note");
}

#[skene_gui::test]
fn dark_mode_recolors_the_page_and_keeps_the_draft() {
  let screen = testing::render(notes_gui::app);

  screen
    .find_by_role(Role::TextField, "New note")
    .type_text("Draft");
  assert_eq!(screen.pixel(4, 4), Color::rgb(0xf5f5f7));

  screen.find_by_role(Role::Switch, "Dark mode").click();
  assert_eq!(screen.pixel(4, 4), Color::rgb(0x1c1c1e));
  assert_eq!(
    screen.find_by_role(Role::TextField, "New note").text(),
    "Draft"
  );

  screen.find_by_role(Role::Switch, "Dark mode").click();
  assert_eq!(screen.pixel(4, 4), Color::rgb(0xf5f5f7));
}

#[skene_gui::test]
fn a_wide_window_pads_the_page_more() {
  let screen = testing::render(notes_gui::app);

  screen.set_width(360);

  let narrow = screen.find_by_text("Notes").read().bounds.x;

  screen.set_width(1000);

  let wide = screen.find_by_text("Notes").read().bounds.x;

  assert_eq!(wide - narrow, 16.);
  assert!(screen.audit().is_empty(), "{:#?}", screen.audit());
}
