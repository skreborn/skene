//! A text field's context menu on the real platform, through the driver: the field is text
//! that opens a popup, as GTK reports GtkEntry; a secondary press or `Shift-F10` opens the
//! editing entries of GTK's entry menu, focus on the first; a chosen entry acts on the field
//! and focus returns to it, its selection kept; `Escape` and `Tab` close the menu; a secure
//! field offers neither cut nor copy. An app's own entries run their commands, report their
//! shortcuts and follow their labels and states, and a standard entry's handler runs after its
//! operation.

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

/// A box whose menu holds the app's own entries, "Refresh" counting its activations under
/// Control+R, "Rename" labelled by a signal, "Archive" disabled while one holds, beside two
/// buttons that change them; and a field whose menu's cut records the field's text as its
/// handler runs.
fn commands() -> impl View {
  let refreshed = Signal::new(String::from("0"));
  let title = Signal::new(String::from("Rename"));
  let locked = Signal::new(true);
  let text = Signal::new(String::from("alpha beta"));
  let seen = Signal::new(String::from("unheard"));

  probe("refreshed", refreshed);
  probe("text", text);
  probe("seen", seen);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Button::new("Unlock").handle_activate(move || locked.set(false)))
    .add_child(Button::new("Retitle").handle_activate(move || title.set("Rename all".into())))
    .add_child(
      ContextMenu::new()
        .add_entry(
          MenuEntry::new("Refresh")
            .set_shortcut(Shortcut::primary('r'))
            .handle_activate(move || {
              refreshed
                .update(|count| *count = (count.parse::<u32>().unwrap_or(0) + 1).to_string());
            }),
        )
        .add_entry(MenuEntry::new("Rename").derive_label(move || title.get()))
        .add_entry(MenuEntry::new("Archive").derive_disabled(move || locked.get()))
        .set_child(
          Container::new()
            .set_label("Inbox")
            .set_padding(20)
            .set_child(Text::new("Inbox")),
        ),
    )
    .add_child(
      TextField::new("Draft")
        .set_context_menu(
          ContextMenu::new()
            .add_entry(MenuEntry::cut().handle_activate(move || seen.set(text.get_untracked()))),
        )
        .bind_text(text),
    )
}

#[skene_gui::test]
fn an_apps_menu_entries_run_their_commands_and_follow_their_state() {
  let screen = testing::render(commands);
  let inbox = screen.find_by_role(Role::Region, "Inbox");

  inbox.context_click();

  let refresh = screen.find_by_role(Role::MenuItem, "Refresh");

  assert!(refresh.is_focused(), "focus is on the first entry");
  assert_eq!(
    refresh.shortcut(),
    "<Control>r",
    "an entry's shortcut is its item's key binding"
  );
  assert!(
    !screen.find_by_role(Role::MenuItem, "Archive").is_enabled(),
    "an entry disabled by its state shows disabled"
  );

  refresh.click();
  assert_eq!(
    probe_once(&screen, "refreshed", "1"),
    "1",
    "choosing an entry runs its command"
  );
  assert!(
    screen.query_by_role(Role::MenuItem, "Refresh").is_none(),
    "and closes the menu"
  );

  screen.find_by_role(Role::Button, "Unlock").click();
  screen.find_by_role(Role::Button, "Retitle").click();
  inbox.context_click();
  assert!(
    screen.find_by_role(Role::MenuItem, "Archive").is_enabled(),
    "an entry's state is read as it shows"
  );
  assert!(
    screen.query_by_role(Role::MenuItem, "Rename all").is_some(),
    "and its label"
  );
  screen.press("Escape");
}

#[skene_gui::test]
fn a_standard_entrys_handler_runs_after_its_operation() {
  let screen = testing::render(commands);
  let draft = screen.find_by_role(Role::TextField, "Draft");

  draft.press("ctrl+a").context_click();
  screen.find_by_role(Role::MenuItem, "Cut").click();
  assert_eq!(probe_once(&screen, "text", ""), "", "the cut took the text");
  assert_eq!(
    screen.probe("seen"),
    "",
    "the entry's handler ran after the cut, the field already empty"
  );
}

/// A spacer `spacer` pixels tall, then a field holding "alpha beta".
fn bottom_field(spacer: Signal<i32>) -> impl View {
  let words = Signal::new(String::from("alpha beta"));

  publish("words", move || format!("{:?}", words.get_untracked()));

  Column::new()
    .set_padding(16)
    .add_child(
      Container::new()
        .set_label("Spacer")
        .derive_height(move || spacer.get()),
    )
    .add_child(TextField::new("Bottom").bind_text(words))
}

#[skene_gui::test]
fn a_context_menu_at_the_windows_foot_stays_on_the_screen_and_answers_where_it_shows() {
  let spacer = Signal::new(10_i32);
  let screen = testing::render(move || bottom_field(spacer));
  let (_, screen_height) = screen.driver().screen_extent();
  let unit = screen.driver().scale();
  let unit = if std::env::var("SKENE_BACKEND").as_deref() == Ok("x11") {
    unit
  } else {
    1.
  };
  // The window nearly as tall as the screen, its field at its foot.
  let height = ((screen_height / unit) - 80.).max(400.) as i32;

  screen.set_size(480, height);
  spacer.set(height - 16 - 34 - 24);
  screen.settle();

  let field = screen.find_by_role(Role::TextField, "Bottom");

  field.context_click();

  let menu = screen.find_by_role(Role::Menu, "").read().bounds;
  let (_, window_y, _, window_height) = screen.driver().window_rect();
  // A reader's bounds are the content's: below a title bar Skene draws.
  let header = window_height - f64::from(screen.driver().size().1) * unit;
  let select_all = screen.find_by_role(Role::MenuItem, "Select All");
  let item = select_all.read().bounds;
  let foot = window_y + header + f64::from(menu.y + menu.height) * unit;

  eprintln!(
    "the screen {screen_height} tall, the window at {window_y} {window_height} tall, the menu at {menu:?} (its foot at {foot}), Select All at {item:?}"
  );
  assert!(
    foot <= screen_height + 1.,
    "the menu stays on the screen ({menu:?} in a window at {window_y}, its foot at {foot}, the screen {screen_height} tall)"
  );

  select_all.click();

  let selection = field
    .read()
    .selection
    .expect("the field reports its selection");

  assert_eq!(
    (
      selection.start.min(selection.end),
      selection.start.max(selection.end)
    ),
    (0, 10),
    "a click where the reader reads Select All selects it all"
  );
}
