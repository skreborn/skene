//! The Simple library on the real platform.

use skene_gui::{
  prelude::*,
  testing::{self, Role},
};
use skene_simple_gui::prelude::*;

fn buttons() -> impl View {
  Theme::new(SimplePalette::default()).set_child(
    SimpleCard::new().set_child(
      Row::new()
        .set_spacing(SimplePalette::GAP)
        .add_child(SimpleButton::new("Save"))
        .add_child(Button::new("Cancel")),
    ),
  )
}

#[skene_gui::test]
fn the_primary_button_is_drawn_prominent() {
  let screen = testing::render(buttons);
  let fill = |name: &str| {
    let bounds = screen.find_by_role(Role::Button, name).read().bounds;

    screen.pixel(bounds.x + 6., bounds.y + bounds.height / 2.)
  };
  let (save, cancel) = (fill("Save"), fill("Cancel"));

  assert_ne!(save, cancel);
  assert!(
    save.blue > save.red + 60,
    "the primary action is drawn in the accent: {save:?}"
  );
}

fn picker() -> impl View {
  let due = Signal::new("2026-10-14".to_owned());

  Theme::new(SimplePalette::default()).set_child(
    Column::new()
      .set_padding(16)
      .set_spacing(8)
      .add_child(
        SimpleDatePicker::new("Due")
          .derive_date(move || due.get())
          .handle_change(move |date| {
            if SimpleDate::parse(&date).is_some_and(|date| !date.is_weekend()) {
              due.set(date);
            }
          }),
      )
      .add_child(Text::derive(move || format!("Due {}", due.get()))),
  )
}

#[skene_gui::test]
fn the_date_picker_takes_a_weekday_and_refuses_a_weekend() {
  let screen = testing::render(picker);

  screen.find_by_role(Role::Region, "Due");
  screen.find_by_text("Due 2026-10-14");
  screen.find_by_role(Role::Button, "20").click();
  screen.find_by_text("Due 2026-10-20");
  screen.find_by_role(Role::Button, "17").click();
  screen.find_by_text("Due 2026-10-20");
}

#[skene_gui::test]
fn the_calendar_arrows_are_named() {
  let screen = testing::render(picker);

  screen.find_by_text("October 2026");
  screen.find_by_role(Role::Button, "Next month").click();
  screen.find_by_text("November 2026");
  assert!(screen.audit().is_empty(), "{:#?}", screen.audit());
}
