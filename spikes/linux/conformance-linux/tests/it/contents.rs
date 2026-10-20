//! What a view says beyond its text, on the real platform, through the driver: a field's
//! placeholder and its being read-only reach a reader, and a read-only field takes no typing;
//! a field's keyboard reaches the input method as its purpose, on every protocol a window
//! speaks where the leg serves one; a text that reserves others' widths lays out as wide as
//! the widest; a picker's list opens below its face at least as wide, and a context menu at
//! the pointer.

use std::time::{Duration, Instant};

use skene_gui::{
  Signal,
  prelude::*,
  testing::{self, Driver as _, Role, publish},
};
use skene_gui_linux::{
  Backend,
  testing::{
    Session,
    input_method::{InputMethod, Purpose},
  },
};

/// A field, "Name", whose placeholder is "Ada", and a read-only one, "Code", holding "ABC".
fn fields() -> impl View {
  let name = Signal::new(String::new());
  let code = Signal::new(String::from("ABC"));

  publish("code", move || code.get_untracked());

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      TextField::new("Name")
        .set_placeholder("Ada")
        .bind_text(name),
    )
    .add_child(TextField::new("Code").set_read_only(true).bind_text(code))
}

#[skene_gui::test]
fn a_placeholder_and_read_only_reach_a_reader_and_a_read_only_field_takes_no_typing() {
  let screen = testing::render(fields);
  let name = screen.find_by_role(Role::TextField, "Name");
  let code = screen.find_by_role(Role::TextField, "Code");
  let attributes = screen.driver().platform_attributes(name.element());

  assert_eq!(
    attributes.get("placeholder-text").map(String::as_str),
    Some("Ada"),
    "the placeholder: {attributes:?}"
  );
  assert_eq!(name.text(), "", "a placeholder is no text");

  let states = screen.driver().platform_states(name.element());

  assert!(
    states.contains(&"editable") && !states.contains(&"read-only"),
    "a plain field is editable: {states:?}"
  );

  let states = screen.driver().platform_states(code.element());

  assert!(
    states.contains(&"read-only") && !states.contains(&"editable"),
    "a read-only field says so: {states:?}"
  );

  code.click().type_text("x");
  assert_eq!(
    screen.probe("code"),
    "ABC",
    "a read-only field takes no typing"
  );
  assert!(code.is_focused(), "and still takes focus");
}

/// Three fields, "Email", "Number" and "Plain", the first two with their keyboards.
fn keyboards() -> impl View {
  let field = |label: &str, keyboard: Keyboard| {
    TextField::new(label)
      .set_keyboard(keyboard)
      .bind_text(Signal::new(String::new()))
  };

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(field("Email", Keyboard::Email))
    .add_child(field("Number", Keyboard::Number))
    .add_child(field("Plain", Keyboard::Text))
}

/// Focuses each field with a click and holds what `input_method` was told it is for.
fn tells_each_purpose(input_method: &InputMethod) {
  let screen = testing::render(keyboards);

  for (label, purpose) in [
    ("Email", Purpose::Email),
    ("Number", Purpose::Number),
    ("Plain", Purpose::Normal),
  ] {
    screen.find_by_role(Role::TextField, label).click();

    let started = Instant::now();

    while input_method.told().purpose != purpose && started.elapsed() < Duration::from_secs(2) {
      screen.driver().wait(Duration::from_millis(20));
    }

    assert_eq!(
      input_method.told().purpose,
      purpose,
      "{label}'s keyboard reaches the input method"
    );
  }
}

#[skene_gui::test]
fn a_fields_keyboard_reaches_fcitx_as_its_purpose() {
  if Session::current().map(Session::backend) != Some(Backend::X11) {
    skene_gui_linux::testing::not_tested_here("on Wayland the input method is the compositor's");

    return;
  }

  tells_each_purpose(&InputMethod::serve().expect("the Fcitx5 stand-in takes its name"));
}

