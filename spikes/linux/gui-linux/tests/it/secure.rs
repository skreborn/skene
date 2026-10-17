//! A secure field on the real platform, through the driver: an input method is told its text
//! as one mark per cluster, its caret and anchor among the marks, on every protocol a window
//! speaks (Fcitx5 and IBus over D-Bus on X11, text-input-v3 behind sway), and what the input
//! method commits or deletes lands by those marks; no trace line shows its text; a word of it
//! is its whole text.

use std::time::{Duration, Instant};

use skene_gui::{
  Signal,
  prelude::*,
  testing::{self, Driver as _, Role, Screen, publish},
};
use skene_gui_linux::{
  Backend,
  testing::{
    Session,
    input_method::{InputMethod, Purpose, Told},
  },
  trace,
};

/// What the tests type into the secure field: no three of its characters in a row, nor one
/// of them quoted, occur in a trace line of anything else.
const SECRET: &str = "qZ7jVw";

/// Publishes `signal` as the probe `name`.
fn probe(name: &'static str, signal: Signal<String>) {
  publish(name, move || signal.get_untracked());
}

/// A field, "Name", and a secure one, "Password".
fn page() -> impl View {
  let name = Signal::new(String::new());
  let secret = Signal::new(String::new());

  probe("name", name);
  probe("secret", secret);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Name").bind_text(name))
    .add_child(
      TextField::new("Password")
        .set_secure(true)
        .bind_text(secret),
    )
}

