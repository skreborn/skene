//! A text field's context menu on the real platform, through the driver: the field is text
//! that opens a popup, as GTK reports GtkEntry; a secondary press or `Shift-F10` opens the
//! editing entries of GTK's entry menu, focus on the first; a chosen entry acts on the field
//! and focus returns to it, its selection kept; `Escape` and `Tab` close the menu; a secure
//! field offers neither cut nor copy.

use std::time::{Duration, Instant};

use skene_gui::{
  Signal,
  prelude::*,
  testing::{self, Driver as _, Role, Screen, publish},
};

/// Publishes `signal` as the probe `name`.
fn probe(name: &'static str, signal: Signal<String>) {
  publish(name, move || signal.get_untracked());
}

/// Three fields: "Name" holding "alpha beta", "Other" empty, and a secure "Password".
fn page() -> impl View {
  let name = Signal::new(String::from("alpha beta"));
  let other = Signal::new(String::new());
  let secret = Signal::new(String::from("hunter2"));

  probe("name", name);
  probe("other", other);
  probe("secret", secret);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Name").bind_text(name))
    .add_child(TextField::new("Other").bind_text(other))
    .add_child(
      TextField::new("Password")
        .set_secure(true)
        .bind_text(secret),
    )
}

/// Turns the window's loop until the probe `name` reads `wanted`, for up to two seconds; what
/// it reads then.
fn probe_once(screen: &Screen, name: &str, wanted: &str) -> String {
  let start = Instant::now();

  loop {
    let read = screen.probe(name);

    if read == wanted || start.elapsed() > Duration::from_secs(2) {
      return read;
    }

    screen.driver().wait(Duration::from_millis(20));
  }
}

/// The labels of the menu items a reader finds once the menu shows its first.
fn items(screen: &Screen) -> Vec<String> {
  screen.find_by_role(Role::MenuItem, "Cut");

  ["Cut", "Copy", "Paste", "Delete", "Select All"]
    .into_iter()
    .filter(|label| screen.query_by_role(Role::MenuItem, label).is_some())
    .map(str::to_owned)
    .collect()
}

#[skene_gui::test]
fn a_text_field_is_text_that_opens_a_menu() {
  let screen = testing::render(page);

  for name in ["Name", "Password"] {
    let read = screen.find_by_role(Role::TextField, name).read();

    assert_eq!(
      &*read.platform_role,
      if name == "Name" {
        "text"
      } else {
        "password text"
      },
      "{name} is reported as GTK 4 reports its entry"
    );
    assert!(read.has_popup, "{name} opens a popup: its context menu");
  }
}

#[skene_gui::test]
fn a_text_fields_menu_cuts_copies_and_pastes_through_the_clipboard() {
  let screen = testing::render(page);
  let name = screen.find_by_role(Role::TextField, "Name");

  name.press("ctrl+a").context_click();
  assert_eq!(
    items(&screen),
    ["Cut", "Copy", "Paste", "Delete", "Select All"],
    "a secondary press opens the editing entries of GTK's entry menu"
  );
  assert_eq!(
    &*screen.focused().name(),
    "Cut",
    "focus is on the first of them"
  );

  screen.find_by_role(Role::MenuItem, "Copy").click();
  assert!(
    screen.query_by_role(Role::MenuItem, "Copy").is_none(),
    "choosing an entry closes the menu"
  );
  assert!(name.is_focused(), "and focus returns to the field");

  let selection = name
    .read()
    .selection
    .expect("the field reports its selection");

  assert_eq!(
    (
      selection.start.min(selection.end),
      selection.start.max(selection.end)
    ),
    (0, 10),
    "whose selection the menu kept"
  );

  let other = screen.find_by_role(Role::TextField, "Other");

  other.context_click();
  screen.find_by_role(Role::MenuItem, "Paste").click();
  assert_eq!(
    probe_once(&screen, "other", "alpha beta"),
    "alpha beta",
    "paste takes what copy put on the clipboard"
  );

  name.press("ctrl+a").context_click();
  screen.find_by_role(Role::MenuItem, "Cut").click();
  assert_eq!(
    probe_once(&screen, "name", ""),
    "",
    "cut removes the selection"
  );
}

#[skene_gui::test]
fn the_menu_key_opens_the_menu_and_escape_or_tab_closes_it() {
  let screen = testing::render(page);
  let other = screen.find_by_role(Role::TextField, "Other");

  for close in ["Escape", "Tab"] {
    other.press("shift+F10");
    assert_eq!(
      items(&screen).len(),
      5,
      "Shift-F10 opens the menu at the focus"
    );
    screen.press(close);
    assert!(
      screen.query_by_role(Role::MenuItem, "Cut").is_none(),
      "{close} closes it"
    );
    assert!(other.is_focused(), "focus back on the field after {close}");
  }
}

#[skene_gui::test]
fn a_secure_fields_menu_offers_neither_cut_nor_copy() {
  let screen = testing::render(page);
  let password = screen.find_by_role(Role::TextField, "Password");

  password.press("ctrl+a").context_click();

  for (label, enabled) in [
    ("Cut", false),
    ("Copy", false),
    ("Paste", true),
    ("Delete", true),
    ("Select All", true),
  ] {
    assert_eq!(
      screen.find_by_role(Role::MenuItem, label).is_enabled(),
      enabled,
      "{label} in a secure field"
    );
  }

  screen.press("Escape");
  assert_eq!(probe_once(&screen, "secret", "hunter2"), "hunter2");
}
