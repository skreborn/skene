//! Drag and drop on the real platform, through the driver, as GTK's entry does it: a
//! selection dragged within its field moves, one dragged to another field or to a box that
//! takes drops copies, the dragged text showing under the pointer as it goes (the drag's
//! icon); and between applications, a GTK 4 peer (`tools/dnd/peer.py`) taking text dragged
//! out of a field and giving text and files dragged in, and on X11 showing the cursors a drag
//! from the window shows. A leg where the peer cannot run, or where the compositor puts it over
//! what the test drags to, says so and tests nothing of it.

use std::{
  io::{BufRead as _, BufReader},
  process::{Child, Command, Stdio},
  sync::{Arc, Mutex},
  time::{Duration, Instant},
};

use skene_gui::{
  Color, Dropped, Signal,
  prelude::*,
  testing::{self, Driver as _, Found, Role, publish},
};

/// Publishes `signal` as the probe `name`, as `Debug` writes it.
fn probe<T: Clone + std::fmt::Debug + 'static>(name: &'static str, signal: Signal<T>) {
  publish(name, move || format!("{:?}", signal.get_untracked()));
}

/// The probe `name` once it reads `wanted`, or as it reads after five seconds, the test's
/// window turning meanwhile: a drop from another application lands once the two have traded
/// its data over the display server.
fn probe_reaching(screen: &testing::Screen, name: &str, wanted: &str) -> String {
  let started = Instant::now();

  loop {
    let read = screen.probe(name);

    if read == wanted || started.elapsed() >= Duration::from_secs(5) {
      return read;
    }

    screen.driver().wait(Duration::from_millis(20));
  }
}

/// Two fields, "First" holding "alpha beta" and "Second" holding "gamma", and a box that takes
/// drops, publishing what it took.
fn fields() -> impl View {
  let first = Signal::new(String::from("alpha beta"));
  let second = Signal::new(String::from("gamma"));
  let dropped = Signal::new(Vec::<String>::new());

  probe("first", first);
  probe("second", second);
  probe("dropped", dropped);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("First").bind_text(first))
    .add_child(TextField::new("Second").bind_text(second))
    .add_child(
      Container::new()
        .set_padding(24)
        .handle_drop(move |what: &Dropped| {
          let took = match &what.text {
            Some(text) => format!("text {text}"),
            None => format!("uris {}", what.uris.join(" ")),
          };

          dropped.update(|dropped| dropped.push(took));
        })
        .set_child(Text::new("Drop box")),
    )
}

/// Selects the first word of `field` from the keyboard: `Home`, then `Shift-Ctrl-Right`.
fn select_first_word(field: &Found<'_>) {
  field.press("Home").press("shift+ctrl+Right");

  let selection = field
    .read()
    .selection
    .expect("the field reports its selection");

  assert_eq!(
    (selection.start, selection.end),
    (0, 5),
    "the first word selected"
  );
}

/// A field holding a line of words above a pale box nothing drops on.
fn icon_page() -> impl View {
  let first = Signal::new(String::from("alpha beta gamma delta"));

  probe("first", first);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("First").bind_text(first))
    .add_child(
      Container::new()
        .set_height(160)
        .set_background(Color::rgb(0xf6d860))
        .set_child(Text::new("Pale box")),
    )
}

