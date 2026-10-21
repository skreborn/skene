//! Focus as a reader follows it: a focused control removed with its row, a picker's focused
//! option removed or replaced while its list is open, and Escape, which a field keeps and an
//! open popup closes on.

use skene_gui::{
  prelude::*,
  testing::{self, Role},
};

use crate::{focused_name, probe_until};

/// Rows "Item 0" to "Item 4", each with a button "Delete {n}" that deletes its own row.
fn todo() -> impl View {
  let items = Signal::new((0..5).collect::<Vec<usize>>());

  testing::publish("items", move || format!("{:?}", items.get_untracked()));

  Column::new()
    .set_padding(16)
    .set_spacing(8)
    .derive_children(
      move || items.get(),
      |item| *item,
      move |item: usize| {
        Row::new()
          .set_spacing(8)
          .add_child(Text::new(format!("Item {item}")))
          .add_child(
            Button::new(format!("Delete {item}"))
              .handle_activate(move || items.update(|items| items.retain(|other| *other != item))),
          )
      },
    )
}

#[skene_gui::test]
fn a_button_that_deletes_its_own_row_by_a_key_leaves_tab_going_on_from_there() {
  let screen = testing::render(todo);

  for _ in 0..3 {
    screen.press("Tab");
  }

  assert_eq!(focused_name(&screen), "Delete 2");
  screen.press("Enter");
  assert_eq!(
    probe_until(&screen, "items", "[0, 1, 3, 4]"),
    "[0, 1, 3, 4]"
  );
  assert_eq!(
    focused_name(&screen),
    "<none>",
    "focus leaves with the removed button, as Chromium's focus fixup takes it"
  );

  // Tab goes on from where the removed button was.
  screen.press("Tab");
  assert_eq!(focused_name(&screen), "Delete 3");
}

#[skene_gui::test]
fn a_button_that_deletes_its_own_row_by_a_click_leaves_the_window_working() {
  let screen = testing::render(todo);

  screen.find_by_role(Role::Button, "Delete 2").click();
  assert_eq!(
    probe_until(&screen, "items", "[0, 1, 3, 4]"),
    "[0, 1, 3, 4]"
  );
  screen.find_by_role(Role::Button, "Delete 3").click();
  assert_eq!(probe_until(&screen, "items", "[0, 1, 4]"), "[0, 1, 4]");
}

/// Strings of `items`.
fn strings(items: &[&str]) -> Vec<String> {
  items.iter().map(|item| (*item).to_owned()).collect()
}

/// A picker over a derived list of fruits, "Banana" chosen; Control+D drops "Banana" from the
/// list, Control+R replaces every fruit by vegetables.
fn shrinking() -> impl View {
  let order = Signal::new(strings(&["Apple", "Banana", "Cherry", "Date"]));
  let fruit = Signal::new("Banana".to_owned());

  testing::publish("order", move || format!("{:?}", order.get_untracked()));
  testing::publish("fruit", move || fruit.get_untracked());

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Button::new("Drop")
        .set_shortcut(Shortcut::primary('d'))
        .handle_activate(move || order.update(|order| order.retain(|item| item != "Banana"))),
    )
    .add_child(
      Button::new("Replace")
        .set_shortcut(Shortcut::primary('r'))
        .handle_activate(move || order.set(strings(&["Kale", "Leek", "Okra"]))),
    )
    .add_child(
      Row::new().add_child(
        Picker::new("Fruit")
          .bind_chosen(fruit)
          .derive_options(move || order.get(), Clone::clone),
      ),
    )
}

#[skene_gui::test]
fn a_picker_whose_focused_option_goes_while_open_keeps_its_keys() {
  let screen = testing::render(shrinking);
  let face = screen.find_by_role(Role::ComboBox, "Fruit");

  face.click();
  assert_eq!(face.read().expanded, Some(true));
  assert_eq!(
    focused_name(&screen),
    "Banana",
    "focus opens on the chosen option"
  );

  screen.press("ctrl+d");
  assert_eq!(
    probe_until(&screen, "order", r#"["Apple", "Cherry", "Date"]"#),
    r#"["Apple", "Cherry", "Date"]"#
  );

  // Focus stays in the open list, and its keys still work.
  screen.press("Escape");
  assert_eq!(face.read().expanded, Some(false), "Escape closes the list");
  assert_eq!(focused_name(&screen), "Fruit", "focus is back on the face");
}

#[skene_gui::test]
fn a_picker_whose_options_are_all_replaced_while_open_chooses_from_the_new_ones() {
  let screen = testing::render(shrinking);

  screen.find_by_role(Role::ComboBox, "Fruit").click();
  assert_eq!(focused_name(&screen), "Banana");
  screen.press("ctrl+r");
  assert_eq!(
    probe_until(&screen, "order", r#"["Kale", "Leek", "Okra"]"#),
    r#"["Kale", "Leek", "Okra"]"#
  );
  screen.press("Down");
  screen.press("Enter");

  let chosen = probe_until(&screen, "fruit", "Kale");

  assert!(
    ["Kale", "Leek", "Okra"].contains(&chosen.as_str()),
    "the arrows and Enter choose among the new options: {chosen:?}"
  );
}

#[skene_gui::test]
fn escape_in_a_field_keeps_its_focus() {
  let text = Signal::new(String::from("hello"));
  let screen = testing::render(move || {
    Column::new()
      .set_padding(16)
      .add_child(TextField::new("Name").bind_text(text))
  });

  screen.press("Tab");
  assert_eq!(focused_name(&screen), "Name");
  screen.press("Escape");
  assert_eq!(
    focused_name(&screen),
    "Name",
    "Escape in a text field leaves its focus where it is, as in GTK and Chromium"
  );
}

/// A button "Menu" opening a popup holding a button "Inside" that asks for focus.
fn dismissable() -> impl View {
  let open = Signal::new(false);
  let anchor = NodeHandle::new();

  testing::publish("open", move || open.get_untracked().to_string());

  Column::new()
    .set_padding(16)
    .add_child(
      Button::new("Menu")
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
            .set_padding(8)
            .set_background(Color::WHITE)
            .set_child(Button::new("Inside").set_autofocus(true)),
        ),
    )
}

#[skene_gui::test]
fn escape_dismisses_an_open_popup_and_focus_returns_to_its_opener() {
  let screen = testing::render(dismissable);

  screen.press("Tab");
  screen.press("Enter");
  assert_eq!(probe_until(&screen, "open", "true"), "true");
  assert_eq!(focused_name(&screen), "Inside");
  screen.press("Escape");
  assert_eq!(
    probe_until(&screen, "open", "false"),
    "false",
    "Escape no handler takes dismisses the topmost open popup"
  );
  assert_eq!(
    focused_name(&screen),
    "Menu",
    "focus returns to what opened it"
  );
}
