//! Hits as the pointer finds them: a part moved at run time takes a press where it shows, the
//! pointer resting where it was, and not where a clipping box cut it off.

use skene_gui::{
  BasicButton,
  behavior::Activate,
  prelude::*,
  testing::{self, Role, Screen},
};

use crate::probe_until;

/// A box 100 by 40, clipping where `clip`, holding a command "Moved" whose mark (40 by 20) a
/// signal moves 160 right of where it starts, past the box's edge.
fn moving(moved: Signal<bool>, clip: bool) -> impl View {
  let presses = Signal::new(0);

  testing::publish("presses", move || presses.get_untracked().to_string());

  Column::new().set_padding(16).add_child(
    Container::new()
      .set_label("Box")
      .set_width(100)
      .set_height(40)
      .set_clip(clip)
      .set_child(
        BasicButton::new(
          Activate::new().handle_activate(move || presses.update(|presses| *presses += 1)),
        )
        .set_height(20)
        .set_spacing(8)
        .add_part(
          Part::Mark,
          Container::new()
            .set_width(40)
            .set_height(20)
            .set_background(Color::rgb(0xc0_00_00))
            .derive_offset(move || {
              if moved.get() {
                Offset::new(160, 0)
              } else {
                Offset::ZERO
              }
            }),
        )
        .set_label("Moved"),
      ),
  )
}

/// Clicks `x`, `y` pixels from the top leading corner of the region "Box".
fn click_in_box(screen: &Screen, x: f32, y: f32) {
  let region = screen.find_by_role(Role::Region, "Box");
  let bounds = region.read().bounds;
  let point = screen.driver().point_in(
    region.element(),
    f64::from(x / bounds.width),
    f64::from(y / bounds.height),
  );

  screen.driver().click_point(point);
}

#[skene_gui::test]
fn a_mark_moved_under_a_resting_pointer_takes_a_press_where_it_shows() {
  let moved = Signal::new(false);
  let screen = testing::render(move || moving(moved, false));

  click_in_box(&screen, 20., 10.);
  assert_eq!(
    probe_until(&screen, "presses", "1"),
    "1",
    "a press on the mark where it starts"
  );

  moved.set(true);
  screen.settle();
  click_in_box(&screen, 180., 10.);
  assert_eq!(
    probe_until(&screen, "presses", "2"),
    "2",
    "a press where the moved mark shows reaches its command"
  );

  // Moved back under the resting pointer, the press there reaches nothing.
  moved.set(false);
  screen.settle();
  click_in_box(&screen, 180., 10.);
  screen.settle();
  assert_eq!(
    screen.probe("presses"),
    "2",
    "nothing shows where the mark was"
  );
}

#[skene_gui::test]
fn a_mark_moved_past_a_clipping_box_takes_no_press() {
  let moved = Signal::new(false);
  let screen = testing::render(move || moving(moved, true));

  moved.set(true);
  screen.settle();
  click_in_box(&screen, 180., 10.);
  screen.settle();
  assert_eq!(
    screen.probe("presses"),
    "0",
    "a press where the clipping box cut the moved mark off reaches nothing"
  );
}