#[skene_gui::test]
fn a_dragged_selection_shows_its_text_under_the_pointer() {
  let screen = testing::render(icon_page);
  let first = screen.find_by_role(Role::TextField, "First");

  first.press("Home").press("shift+End");

  let driver = screen.driver();
  let unit = driver.units_per_pixel();
  let start = driver.character_rect(first.element(), 0);
  let end = driver.character_rect(first.element(), 21);
  let from = driver.character_point(first.element(), 2, 0.5);
  let to = (from.0 + 40. * unit, from.1 + 110. * unit);
  // The icon's text stands where the press was in the selection: from its start as far as the
  // press was, as far up as the press was from the line's top.
  let left = to.0 - (from.0 - start.0);
  let top = to.1 - (from.1 - start.1);
  let width = end.0 + end.2 - start.0;
  let rect = (
    left - 24. * unit,
    top - 24. * unit,
    width + 48. * unit,
    72. * unit,
  );

  drop(driver);

  let before = screen.driver().screen_area(rect);

  screen.driver().hold_drag(from, to);

  let during = screen.driver().screen_area(rect);

  screen.driver().release_button();

  let after = screen.driver().screen_area(rect);
  let shown =
    crate::changed(rect, &before, &during).expect("the drag shows its icon under the pointer");

  eprintln!("the icon shows at {shown:?}; the text stands at {left},{top}, {width} wide");
  assert!(
    (shown.0 - left).abs() <= 4. * unit,
    "the icon's text starts where the press was in it: {shown:?} against {left}"
  );
  assert!(
    (shown.0 + shown.2 - (left + width)).abs() <= 6. * unit,
    "the icon holds the whole selection: {shown:?} against {width} wide"
  );
  assert!(
    (shown.1 - top).abs() <= 8. * unit && shown.3 <= 32. * unit,
    "the icon is a line of text under the pointer: {shown:?} against {top}"
  );
  assert_eq!(
    crate::changed(rect, &before, &after),
    None,
    "the icon goes as the drag ends"
  );
  assert_eq!(
    screen.probe("first"),
    "\"alpha beta gamma delta\"",
    "nothing took the drop"
  );
}

#[skene_gui::test]
fn a_selection_dragged_within_its_field_moves_there() {
  let screen = testing::render(fields);
  let first = screen.find_by_role(Role::TextField, "First");

  select_first_word(&first);

  let (from, to) = {
    let driver = screen.driver();

    (
      driver.character_point(first.element(), 2, 0.5),
      driver.character_point(first.element(), 9, 0.9),
    )
  };

  screen.driver().drag(from, to);
  assert_eq!(
    screen.probe("first"),
    "\" betaalpha\"",
    "the word moved to the end"
  );
}

#[skene_gui::test]
fn a_selection_dragged_to_another_field_copies_there() {
  let screen = testing::render(fields);
  let first = screen.find_by_role(Role::TextField, "First");
  let second = screen.find_by_role(Role::TextField, "Second");

  select_first_word(&first);

  let (from, to) = {
    let driver = screen.driver();

    (
      driver.character_point(first.element(), 2, 0.5),
      driver.character_point(second.element(), 4, 0.9),
    )
  };

  screen.driver().drag(from, to);
  assert_eq!(screen.probe("second"), "\"gammaalpha\"");
  assert_eq!(
    screen.probe("first"),
    "\"alpha beta\"",
    "a copy leaves the field it came from"
  );
}

#[skene_gui::test]
fn a_selection_dropped_on_a_box_that_takes_drops_reaches_its_handler() {
  let screen = testing::render(fields);
  let first = screen.find_by_role(Role::TextField, "First");

  let target = screen.find_by_text("Drop box");

  select_first_word(&first);

  let (from, to) = {
    let driver = screen.driver();

    (
      driver.character_point(first.element(), 2, 0.5),
      driver.point_in(target.element(), 0.5, 0.5),
    )
  };

  screen.driver().drag(from, to);
  assert_eq!(screen.probe("dropped"), "[\"text alpha\"]");
  assert_eq!(screen.probe("first"), "\"alpha beta\"");
}

/// The GTK 4 peer application, running beside the test's window.
struct Peer {
  /// Its process.
  child: Child,
  /// What it printed, a line each.
  lines: Arc<Mutex<Vec<String>>>,
  /// Its window's origin in the pointer's space.
  origin: (f64, f64),
  /// Its window's size in the pointer's space.
  size: (f64, f64),
  /// How many of the pointer space's units a pixel of the peer is.
  unit: f64,
  /// The test's window's rectangle in the pointer's space.
  window: (f64, f64, f64, f64),
}

