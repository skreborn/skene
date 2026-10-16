//! Testing on the real platform: a view rendered into a real window, found as a screen reader
//! finds it and acted on as a user acts.
//!
//! ```no_run
//! use skene_gui::{
//!   prelude::*,
//!   testing::{self, Role},
//! };
//!
//! fn settings() -> impl View {
//!   let sync = Signal::new(true);
//!
//!   Column::new()
//!     .add_child(Toggle::new("Sync").bind_on(sync))
//!     .add_child(Text::derive(move || if sync.get() { "Sync is on" } else { "Sync is off" }))
//! }
//!
//! #[skene_gui::test]
//! fn sync_turns_off() {
//!   let screen = testing::render(settings);
//!
//!   screen.find_by_role(Role::Switch, "Sync").click();
//!   screen.find_by_text("Sync is off");
//! }
//! ```
//!
//! There is no test double: the realization is the real one, on the build's platform.
//!
//! - **What a test reads is what a reader finds**: roles, names, states, values and text come
//!   from the platform's accessibility layer (on Linux, AT-SPI over D-Bus), never from the
//!   app's own tree.
//! - **What a test does is what a user does**: a click, a key and typed text are real input
//!   (on Linux, XTest on X11, mutter's EIS or sway's virtual devices on Wayland), at the
//!   bounds a reader reads. A key goes where focus is, and focus moves as a user moves it:
//!   [`Found::press`] presses `Tab` until a reader reads the node focused.
//! - **Every action settles**: the input delivered, then no frame painted, nothing reported
//!   and no input arriving for a quiet window. A find looks again for up to a second.
//! - **The reader's settings come through the platform's path**: on Linux a stand-in of the
//!   settings portal on the test's private bus, which the app follows as it follows the
//!   desktop's.
//!
//! Positions ([`AccessSnapshot::bounds`], [`Screen::pixel`]) are pixels of the window's
//! content: a title bar the platform draws (on mutter, Skene's own) is not part of it.
//!
//! A test runs under `#[skene_gui::test]`: a plain `#[test]` (so `cargo test` and
//! `cargo nextest` both run it) whose body runs on the process's window thread, the
//! environment and the reader's settings reset before each. Cargo runs every test binary
//! through the display runner (`skene-runner`, which the `.cargo/config.toml` `skene new`
//! writes names), so each test process has a private session (on Linux, Xvfb by default, or
//! headless mutter where `SKENE_TEST_SESSION=mutter` asks), session bus and accessibility
//! bus, and never touches the user's display: a test started outside the runner's session
//! fails before its body runs.

use std::{
  cell::{RefCell, RefMut},
  path::Path,
  rc::Rc,
};

pub use skene_foundation::seam::{
  access::{Checked, Role},
  testing::{
    AccessSnapshot, Action, Driver, EnvironmentRequest, Frame, Issue, Query, Screenshot, pages,
  },
};
use skene_foundation::{Owner, seam::report::Modifiers, value::IntoUnit};
/// The driver on Linux: AT-SPI reads, real input, settling on the window's own loop.
pub use skene_gui_linux::testing::{Drawn, Element, Session};

use crate::{__private, Color, Scheme, View};

/// Renders the view `app` returns into a new window, 480 by 640 pixels, settled.
///
/// # Panics
///
/// Outside a `#[skene_gui::test]`, or where the window never shows.
pub fn render<V: View>(app: impl FnOnce() -> V) -> Screen {
  window_thread();

  let owner = Owner::new();
  let root = owner.run(|| __private::root(app));
  let mut driver = Drawn::new();

  driver.show(owner, root);

  Screen {
    driver: RefCell::new(driver),
  }
}

/// Opens the page registered as `name` ([`register`]) in a new window, settled.
///
/// # Panics
///
/// Outside a `#[skene_gui::test]`, or where no page is registered under `name`.
pub fn open(name: &str) -> Screen {
  window_thread();

  let mut driver = Drawn::new();

  driver.open(name);

  Screen {
    driver: RefCell::new(driver),
  }
}

/// Registers the view `page` returns as a page named `name`, which a test opens by name
/// ([`open`], [`Driver::open`]): what a conformance suite runs its scenarios on.
pub fn register<V: View>(name: &'static str, page: fn() -> V) {
  skene_foundation::seam::testing::register(name, move || __private::root(page));
}