#[skene_gui::test]
fn a_fields_keyboard_reaches_ibus_as_its_purpose() {
  if Session::current().map(Session::backend) != Some(Backend::X11) {
    skene_gui_linux::testing::not_tested_here("on Wayland the input method is the compositor's");

    return;
  }

  tells_each_purpose(&InputMethod::serve_ibus().expect("the IBus stand-in takes its name"));
}

#[skene_gui::test]
fn a_fields_keyboard_reaches_the_compositors_input_method_as_its_purpose() {
  if Session::current() != Some(Session::Sway) {
    skene_gui_linux::testing::not_tested_here(
      "of the legs, only sway lets a client be the seat's input method (zwp_input_method_v2)",
    );

    return;
  }

  tells_each_purpose(&InputMethod::serve_wayland().expect("sway takes an input method"));
}

/// A short text that reserves a long one's width, above the long one, each in a row of its
/// own so neither stretches.
fn reserved() -> impl View {
  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Row::new().add_child(Text::new("Go").set_reserved_texts(["Going somewhere far"])))
    .add_child(Row::new().add_child(Text::new("Going somewhere far")))
    .add_child(Row::new().add_child(Text::new("Go!")))
}

#[skene_gui::test]
fn a_text_that_reserves_others_is_as_wide_as_the_widest() {
  let screen = testing::render(reserved);
  let width = |text: &str| screen.find_by_text(text).read().bounds.width;
  let long = width("Going somewhere far");
  let reserving = width("Go");
  let short = width("Go!");

  eprintln!("reserving {reserving}, long {long}, short {short}");
  assert!(short < long / 2., "the short text alone is short");
  assert!(
    (reserving - long).abs() <= 1.,
    "the reserving text is as wide as the long one"
  );
}

/// A picker, "Fruit", as wide as the window's content, its options far narrower, and a field,
/// "Note", holding "some text".
fn placed() -> impl View {
  let fruit = Signal::new("Fig");
  let note = Signal::new(String::from("some text"));

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .set_item_alignment(AxisAlignment::Stretch)
    .add_child(
      Picker::new("Fruit")
        .bind_chosen(fruit)
        .set_options(["Fig", "Kiwi", "Lime"].map(|fruit| (fruit, fruit))),
    )
    .add_child(TextField::new("Note").bind_text(note))
}

/// Whether `actual` is `wanted` within `within` pixels: the border, padding and gap a popup's
/// style puts between where it opens and the items a reader reads in it.
fn near(actual: f32, wanted: f32, within: f32) -> bool {
  (actual - wanted).abs() <= within
}

#[skene_gui::test]
fn a_pickers_list_opens_below_its_face_and_a_context_menu_at_the_pointer() {
  let screen = testing::render(placed);
  let face = screen.find_by_role(Role::ComboBox, "Fruit");
  let face_bounds = face.read().bounds;

  face.click();

  // The list, inside the popup's border and padding.
  let list = screen.find_by_role(Role::ListBox, "").read().bounds;

  eprintln!("face {face_bounds:?}, list {list:?}");
  assert!(
    list.y >= face_bounds.y + face_bounds.height
      && near(list.y, face_bounds.y + face_bounds.height, 12.),
    "the list opens just below its face"
  );
  assert!(
    list.x >= face_bounds.x
      && near(list.x, face_bounds.x, 8.)
      && near(list.width, face_bounds.width, 16.),
    "from its leading edge, as wide as it rather than as its options"
  );

  screen.press("Escape");

  let note = screen.find_by_role(Role::TextField, "Note");
  let at = note.read().bounds;

  note.context_click();

  let menu = screen.find_by_role(Role::Menu, "").read().bounds;
  let (x, y) = (at.x + at.width / 2., at.y + at.height / 2.);

  eprintln!("pointer at {x}, {y}; menu {menu:?}");
  assert!(
    menu.x >= x && near(menu.x, x, 8.) && menu.y >= y && near(menu.y, y, 8.),
    "the menu opens at the pointer, its items inside its border and padding"
  );
}
