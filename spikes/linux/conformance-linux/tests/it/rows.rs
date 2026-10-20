//! Activatable rows on the real platform, through the driver: a row restyles under the pointer
//! and its press and acts on a click beside the switch it holds, which owns its own press;
//! `Tab` walks the rows in the scene's order, wraps, and shows where focus is; a row answers
//! keys as a button does.

use skene_gui::{
  Color, State, Value,
  prelude::*,
  testing::{self, Role, Screen, publish},
};

/// A row at rest.
const REST: Color = Color::rgb(0xffffff);

/// A row under the pointer.
const HOVER: Color = Color::rgb(0xdbeafe);

/// A row pressed.
const PRESSED: Color = Color::rgb(0x93c5fd);

/// An activatable row named `name`, 60 pixels tall, its background following the pointer and
/// its press, counting its activations into `count`, holding `child`.
fn row(name: &str, count: Signal<u32>, child: impl View) -> Container {
  Container::new()
    .set_label(name)
    .set_height(60)
    .set_padding(12)
    .set_background(
      Value::from(REST)
        .when(State::Hovered, HOVER)
        .when(State::Pressed, PRESSED),
    )
    .handle_click(move || count.update(|count| *count += 1))
    .set_child(child)
}

/// The row's color as the screen shows it, inside its leading edge.
fn row_color(screen: &Screen, name: &str) -> Color {
  let bounds = screen.find_by_role(Role::Row, name).read().bounds;

  screen.pixel(bounds.x + 4., bounds.y + 4.)
}

/// One row, "Inbox", holding a switch, "Mute Inbox".
fn inbox() -> impl View {
  let opened = Signal::new(0_u32);
  let muted = Signal::new(false);

  publish("opened", move || opened.get_untracked().to_string());
  publish("muted", move || muted.get_untracked().to_string());

  Column::new().set_padding(16).add_child(row(
    "Inbox",
    opened,
    Row::new()
      .set_spacing(12)
      .set_item_alignment(AxisAlignment::Center)
      .add_child(
        Flexible::new()
          .set_flexibility(1)
          .set_child(Text::new("Inbox")),
      )
      .add_child(
        Toggle::new("Muted")
          .set_accessible_name("Mute Inbox")
          .bind_on(muted),
      ),
  ))
}

#[skene_gui::test]
fn a_row_restyles_under_the_pointer_and_its_press_and_acts_on_a_click_beside_its_switch() {
  let screen = testing::render(inbox);
  let inbox = screen.find_by_role(Role::Row, "Inbox");

  assert_eq!(row_color(&screen, "Inbox"), REST);

  inbox.hover();
  assert_eq!(row_color(&screen, "Inbox"), HOVER, "the pointer over it");

  let at = screen.driver().point_in(inbox.element(), 0.5, 0.5);

  screen.driver().hold_button(at);
  assert_eq!(
    row_color(&screen, "Inbox"),
    PRESSED,
    "the button held on it"
  );
  assert_eq!(screen.probe("opened"), "0", "a press alone acts on nothing");

  screen.driver().release_button();
  assert_eq!(screen.probe("opened"), "1", "the click acts on the row");
  assert_eq!(row_color(&screen, "Inbox"), HOVER);

  let switch = screen.find_by_role(Role::Switch, "Mute Inbox");

  switch.click();
  assert_eq!(screen.probe("muted"), "true", "the switch owns its press");
  assert_eq!(screen.probe("opened"), "1", "the row does not act");

  let (width, height) = screen.driver().screen_extent();

  screen.driver().move_pointer((width - 2., height - 2.));
  assert_eq!(
    row_color(&screen, "Inbox"),
    REST,
    "the pointer gone, at rest"
  );
}

/// A field, "Before", and three rows, "One", "Two" and "Three", each publishing the name of the
/// row last activated as "activated".
fn three() -> impl View {
  let before = Signal::new(String::new());
  let activated = Signal::new(String::from("none"));

  publish("activated", move || activated.get_untracked());

  ["One", "Two", "Three"].into_iter().fold(
    Column::new()
      .set_padding(16)
      .set_spacing(12)
      .add_child(TextField::new("Before").bind_text(before)),
    |column, name| {
      let count = Signal::new(0_u32);

      Effect::new(move || {
        if count.get() > 0 {
          activated.set(name.to_owned());
        }
      });

      column.add_child(row(name, count, Text::new(name)))
    },
  )
}

#[skene_gui::test]
fn tab_walks_the_rows_in_the_scenes_order_wraps_and_shows_focus_and_enter_acts() {
  let screen = testing::render(three);
  let ring = |screen: &Screen, name: &str| {
    let bounds = screen.find_by_role(Role::Row, name).read().bounds;

    screen.pixel(bounds.x - 1., bounds.y + bounds.height / 2.)
  };
  let unfocused = ring(&screen, "One");

  screen.find_by_role(Role::TextField, "Before").click();

  let mut order = Vec::new();

  for _ in 0..4 {
    screen.press("Tab");
    order.push(screen.focused().name());
  }

  assert_eq!(
    order,
    ["One", "Two", "Three", "Before"],
    "the scene's order, then around"
  );

  screen.press("Tab");
  assert_eq!(screen.focused().name(), "One");
  assert_ne!(
    ring(&screen, "One"),
    unfocused,
    "a ring shows where focus is after a key"
  );

  screen.press("Enter");
  assert_eq!(screen.probe("activated"), "One", "Enter acts on the row");
}

/// A field, "Before", and one row, "Inbox", counting its activations as "opened".
fn keyed() -> impl View {
  let before = Signal::new(String::new());
  let opened = Signal::new(0_u32);

  publish("opened", move || opened.get_untracked().to_string());

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Before").bind_text(before))
    .add_child(row("Inbox", opened, Text::new("Inbox")))
}

#[skene_gui::test]
fn a_row_answers_keys_as_a_button_does() {
  let screen = testing::render(keyed);

  screen.find_by_role(Role::TextField, "Before").click();

  // Space goes down in the field, focus moves to the row while it is held, and it comes up
  // on the row: a release whose press was not the row's acts on nothing.
  screen.driver().hold_key("space");
  screen.press("Tab");
  assert_eq!(screen.focused().name(), "Inbox");
  screen.driver().release_key("space");
  assert_eq!(
    screen.probe("opened"),
    "0",
    "a release alone acts on nothing"
  );

  screen.press("ctrl+Enter");
  assert_eq!(
    screen.probe("opened"),
    "0",
    "Control-Enter is not the row's"
  );

  screen.driver().hold_key("space");
  assert_eq!(
    row_color(&screen, "Inbox"),
    PRESSED,
    "Space held shows the row pressed"
  );
  assert_eq!(screen.probe("opened"), "0", "Space acts on its release");
  screen.press("Escape");
  screen.driver().release_key("space");
  assert_eq!(screen.probe("opened"), "0", "Escape cancels a held Space");
  assert_eq!(
    screen.focused().name(),
    "Inbox",
    "and keeps focus where it is"
  );

  screen.press("space");
  assert_eq!(screen.probe("opened"), "1", "Space acts on its release");

  screen.driver().hold_key("Enter");
  assert_eq!(screen.probe("opened"), "2", "Enter acts on its press");
  screen.driver().release_key("Enter");
  assert_eq!(screen.probe("opened"), "2", "and not again on its release");
}