/// The file the peer offers.
const PEER_FILE: &str = "/tmp/skene-peer.txt";

impl Peer {
  /// A Python with PyGObject and GTK 4, the peer's interpreter.
  fn python() -> Option<&'static str> {
    ["python3", "python3.12", "python3.13", "python3.11"]
      .into_iter()
      .find(|python| {
        Command::new(python)
          .args([
            "-I",
            "-c",
            "import gi; gi.require_version('Gtk', '4.0'); from gi.repository import Gtk",
          ])
          .stdout(Stdio::null())
          .stderr(Stdio::null())
          .status()
          .is_ok_and(|status| status.success())
      })
  }

  /// Starts the peer, its field holding `text`, beside the window on `screen`, where `clear`
  /// holds of it: a compositor that places a new window at random (weston) puts the peer
  /// over what the test drags from or to as often as not, so the peer is started afresh, up
  /// to five times, until it does not. `None`, saying why, where it cannot run, is never
  /// found, or never lands clear.
  fn start(
    screen: &testing::Screen,
    test: &str,
    text: &str,
    clear: impl Fn(&Self) -> bool,
  ) -> Option<Self> {
    let Some(python) = Self::python() else {
      skene_gui_linux::testing::not_tested_here("no Python with PyGObject and GTK 4 for the peer");

      return None;
    };
    // The test's window is found before the peer's can cover it.
    let window = screen.driver().window_rect();

    for _ in 0..5 {
      let peer = Self::start_once(screen, test, text, python, window)?;

      if clear(&peer) {
        return Some(peer);
      }

      eprintln!(
        "{test}: the compositor put the peer and the test's window over each other; starting it again"
      );
    }

    skene_gui_linux::testing::not_tested_here(
      "the compositor never put the peer clear of the test's window",
    );

    None
  }

  /// Starts the peer once, `python` running it, the test's window at `window`, and finds
  /// where its window is by moving the pointer over the screen until it sees it. Where the
  /// peer places itself (X11 without a window manager, sway), it asks for the room beside the
  /// test's window.
  fn start_once(
    screen: &testing::Screen,
    test: &str,
    text: &str,
    python: &str,
    window: (f64, f64, f64, f64),
  ) -> Option<Self> {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../tools/dnd/peer.py");
    let backend = if std::env::var("SKENE_WINDOW_SYSTEM").as_deref() == Ok("x11") {
      "x11"
    } else {
      "wayland"
    };
    // An X11 peer's pixel is the X screen's device pixel, a Wayland one's the compositor's
    // logical pixel.
    let unit = if backend == "x11" {
      let driver = screen.driver();

      driver.units_per_pixel() / driver.scale()
    } else {
      1.
    };
    let placed = (window.0 + window.2 + 24., window.1.max(0.));
    let mut child = Command::new(python)
      .args(["-I", script, text, PEER_FILE])
      .env("SKENE_PEER_X", format!("{}", (placed.0 / unit).round()))
      .env("SKENE_PEER_Y", format!("{}", (placed.1 / unit).round()))
      .env("GDK_BACKEND", backend)
      .env("GTK_A11Y", "none")
      .env("GSK_RENDERER", "cairo")
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .ok()?;
    let lines = Arc::new(Mutex::new(Vec::new()));
    let stdout = child.stdout.take()?;

    {
      let lines = Arc::clone(&lines);

      std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
          lines.lock().expect("the peer's lines").push(line);
        }
      });
    }

    let mut peer = Self {
      child,
      lines,
      origin: (0., 0.),
      size: (0., 0.),
      unit,
      window,
    };
    let Some(ready) = peer.wait_for(screen, Duration::from_secs(20), |line| {
      line.starts_with("[peer] ready ")
    }) else {
      skene_gui_linux::testing::not_tested_here("the peer never showed its window");

      return None;
    };
    let numbers = |line: &str| -> Vec<f64> {
      line
        .split_whitespace()
        .filter_map(|word| word.parse().ok())
        .collect()
    };
    let size = numbers(&ready);

    peer.size = (size[0] * unit, size[1] * unit);

    let (width, height) = screen.driver().screen_extent();
    let outside = |(x, y): (f64, f64)| {
      x < window.0 - 40.
        || y < window.1 - 40.
        || x > window.0 + window.2 + 40.
        || y > window.1 + window.3 + 40.
    };
    let mut points = vec![(placed.0 + peer.size.0 / 2., placed.1 + peer.size.1 / 2.)];

    // Eight by eight cells, each smaller than the peer's window, so a point falls on it
    // wherever it is.
    for row in 1..8 {
      for column in 1..8 {
        points.push((width * f64::from(column) / 8., height * f64::from(row) / 8.));
      }
    }

    // Off the test's window first; then over it, where the peer may have opened on top.
    let (off, over): (Vec<_>, Vec<_>) = points.into_iter().partition(|point| outside(*point));

    for point in off.into_iter().chain(over) {
      let seen = peer.lines().len();

      screen.driver().move_pointer(point);

      let found = peer.wait_for_after(screen, seen, Duration::from_millis(250), |line| {
        line.starts_with("[peer] pointer ")
      });

      if let Some(line) = found {
        let at = numbers(&line);

        peer.origin = (point.0 - at[0] * unit, point.1 - at[1] * unit);
        eprintln!(
          "{test}: the peer's window is at {:?}, {:?} large; the test's at {window:?}",
          peer.origin, peer.size
        );

        return Some(peer);
      }
    }

    skene_gui_linux::testing::not_tested_here("the pointer never found the peer's window");

    None
  }

  /// Makes room for the peer where the compositor moves a window when asked (sway): the
  /// test's window goes to the screen's top left. A test calls it before it reads where
  /// anything is.
  fn make_room(screen: &testing::Screen) {
    if std::env::var_os("SWAYSOCK").is_none() {
      return;
    }

    let moved = Command::new("swaymsg")
      .arg("[app_id=dev.skene.test] move absolute position 0 0")
      .stdout(Stdio::null())
      .status()
      .is_ok_and(|status| status.success());

    assert!(moved, "swaymsg moves the window");
    screen.driver().window_moved();
  }

  /// What the peer printed so far.
  fn lines(&self) -> Vec<String> {
    self.lines.lock().expect("the peer's lines").clone()
  }

  /// The first line the peer printed that `wanted` takes, the test's window turning while it
  /// waits up to `timeout`.
  fn wait_for(
    &self,
    screen: &testing::Screen,
    timeout: Duration,
    wanted: impl Fn(&str) -> bool,
  ) -> Option<String> {
    self.wait_for_after(screen, 0, timeout, wanted)
  }

  /// The first line after the first `skip` that `wanted` takes, as [`wait_for`] waits.
  fn wait_for_after(
    &self,
    screen: &testing::Screen,
    skip: usize,
    timeout: Duration,
    wanted: impl Fn(&str) -> bool,
  ) -> Option<String> {
    let started = Instant::now();

    loop {
      if let Some(line) = self
        .lines()
        .into_iter()
        .skip(skip)
        .find(|line| wanted(line))
      {
        return Some(line);
      }

      if started.elapsed() >= timeout {
        return None;
      }

      screen.driver().wait(Duration::from_millis(20));
    }
  }

  /// The point at fractions `(x, y)` of the peer's widget `name` (`field`, `drop` or
  /// `source`), in the pointer's space.
  fn point(&self, name: &str, x: f64, y: f64) -> (f64, f64) {
    let prefix = format!("[peer] place {name} ");
    let line = self
      .lines()
      .into_iter()
      .find(|line| line.starts_with(&prefix))
      .unwrap_or_else(|| panic!("the peer placed no {name}"));
    let numbers: Vec<f64> = line[prefix.len()..]
      .split_whitespace()
      .filter_map(|word| word.parse().ok())
      .collect();

    (
      self.origin.0 + (numbers[0] + numbers[2] * x) * self.unit,
      self.origin.1 + (numbers[1] + numbers[3] * y) * self.unit,
    )
  }

  /// Whether `point` (in the pointer's space) is on the test's window or the margin around it
  /// that takes the pointer for resizing; a press on the window raises it over the peer's.
  fn under_window(&self, point: (f64, f64)) -> bool {
    let (x, y, width, height) = self.window;

    Self::within(point, (x, y), (width, height))
  }

  /// Whether `point` (in the pointer's space) is on the peer's window or its resize margin.
  fn covers(&self, point: (f64, f64)) -> bool {
    Self::within(point, self.origin, self.size)
  }

  /// Whether `point` is within a window at `origin` of `size`, or its resize margin (GTK's and
  /// Skene's are under 24 pixels).
  fn within(point: (f64, f64), origin: (f64, f64), size: (f64, f64)) -> bool {
    const MARGIN: f64 = 24.;

    point.0 >= origin.0 - MARGIN
      && point.1 >= origin.1 - MARGIN
      && point.0 < origin.0 + size.0 + MARGIN
      && point.1 < origin.1 + size.1 + MARGIN
  }
}

