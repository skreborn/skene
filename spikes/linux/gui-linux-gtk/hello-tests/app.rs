//! Hello on the real platform: found as a reader finds it, acted on as a user acts.

use skene_gui::{
  Color,
  testing::{self, Role},
};

#[skene_gui::test]
fn typing_a_name_greets_it() {
  let screen = testing::render(hello::app);

  screen.find_by_text("Who's there?");
  screen
    .find_by_role(Role::TextField, "Name")
    .type_text("Ada");
  screen.find_by_text("Hello, Ada!");
}

#[skene_gui::test]
fn sync_turns_off() {
  let screen = testing::render(hello::app);
  let sync = screen.find_by_role(Role::Switch, "Sync");

  assert!(sync.is_on());
  sync.click();
  assert!(!sync.is_on());
  screen.find_by_text("Sync is off");
}

#[skene_gui::test]
fn greeting_adds_a_name_once_and_reset_clears() {
  let screen = testing::render(hello::app);

  screen
    .find_by_role(Role::TextField, "Name")
    .type_text("Ada");
  screen.find_by_role(Role::Button, "Greet").click();
  screen.find_by_role(Role::Button, "Greet").click();
  screen.find_by_text("Greeted Ada");

  assert_eq!(
    screen.outline().matches("\"Greeted Ada\"").count(),
    1,
    "a name is greeted once"
  );

  screen.find_by_role(Role::Button, "Reset").click();
  screen.find_by_text("Who's there?");
  assert!(!screen.outline().contains("Greeted Ada"));
}

#[skene_gui::test]
fn the_dark_switch_reinks_the_page() {
  let screen = testing::render(hello::app);

  assert_eq!(screen.pixel(4., 4.), Color::rgb(0xf2f2f7));

  let dark = screen.find_by_role(Role::Switch, "Dark");

  dark.click();
  assert!(dark.is_on());
  assert_eq!(screen.pixel(4., 4.), Color::rgb(0x000000));

  dark.click();
  assert_eq!(screen.pixel(4., 4.), Color::rgb(0xf2f2f7));
}

#[skene_gui::test]
fn the_volume_steps_from_the_keyboard() {
  let screen = testing::render(hello::app);
  let volume = screen.find_by_role(Role::Slider, "Volume");

  assert_eq!(volume.value(), 0.5);
  volume.press("Right");
  assert!((volume.value() - 0.6).abs() < 1e-6, "{}", volume.value());
}

#[skene_gui::test]
fn the_size_is_chosen_from_its_list() {
  let screen = testing::render(hello::app);

  screen.find_by_role(Role::ComboBox, "Size").click();
  screen.find_by_text("Large").click();
  screen.find_by_text("Large");
  assert!(screen.query_by_text("Medium").is_none(), "the list closed");
}

#[skene_gui::test]
fn the_page_is_padded_wider_on_a_wide_window() {
  let screen = testing::render(hello::app);
  let title = |screen: &testing::Screen| screen.find_by_text("Hello, Skene").read().bounds.x;
  let narrow = title(&screen);

  screen.set_width(1000);

  assert_eq!(title(&screen) - narrow, (48. - 16.) + (48. - 24.));
  assert!(screen.audit().is_empty(), "{:#?}", screen.audit());
}