/// Publishes a probe of the page being built: what a signal it binds holds, by name. The only
/// state a test reads that a reader does not.
pub fn publish(name: &'static str, probe: impl Fn() -> String + 'static) {
  skene_foundation::seam::testing::publish(name, probe);
}

/// Fails outside `#[skene_gui::test]`, whose window thread a test's windows open on.
fn window_thread() {
  assert!(
    std::thread::current().name() == Some("skene-ui"),
    "skene_gui::testing runs inside a #[skene_gui::test], on the window thread it provides"
  );
}

/// The key `chord` names (`"Tab"`, `"ctrl+z"`, `"shift+Tab"`), as the driver presses it.
fn chord(chord: &str) -> (Rc<str>, Modifiers) {
  let mut modifiers = Modifiers::default();
  let mut parts: Vec<&str> = chord.split('+').collect();
  let key = match parts.pop() {
    Some("") if chord.ends_with('+') => {
      parts.pop();
      "plus"
    }
    Some(key) => key,
    None => chord,
  };

  for part in parts {
    match part.to_ascii_lowercase().as_str() {
      "ctrl" | "control" => modifiers.control = true,
      "shift" => modifiers.shift = true,
      "alt" => modifiers.alt = true,
      "super" | "meta" | "cmd" => modifiers.meta = true,
      other => panic!("{other:?} in {chord:?} is not a modifier: ctrl, shift, alt or super"),
    }
  }

  (Rc::from(key), modifiers)
}

/// A window a test rendered or opened, and the driver reading and acting on it.
#[must_use]
pub struct Screen {
  driver: RefCell<Drawn>,
}