impl Drop for Peer {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

#[skene_gui::test]
fn a_selection_dragged_out_drops_into_another_applications_field() {
  let test = "a_selection_dragged_out_drops_into_another_applications_field";
  let screen = testing::render(fields);

  Peer::make_room(&screen);

  let first = screen.find_by_role(Role::TextField, "First");

  select_first_word(&first);

  let from = screen.driver().character_point(first.element(), 2, 0.5);
  let to = |peer: &Peer| peer.point("field", 0.97, 0.5);
  // A press on the test's window raises it over the peer's.
  let Some(peer) = Peer::start(&screen, test, "peer words", |peer| {
    !peer.covers(from) && !peer.under_window(to(peer))
  }) else {
    return;
  };

  screen.driver().drag(from, to(&peer));

  let took = peer.wait_for(&screen, Duration::from_secs(3), |line| {
    line.starts_with("[peer] field ")
  });

  assert_eq!(
    took.as_deref(),
    Some("[peer] field alpha"),
    "GTK's entry took the text in place of its selection, as it takes a drop on it"
  );
  assert_eq!(
    screen.probe("first"),
    "\"alpha beta\"",
    "a drop into another application copies"
  );
}

#[skene_gui::test]
fn text_dragged_in_from_another_application_drops_into_a_field() {
  let test = "text_dragged_in_from_another_application_drops_into_a_field";
  let screen = testing::render(fields);

  Peer::make_room(&screen);

  let second = screen.find_by_role(Role::TextField, "Second");
  let to = screen.driver().character_point(second.element(), 4, 0.9);
  let from = |peer: &Peer| peer.point("field", 0.1, 0.5);
  // A press on the peer's window raises it over the test's.
  let Some(peer) = Peer::start(&screen, test, "peer words", |peer| {
    !peer.covers(to) && !peer.under_window(from(peer))
  }) else {
    return;
  };

  // The peer's field holds its text selected; a drag from it carries the selection.
  screen.driver().drag(from(&peer), to);
  assert_eq!(
    probe_reaching(&screen, "second", "\"gammapeer words\""),
    "\"gammapeer words\"",
    "the peer printed {:?}",
    peer.lines()
  );
}

#[skene_gui::test]
fn files_dragged_in_from_another_application_reach_a_box_that_takes_drops() {
  let test = "files_dragged_in_from_another_application_reach_a_box_that_takes_drops";
  let screen = testing::render(fields);

  Peer::make_room(&screen);

  let target = screen.find_by_text("Drop box");
  let to = screen.driver().point_in(target.element(), 0.5, 0.5);
  let from = |peer: &Peer| peer.point("source", 0.5, 0.5);
  let Some(peer) = Peer::start(&screen, test, "peer words", |peer| {
    !peer.covers(to) && !peer.under_window(from(peer))
  }) else {
    return;
  };

  let dropped = format!("[\"uris file://{PEER_FILE}\"]");

  screen.driver().drag(from(&peer), to);
  assert_eq!(
    probe_reaching(&screen, "dropped", &dropped),
    dropped,
    "the peer printed {:?}",
    peer.lines()
  );
}

#[skene_gui::test]
fn a_drag_shows_the_cursors_gtks_drag_shows_on_x11() {
  let test = "a_drag_shows_the_cursors_gtks_drag_shows_on_x11";
  let screen = testing::render(fields);

  if std::env::var("SKENE_WINDOW_SYSTEM").as_deref() != Ok("x11") {
    skene_gui_linux::testing::not_tested_here("a Wayland compositor shows a drag's cursor itself");

    return;
  }

  Peer::make_room(&screen);

  let first = screen.find_by_role(Role::TextField, "First");
  let second = screen.find_by_role(Role::TextField, "Second");
  let caption = screen.find_by_text("Drop box");
  let (from, own, other, taker, nothing) = {
    let driver = screen.driver();
    let taker = driver.point_in(caption.element(), 0.5, 0.5);

    (
      driver.character_point(first.element(), 2, 0.5),
      driver.character_point(first.element(), 9, 0.9),
      driver.character_point(second.element(), 4, 0.9),
      taker,
      // The padding left of the fields, which takes no drop.
      (driver.point_in(first.element(), 0., 0.5).0 - 8., taker.1),
    )
  };

  select_first_word(&first);
  screen.driver().hold_drag(from, taker);

  let mut ours = vec![("a box that takes drops", screen.driver().cursor_name())];

  for (over, at) in [
    ("another field", other),
    ("its own field", own),
    ("nothing that takes drops", nothing),
  ] {
    screen.driver().move_pointer(at);
    ours.push((over, screen.driver().cursor_name()));
  }

  screen.driver().release_button();
  eprintln!("Skene's drag shows {ours:?}");

  let Some(peer) = Peer::start(&screen, test, "peer words", |peer| {
    !peer.covers(taker)
      && !peer.covers(nothing)
      && !peer.under_window(peer.point("field", 0.1, 0.5))
  }) else {
    return;
  };

  // GTK's entry holds its text selected; a drag from it goes over the same kinds of place but
  // its own field, all of it selected. The peer answers its own drag in its own time: each
  // cursor is read once it had it.
  screen
    .driver()
    .hold_drag(peer.point("field", 0.1, 0.5), peer.point("drop", 0.5, 0.5));

  let settled = || {
    screen.driver().wait(Duration::from_millis(400));
    screen.driver().cursor_name()
  };
  let mut gtk = vec![("a box that takes drops", settled())];

  for (over, at) in [
    ("another field", other),
    ("nothing that takes drops", peer.point("source", 0.5, 0.5)),
  ] {
    screen.driver().move_pointer(at);
    gtk.push((over, settled()));
  }

  screen.driver().release_button();
  eprintln!("GTK's drag shows {gtk:?}");

  // A move shows GDK's `dnd-move` where the theme has it; where it has none (Adwaita 46), the
  // default cursor, as GTK's drag shows none of that name there.
  let moving = ours[2].1.as_deref().filter(|name| *name == "dnd-move");

  assert_eq!(
    ours
      .iter()
      .map(|(over, name)| (*over, name.as_deref()))
      .collect::<Vec<_>>(),
    [
      ("a box that takes drops", Some("copy")),
      ("another field", Some("copy")),
      ("its own field", moving.or(Some("default"))),
      ("nothing that takes drops", Some("default")),
    ],
    "Skene's drag names GDK's cursors"
  );
  assert_eq!(
    [&ours[0], &ours[1], &ours[3]],
    [&gtk[0], &gtk[1], &gtk[2]],
    "Skene's drag shows the cursors GTK's shows"
  );
}
