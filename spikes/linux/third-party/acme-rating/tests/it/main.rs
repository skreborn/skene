//! The rating on the real platform: found as a reader finds it, acted on as a user acts.

use acme_rating::star_rating;
use skene_gui::{
  AccessRequest,
  prelude::*,
  testing::{self, Role},
};

/// A rating between two buttons, and a disabled copy.
fn page() -> impl View {
  let stars = Signal::new(0_u8);
  let locked = Signal::new(4_u8);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Button::new("Before"))
    .add_child(star_rating("Rating", stars))
    .add_child(star_rating("Locked", locked).set_disabled(true))
    .add_child(Button::new("After"))
}

#[skene_gui::test]
fn a_rating_answers_the_sliders_keys_a_press_and_a_reader() {
  let screen = testing::render(page);
  let rating = screen.find_by_role(Role::Slider, "Rating");

  assert_eq!(rating.value(), 0.);

  for _ in 0..3 {
    rating.press("Right");
  }

  assert_eq!(rating.value(), 3., "the slider's arrows step it");

  rating.click_at(0.9, 0.5);
  assert_eq!(rating.value(), 5., "a press on the fifth star rates five");

  rating.trigger_access(AccessRequest::SetValue(2.));
  assert_eq!(rating.value(), 2., "a reader sets it at once");
}

#[skene_gui::test]
fn a_disabled_rating_takes_no_focus() {
  let screen = testing::render(page);
  let locked = screen.find_by_role(Role::Slider, "Locked");

  assert!(!locked.is_enabled());
  assert_eq!(locked.value(), 4.);

  screen.find_by_role(Role::Slider, "Rating").press("Tab");
  assert_eq!(
    screen.focused().name(),
    "After",
    "Tab skips the disabled rating"
  );
}