impl Screen {
  /// The driver, for what the methods here do not cover (a conformance scenario's
  /// [`Driver`] calls).
  pub fn driver(&self) -> RefMut<'_, Drawn> {
    self.driver.borrow_mut()
  }

  /// The object of `role` named `name`, looking again for up to a second.
  ///
  /// # Panics
  ///
  /// Where there is none, showing what a reader finds instead.
  pub fn find_by_role(&self, role: Role, name: &str) -> Found<'_> {
    self.query_by_role(role, name).unwrap_or_else(|| {
      panic!(
        "no {role:?} named {name:?}; a reader finds:\n{}",
        self.outline()
      )
    })
  }

  /// The object of `role` named `name`, where one turns up within a second.
  pub fn query_by_role(&self, role: Role, name: &str) -> Option<Found<'_>> {
    self.first(&Query::Role(role, name.into()))
  }

  /// The text showing `text`, looking again for up to a second.
  ///
  /// # Panics
  ///
  /// Where there is none, showing what a reader finds instead.
  pub fn find_by_text(&self, text: &str) -> Found<'_> {
    self
      .query_by_text(text)
      .unwrap_or_else(|| panic!("no text {text:?}; a reader finds:\n{}", self.outline()))
  }

  /// The text showing `text`, where one turns up within a second.
  pub fn query_by_text(&self, text: &str) -> Option<Found<'_>> {
    self.first(&Query::Text(text.into()))
  }

  /// The object that has keyboard focus.
  ///
  /// # Panics
  ///
  /// Where nothing has focus, showing what a reader finds.
  pub fn focused(&self) -> Found<'_> {
    self
      .first(&Query::Focused)
      .unwrap_or_else(|| panic!("nothing has focus; a reader finds:\n{}", self.outline()))
  }

  fn first(&self, query: &Query) -> Option<Found<'_>> {
    let element = self.driver().find_soon(query).into_iter().next()?;

    Some(Found {
      screen: self,
      element,
    })
  }

  /// Presses the key `chord` names (`"Tab"`, `"ctrl+z"`, `"shift+Tab"`; X11's key names,
  /// `Enter` and `Esc` among their aliases), focus where it is, and settles.
  pub fn press(&self, chord_name: &str) {
    let (key, modifiers) = chord(chord_name);

    self.act_at_focus(&Action::Press(key, modifiers));
  }

  /// Types `text`, a key per character, focus where it is, and settles.
  pub fn type_text(&self, text: &str) {
    self.act_at_focus(&Action::Type(text.into()));
  }

  fn act_at_focus(&self, action: &Action) {
    self.driver().act_at_focus(action);
  }

  /// Runs the platform until it is idle.
  pub fn settle(&self) {
    self.driver().settle();
  }

  /// The app chooses the dark scheme, or the light one (`media::set_scheme`), through the
  /// platform's own path, and settles.
  pub fn set_dark(&self, dark: bool) {
    self.set_scheme(Some(if dark { Scheme::Dark } else { Scheme::Light }));
  }

  /// The app chooses the scheme, or follows the reader's again (`None`), and settles.
  pub fn set_scheme(&self, scheme: Option<Scheme>) {
    self
      .driver()
      .set_environment(EnvironmentRequest::Scheme(scheme));
  }

  /// The reader's own scheme changes to `scheme`, as the platform's settings report it (on
  /// Linux, the settings portal's `color-scheme`), and settles. The app follows it while it
  /// chooses none.
  pub fn set_reader_scheme(&self, scheme: Scheme) {
    self
      .driver()
      .set_environment(EnvironmentRequest::ReaderScheme(scheme));
  }

  /// The reader asks for more contrast, or no longer does, as the platform's settings report it
  /// (on Linux, the settings portal's `contrast`), and settles.
  pub fn set_high_contrast(&self, high: bool) {
    self
      .driver()
      .set_environment(EnvironmentRequest::HighContrast(high));
  }

  /// The reader turns animations off (reduced motion), or on again, as the platform's settings
  /// report it (on Linux, the settings portal's `reduced-motion`), and settles.
  pub fn set_reduced_motion(&self, reduced: bool) {
    self
      .driver()
      .set_environment(EnvironmentRequest::ReducedMotion(reduced));
  }

  /// Resizes the window's content to `width` pixels wide, its height kept, and settles.
  pub fn set_width(&self, width: i32) {
    let height = self.driver().size().1.round() as i32;

    self.set_size(width, height);
  }

  /// Resizes the window's content to `width` by `height` pixels, and settles.
  pub fn set_size(&self, width: i32, height: i32) {
    self
      .driver()
      .set_environment(EnvironmentRequest::Size(width as f32, height as f32));
  }

  /// The window's content drawn now, as its pixels.
  pub fn render_to_image(&self) -> Screenshot {
    self.driver().screenshot()
  }

  /// Writes the window's content, as drawn now, into a PNG at `path`.
  ///
  /// # Panics
  ///
  /// Where the file cannot be written.
  pub fn render_to_png(&self, path: impl AsRef<Path>) {
    self
      .driver()
      .save_png(path.as_ref())
      .unwrap_or_else(|error| panic!("{}: {error}", path.as_ref().display()));
  }

  /// The color of the content's pixel at `(x, y)`, in pixels from its top left corner (an
  /// integer or a float each).
  pub fn pixel(&self, x: impl IntoUnit<f32>, y: impl IntoUnit<f32>) -> Color {
    let shot = self.render_to_image();
    let (x, y) = (
      x.into_unit().max(0.) as usize,
      y.into_unit().max(0.) as usize,
    );
    let at = (y * shot.width as usize + x) * 4;
    let rgba = shot.rgba.get(at..at + 4).unwrap_or(&[0, 0, 0, 0]);

    Color::rgba(u32::from_be_bytes([rgba[0], rgba[1], rgba[2], rgba[3]]))
  }

  /// What a reader finds in the window, one object a line, indented by depth.
  #[must_use]
  pub fn outline(&self) -> String {
    self.driver().outline()
  }

  /// The open page's probe `name` ([`publish`]).
  ///
  /// # Panics
  ///
  /// Where the page published no such probe.
  #[must_use]
  pub fn probe(&self, name: &str) -> String {
    self
      .driver()
      .probe(name)
      .unwrap_or_else(|| panic!("the page publishes no probe {name:?}"))
      .to_string()
  }

  /// What a reader would meet that it should not: a control without a name, a picture
  /// without a description, a disabled control with focus.
  #[must_use]
  pub fn audit(&self) -> Vec<Issue> {
    self.driver().audit()
  }
}

/// An object a reader found in a [`Screen`]. Finding it is itself a check, so it may be
/// dropped unused.
pub struct Found<'s> {
  screen: &'s Screen,
  element: Element,
}