/// Turns the window's loop until `holds` holds of what `input_method` was told, for up to two
/// seconds; what it was told then.
fn told_once(screen: &Screen, input_method: &InputMethod, holds: impl Fn(&Told) -> bool) -> Told {
  let start = Instant::now();

  loop {
    let told = input_method.told();

    if holds(&told) || start.elapsed() > Duration::from_secs(2) {
      return told;
    }

    screen.driver().wait(Duration::from_millis(20));
  }
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

/// Whether `line` shows any of `SECRET`: three of its characters in a row, or one of them
/// quoted.
fn shows_the_secret(line: &str) -> bool {
  let characters: Vec<char> = SECRET.chars().collect();

  characters
    .windows(3)
    .any(|run| line.contains(&run.iter().collect::<String>()))
    || characters.iter().any(|character| {
      line.contains(&format!("\"{character}\"")) || line.contains(&format!("'{character}'"))
    })
}

/// Types into both fields with `input_method` serving, and holds what it was told: the plain
/// field's text as it is, the secure field's as marks, the caret among them, what the input
/// method commits and deletes landing by them, and no trace line showing the secret.
fn tells_only_marks(input_method: &InputMethod) {
  let screen = testing::render(page);

  trace::set_recording(true);

  // The plain field's text reaches the input method as it is.
  screen
    .find_by_role(Role::TextField, "Name")
    .type_text("abc");

  let told = told_once(&screen, input_method, |told| {
    told
      .surrounding
      .as_ref()
      .is_some_and(|(text, ..)| text == "abc")
  });

  eprintln!("told for the plain field: {told:?}");
  assert!(told.focused, "the field took the input method's focus");
  assert_eq!(told.surrounding, Some(("abc".to_owned(), 3, 3)));
  assert_eq!(
    told.purpose,
    Purpose::Normal,
    "a plain field is no password"
  );

  let plain = trace::take_recorded();

  assert!(
    plain
      .iter()
      .any(|line| line.contains("\"a\"") || line.contains("abc")),
    "the trace shows a plain field's text, so its absence below means something: {plain:#?}"
  );

  // The secure field's reaches it as marks.
  let password = screen.find_by_role(Role::TextField, "Password");

  password.type_text(SECRET);
  assert_eq!(probe_once(&screen, "secret", SECRET), SECRET);

  let told = told_once(&screen, input_method, |told| {
    told.purpose == Purpose::Password
      && told
        .surrounding
        .as_ref()
        .is_some_and(|(text, ..)| text == "••••••")
  });

  eprintln!("told for the secure field: {told:?}");
  assert!(told.focused);
  assert_eq!(
    told.purpose,
    Purpose::Password,
    "the input method was told the field holds a password"
  );
  assert_eq!(
    told.surrounding,
    Some(("••••••".to_owned(), 6, 6)),
    "six marks, the caret after the sixth"
  );

  // What the input method commits and deletes lands by the marks it was told.
  input_method
    .trigger_commit("x")
    .expect("the input method commits");
  assert_eq!(probe_once(&screen, "secret", "qZ7jVwx"), "qZ7jVwx");

  let told = told_once(&screen, input_method, |told| {
    told
      .surrounding
      .as_ref()
      .is_some_and(|(text, ..)| text == "•••••••")
  });

  assert_eq!(told.surrounding, Some(("•••••••".to_owned(), 7, 7)));
  input_method
    .trigger_delete_before(2)
    .expect("the input method deletes");
  assert_eq!(
    probe_once(&screen, "secret", "qZ7jV"),
    "qZ7jV",
    "two marks before the caret are the two clusters before it"
  );

  let told = told_once(&screen, input_method, |told| {
    told
      .surrounding
      .as_ref()
      .is_some_and(|(text, ..)| text == "•••••")
  });

  assert_eq!(told.surrounding, Some(("•••••".to_owned(), 5, 5)));

  // The anchor too: a selection made backward is told as marks, the caret before the anchor.
  password.press("Shift+Left");
  password.press("Shift+Left");

  let told = told_once(&screen, input_method, |told| {
    told
      .surrounding
      .as_ref()
      .is_some_and(|(_, cursor, anchor)| (*cursor, *anchor) == (3, 5))
  });

  assert_eq!(told.surrounding, Some(("•••••".to_owned(), 3, 5)));

  let since = told
    .surroundings
    .iter()
    .position(|text| text.starts_with('•'))
    .expect("the secure field's text was told");

  assert!(
    told.surroundings[since..]
      .iter()
      .all(|text| text.chars().all(|character| character == '•')),
    "from the secure field's focus on, every text told is marks: {:?}",
    told.surroundings
  );

  let secure = trace::take_recorded();

  trace::set_recording(false);
  eprintln!(
    "{} trace lines while the secure field was used",
    secure.len()
  );
  assert!(
    secure.iter().any(|line| line.starts_with("[key]")),
    "the keys typed into the secure field were traced"
  );

  let shown: Vec<&String> = secure
    .iter()
    .filter(|line| shows_the_secret(line))
    .collect();

  assert!(
    shown.is_empty(),
    "no trace line shows the secret: {shown:#?}"
  );
}

#[skene_gui::test]
fn a_secure_field_tells_fcitx_only_its_marks() {
  if Session::current().map(Session::backend) != Some(Backend::X11) {
    eprintln!(
      "not tested here: on Wayland the input method is the compositor's, over text-input-v3"
    );

    return;
  }

  let input_method = InputMethod::serve().expect("the Fcitx5 stand-in takes its name");

  tells_only_marks(&input_method);
  assert!(
    input_method.told().keys > 0,
    "the keys typed reached the input method as keys, as every key does"
  );
}

#[skene_gui::test]
fn a_secure_field_tells_ibus_only_its_marks() {
  if Session::current().map(Session::backend) != Some(Backend::X11) {
    eprintln!(
      "not tested here: on Wayland the input method is the compositor's, over text-input-v3"
    );

    return;
  }

  let input_method = InputMethod::serve_ibus().expect("the IBus stand-in takes its name");

  tells_only_marks(&input_method);
  assert_eq!(input_method.told().contexts, 1, "the window found IBus");
}

#[skene_gui::test]
fn a_secure_field_tells_the_compositors_input_method_only_its_marks() {
  if Session::current() != Some(Session::Sway) {
    eprintln!(
      "not tested here: of the legs, only sway lets a client be the seat's input method (zwp_input_method_v2)"
    );

    return;
  }

  let input_method = InputMethod::serve_wayland().expect("sway takes an input method");

  tells_only_marks(&input_method);
}

#[skene_gui::test]
fn no_trace_line_shows_a_secure_fields_text() {
  let screen = testing::render(page);
  let password = screen.find_by_role(Role::TextField, "Password");

  trace::set_recording(true);
  password.type_text(SECRET);
  assert_eq!(probe_once(&screen, "secret", SECRET), SECRET);

  for chord in ["Ctrl+a", "Ctrl+c", "Ctrl+x"] {
    password.press(chord);
  }

  assert_eq!(
    probe_once(&screen, "secret", SECRET),
    SECRET,
    "a secure field offers no cut"
  );

  for chord in ["Home", "Shift+End", "BackSpace"] {
    password.press(chord);
  }

  assert_eq!(probe_once(&screen, "secret", ""), "");
  password.type_text(SECRET);
  assert_eq!(probe_once(&screen, "secret", SECRET), SECRET);

  let lines = trace::take_recorded();

  trace::set_recording(false);
  assert!(
    lines.iter().any(|line| line.starts_with("[key]"))
      && lines.iter().any(|line| line.starts_with("[entry]")),
    "the keys and the edits were traced"
  );

  let shown: Vec<&String> = lines.iter().filter(|line| shows_the_secret(line)).collect();

  assert!(
    shown.is_empty(),
    "no trace line shows the secret: {shown:#?}"
  );
}

#[skene_gui::test]
fn a_double_press_in_a_secure_field_selects_it_all() {
  let screen = testing::render(page);
  let password = screen.find_by_role(Role::TextField, "Password");

  password.type_text("pass word");

  let at = screen.driver().character_point(password.element(), 2, 0.5);

  screen.driver().double_click_point(at);

  let selection = password
    .read()
    .selection
    .expect("the field reports its selection");

  assert_eq!(
    (
      selection.start.min(selection.end),
      selection.start.max(selection.end)
    ),
    (0, 9),
    "a word of a secure field is its whole text"
  );
}
