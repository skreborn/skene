//! Roles as a reader finds them, exactly: AT-SPI 2.52 reports a switch as a check box (as GTK 4
//! reports `GtkSwitch`) and a row and an option as list items, so the realization names the
//! exact role in the `xml-roles` attribute, as browsers name an ARIA role there, and a query
//! for one role never finds another.

use skene_gui::{
  Signal,
  prelude::*,
  testing::{self, Driver as _, Query, Role, Screen},
};

/// A switch, a check box, an activatable row, a picker, a field and an editor.
fn page() -> impl View {
  let wifi = Signal::new(false);
  let remember = Signal::new(false);
  let size = Signal::new("Small");
  let name = Signal::new(String::new());
  let notes = Signal::new(String::new());

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Toggle::new("Wi-Fi").bind_on(wifi))
    .add_child(Checkbox::new("Remember me").bind_checked(remember))
    .add_child(
      Container::new()
        .set_padding(12)
        .set_label("Inbox")
        .handle_click(|| {})
        .set_child(Text::new("Inbox")),
    )
    .add_child(
      Picker::new("Size")
        .bind_chosen(size)
        .set_options(["Small", "Large"].map(|size| (size, size))),
    )
    .add_child(TextField::new("Name").bind_text(name))
    .add_child(TextEditor::new("Notes").bind_text(notes))
}

/// Whether a reader finds an object of `role` named `name` now.
fn finds(screen: &Screen, role: Role, name: &str) -> bool {
  !screen
    .driver()
    .find_now(&Query::Role(role, name.into()))
    .is_empty()
}

#[skene_gui::test]
fn a_query_finds_its_role_alone() {
  let screen = testing::render(page);

  for (role, name, platform_role) in [
    (Role::Switch, "Wi-Fi", "check box"),
    (Role::Checkbox, "Remember me", "check box"),
    (Role::Row, "Inbox", "list item"),
    (Role::TextField, "Name", "text"),
    (Role::TextEditor, "Notes", "text"),
  ] {
    assert_eq!(
      &*screen.find_by_role(role, name).read().platform_role,
      platform_role,
      "{name} is reported as GTK 4 reports its kind"
    );
  }

  assert!(
    !finds(&screen, Role::Checkbox, "Wi-Fi"),
    "a switch is not a check box"
  );
  assert!(
    !finds(&screen, Role::Switch, "Remember me"),
    "nor a check box a switch"
  );
  assert!(
    !finds(&screen, Role::Option, "Inbox"),
    "a row is not an option"
  );
  assert!(
    !finds(&screen, Role::TextEditor, "Name"),
    "a field is not an editor"
  );
  assert!(
    !finds(&screen, Role::TextField, "Notes"),
    "nor an editor a field"
  );

  screen.find_by_role(Role::ComboBox, "Size").click();

  let small = screen.find_by_role(Role::Option, "Small");

  assert_eq!(&*small.read().platform_role, "list item");
  assert!(
    !finds(&screen, Role::Row, "Small"),
    "an option is not a row"
  );
}