impl Found<'_> {
  /// The object as the driver holds it.
  #[must_use]
  pub const fn element(&self) -> &Element {
    &self.element
  }

  fn act(&self, action: &Action) -> &Self {
    self.screen.driver().act(&self.element, action);
    self
  }

  /// Asks for `request` as a reader does, through the platform's accessibility layer (AT-SPI's
  /// actions, its value interface and its focus), and settles.
  pub fn trigger_access(&self, request: skene_foundation::seam::control::AccessRequest) -> &Self {
    self.act(&Action::Access(request))
  }

  /// Clicks its center with the primary button, and settles.
  pub fn click(&self) -> &Self {
    self.act(&Action::Click)
  }

  /// Clicks the point of its bounds `x` and `y` of their width and height from their top
  /// leading corner (`0.9, 0.5`: near its trailing edge) with the primary button, and settles.
  pub fn click_at(&self, x: f32, y: f32) -> &Self {
    self.act(&Action::ClickAt(x, y))
  }

  /// Clicks its center with the secondary button, and settles.
  pub fn context_click(&self) -> &Self {
    self.act(&Action::ContextClick)
  }

  /// Moves the pointer over its center, and settles.
  pub fn hover(&self) -> &Self {
    self.act(&Action::Hover)
  }

  /// Presses the primary button at its center, drags `right` and `down` pixels (negative the
  /// other way; an integer or a float each) and releases there, and settles.
  pub fn drag_by(&self, right: impl IntoUnit<f32>, down: impl IntoUnit<f32>) -> &Self {
    self.act(&Action::Drag(right.into_unit(), down.into_unit()))
  }

  /// Turns the wheel over it: `right` and `down` steps (negative the other way), and settles.
  pub fn scroll(&self, right: i32, down: i32) -> &Self {
    self.act(&Action::Scroll(right, down))
  }

  /// Moves focus to it as a user does from the keyboard (`Tab` until a reader reads it
  /// focused), then presses the key `chord` names, and settles.
  pub fn press(&self, chord_name: &str) -> &Self {
    let (key, modifiers) = chord(chord_name);

    self.screen.driver().focus(&self.element);
    self.act(&Action::Press(key, modifiers))
  }

  /// Clicks into it, moves the caret to the end of its text, and types `text` there, a key
  /// per character; settles.
  pub fn type_text(&self, text: &str) -> &Self {
    self.click();

    self.act(&Action::Press(
      "End".into(),
      Modifiers {
        control: true,
        ..Modifiers::default()
      },
    ));
    self.act(&Action::Type(text.into()))
  }

  /// Starts composing `character` in it through the platform's input method, focus where it
  /// is, its preedit left showing; settles.
  pub fn compose(&self, character: char) -> &Self {
    self.act(&Action::Compose(character))
  }

  /// Commits the open composition; settles.
  pub fn commit(&self) -> &Self {
    self.act(&Action::Commit)
  }

  /// Everything a reader reads of it now.
  #[must_use]
  pub fn read(&self) -> AccessSnapshot {
    self.screen.driver().read(&self.element)
  }

  /// Its accessible name now.
  #[must_use]
  pub fn name(&self) -> String {
    self.read().name.to_string()
  }

  /// Its description now, empty where it has none.
  #[must_use]
  pub fn description(&self) -> String {
    self
      .read()
      .description
      .map(|description| description.to_string())
      .unwrap_or_default()
  }

  /// Whether a switch, a checkbox or a radio is on now.
  #[must_use]
  pub fn is_on(&self) -> bool {
    self.read().checked == Some(Checked::On)
  }

  /// A text entry's text now, or a text's.
  #[must_use]
  pub fn text(&self) -> String {
    let snapshot = self.read();

    snapshot.text.unwrap_or(snapshot.name).to_string()
  }

  /// A slider's value now.
  ///
  /// # Panics
  ///
  /// Where it has no value.
  #[must_use]
  pub fn value(&self) -> f64 {
    self
      .read()
      .value
      .unwrap_or_else(|| panic!("{:?} has no value", self.element.name))
  }

  /// Whether it has keyboard focus now.
  #[must_use]
  pub fn is_focused(&self) -> bool {
    self.read().focused
  }

  /// Whether it is enabled now.
  #[must_use]
  pub fn is_enabled(&self) -> bool {
    self.read().enabled
  }

  /// The keyboard shortcut a reader reads for it (`"<Control>s"`), empty where it has none.
  #[must_use]
  pub fn shortcut(&self) -> String {
    skene_gui_linux::testing::key_binding(&self.element)
  }
}
