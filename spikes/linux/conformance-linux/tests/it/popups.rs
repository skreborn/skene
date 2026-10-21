//! A popup inside a popup: a picker in an overlay opens its list over the overlay, chooses by
//! a click and by its keys, and leaves the overlay open.

use skene_gui::{
  prelude::*,
  testing::{self, Role},
};

use crate::probe_until;

/// A button "Settings" opening a popup that holds a picker "Size" of three sizes.
fn popover() -> impl View {
  let open = Signal::new(false);
  let size = Signal::new("Small".to_owned());
  let anchor = NodeHandle::new();

  testing::publish("size", move || size.get_untracked());
  testing::publish("open", move || open.get_untracked().to_string());

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Button::new("Settings")
        .set_handle(&anchor)
        .handle_activate(move || open.set(true)),
    )
    .add_child(
      Overlay::popup()
        .set_anchor(&anchor)
        .bind_open(open)
        .handle_dismiss(move || open.set(false))
        .set_child(
          Container::new()
            .set_padding(12)
            .set_background(Color::WHITE)
            .set_child(
              Column::new()
                .set_spacing(8)
                .add_child(Text::new("Options"))
                .add_child(Row::new().add_child(
                  Picker::new("Size").bind_chosen(size).set_options([
                    ("Small".to_owned(), "Small"),
                    ("Medium".to_owned(), "Medium"),
                    ("Large".to_owned(), "Large"),
                  ]),
                )),
            ),
        ),
    )
}

#[skene_gui::test]
fn a_picker_inside_a_popup_opens_its_list_and_chooses_by_a_click() {
  let screen = testing::render(popover);

  screen.find_by_role(Role::Button, "Settings").click();
  assert_eq!(probe_until(&screen, "open", "true"), "true");

  let face = screen.find_by_role(Role::ComboBox, "Size");

  face.click();
  assert_eq!(
    face.read().expanded,
    Some(true),
    "the picker inside the popup opens"
  );
  screen.find_by_role(Role::Option, "Large").click();
  assert_eq!(
    probe_until(&screen, "size", "Large"),
    "Large",
    "a click on an option of the list over the popup chooses it"
  );
  assert_eq!(
    screen.probe("open"),
    "true",
    "the popup holding it stays open"
  );
}

#[skene_gui::test]
fn a_picker_inside_a_popup_chooses_by_its_keys() {
  let screen = testing::render(popover);

  screen.find_by_role(Role::Button, "Settings").click();
  screen.find_by_role(Role::ComboBox, "Size").click();
  screen.press("Down");
  screen.press("Down");
  screen.press("Enter");
  assert_eq!(probe_until(&screen, "size", "Large"), "Large");
  assert_eq!(screen.probe("open"), "true");
}
