//! # skene-conformance
//!
//! One suite of scenarios every realization passes, written once against the testing
//! [`Driver`]: each opens a page of the suite (a view built with skene-gui's shared views,
//! publishing probes of the signals it binds), acts as a user acts, and checks what a reader
//! finds. A divergence a scenario finds is a bug in the realization, never a special case in
//! the suite. Here the suite runs through the Linux driver (`tests/linux.rs`), on the window
//! Skene draws, in each of the display runner's sessions (X11, headless mutter, headless sway).
//!
//! The pages ([`pages`]) and the scenarios ([`scenarios`]) are public, so another driver runs
//! the same suite against the same pages.

pub mod pages;

use std::time::Duration;

use skene_gui::testing::{
  AccessSnapshot, Action, Checked, Driver, EnvironmentRequest, Query, Role,
};
use skene_gui::{Modifiers, Scheme};

/// A scenario: its name and what it does through a driver.
pub type Scenario<D> = (&'static str, fn(&mut D));

/// Every scenario, in the order the suite runs them.
#[must_use]
pub fn scenarios<D: Driver>() -> Vec<Scenario<D>> {
  vec![
    (
      "a_toggle_takes_a_flip_its_handler_accepts",
      toggle_accepts::<D>,
    ),
    (
      "a_toggle_refuses_a_flip_its_handler_ignores",
      toggle_refuses::<D>,
    ),
    ("a_text_field_takes_what_is_typed", text_types::<D>),
    (
      "a_text_field_shows_what_its_handler_makes_of_an_edit",
      text_transforms::<D>,
    ),
    (
      "a_text_field_refuses_a_character_its_handler_refuses",
      text_refuses::<D>,
    ),
    ("a_composition_commits_into_the_truth", text_composes::<D>),
    (
      "a_slider_steps_and_its_handler_clamps",
      slider_steps_and_clamps::<D>,
    ),
    ("a_picker_chooses_from_its_list", picker_chooses::<D>),
    (
      "a_control_owns_its_press_inside_a_box_that_hears_presses",
      control_owns_its_press::<D>,
    ),
    ("tab_moves_focus_in_reading_order", focus_order::<D>),
    (
      "tab_follows_the_scene_not_the_geometry",
      focus_follows_scene::<D>,
    ),
    (
      "an_activatable_row_holds_controls_that_own_their_presses",
      row_holds_controls::<D>,
    ),
    ("a_shortcut_activates_its_button", shortcut::<D>),
    (
      "the_dark_scheme_reinks_without_rebuilding",
      dark_reinks::<D>,
    ),
    ("a_width_condition_follows_the_window", width_condition::<D>),
    ("a_library_view_is_controlled", library_controlled::<D>),
    (
      "a_painted_view_is_its_description",
      painted_description::<D>,
    ),
    ("every_page_passes_the_audit", audit::<D>),
  ]
}

/// Runs the scenario `name` through `driver`, the pages registered first.
///
/// # Panics
///
/// Where no scenario is named `name`, or the scenario fails.
pub fn run<D: Driver>(driver: &mut D, name: &str) {
  pages::register();

  let (_, scenario) = scenarios::<D>()
    .into_iter()
    .find(|(scenario, _)| *scenario == name)
    .unwrap_or_else(|| panic!("no scenario is named {name:?}"));

  scenario(driver);
}

/// How long a find looks again.
const FIND: Duration = Duration::from_secs(1);

/// The one node `query` finds.
fn one<D: Driver>(driver: &mut D, query: &Query) -> D::Found {
  let mut found = driver.find(query, FIND);

  match found.len() {
    1 => found.remove(0),
    count => panic!(
      "{query:?} found {count} nodes, one expected; a reader finds:\n{}",
      driver.outline()
    ),
  }
}

/// The node of `role` named `name`.
fn role<D: Driver>(driver: &mut D, role: Role, name: &str) -> D::Found {
  one(driver, &Query::Role(role, name.into()))
}

/// The text showing `text`.
fn text<D: Driver>(driver: &mut D, text: &str) -> D::Found {
  one(driver, &Query::Text(text.into()))
}

/// What a reader reads of the node of `role` named `name`.
fn read<D: Driver>(driver: &mut D, kind: Role, name: &str) -> AccessSnapshot {
  let found = role(driver, kind, name);

  driver.read(&found)
}

/// The open page's probe `name`.
fn probe<D: Driver>(driver: &mut D, name: &str) -> String {
  driver
    .probe(name)
    .unwrap_or_else(|| panic!("the page publishes no probe {name:?}"))
    .to_string()
}

/// Clicks the node of `role` named `name`.
fn click<D: Driver>(driver: &mut D, kind: Role, name: &str) {
  let found = role(driver, kind, name);

  driver.act(&found, &Action::Click);
}

/// Presses the key `key` (with `modifiers`) at the focus, `found` anchoring the action.
fn press<D: Driver>(driver: &mut D, found: &D::Found, key: &str, modifiers: Modifiers) {
  driver.act(found, &Action::Press(key.into(), modifiers));
}

/// Clicks into the text entry named `name` and types `typed` at its end.
fn type_into<D: Driver>(driver: &mut D, name: &str, typed: &str) -> D::Found {
  let field = role(driver, Role::TextField, name);

  driver.act(&field, &Action::Click);

  press(
    driver,
    &field,
    "End",
    Modifiers {
      control: true,
      ..Modifiers::default()
    },
  );

  driver.act(&field, &Action::Type(typed.into()));

  field
}

fn toggle_accepts<D: Driver>(driver: &mut D) {
  driver.open("toggles");

  assert_eq!(
    read(driver, Role::Switch, "Wi-Fi").checked,
    Some(Checked::Off)
  );

  click(driver, Role::Switch, "Wi-Fi");

  assert_eq!(
    read(driver, Role::Switch, "Wi-Fi").checked,
    Some(Checked::On)
  );
  assert_eq!(probe(driver, "wifi"), "true");
}

fn toggle_refuses<D: Driver>(driver: &mut D) {
  driver.open("toggles");
  click(driver, Role::Switch, "Locked");

  assert_eq!(
    read(driver, Role::Switch, "Locked").checked,
    Some(Checked::Off)
  );
  assert_eq!(probe(driver, "locked"), "false");
  assert_eq!(probe(driver, "locked asked"), "1");
}

fn text_types<D: Driver>(driver: &mut D) {
  driver.open("text");

  let field = type_into(driver, "Name", "Ada");
  let snapshot = driver.read(&field);

  assert_eq!(snapshot.text.as_deref(), Some("Ada"));
  assert_eq!(probe(driver, "name"), "Ada");
  assert_eq!(
    snapshot
      .selection
      .map(|selection| (selection.start, selection.end)),
    Some((3, 3))
  );
}

fn text_transforms<D: Driver>(driver: &mut D) {
  driver.open("text");

  let field = type_into(driver, "Code", "ab1c");
  let snapshot = driver.read(&field);

  assert_eq!(snapshot.text.as_deref(), Some("ABC"));
  assert_eq!(probe(driver, "code"), "ABC");
  assert_eq!(snapshot.selection.map(|selection| selection.end), Some(3));
}

fn text_refuses<D: Driver>(driver: &mut D) {
  driver.open("text");

  let field = type_into(driver, "Letters", "axb");

  assert_eq!(driver.read(&field).text.as_deref(), Some("ab"));
  assert_eq!(probe(driver, "letters"), "ab");
}

fn text_composes<D: Driver>(driver: &mut D) {
  driver.open("text");

  let field = type_into(driver, "Name", "caf");

  driver.act(&field, &Action::Compose('é'));
  driver.act(&field, &Action::Commit);
  assert_eq!(driver.read(&field).text.as_deref(), Some("café"));
  assert_eq!(probe(driver, "name"), "café");

  let code = type_into(driver, "Code", "caf");

  driver.act(&code, &Action::Compose('é'));
  driver.act(&code, &Action::Commit);
  assert_eq!(
    driver.read(&code).text.as_deref(),
    Some("CAFÉ"),
    "the handler's truth stands once the composition ends"
  );
  assert_eq!(probe(driver, "code"), "CAFÉ");
}

fn slider_steps_and_clamps<D: Driver>(driver: &mut D) {
  driver.open("sliders");

  let volume = role(driver, Role::Slider, "Volume");

  driver.act(&volume, &Action::Click);
  press(driver, &volume, "End", Modifiers::default());
  assert_eq!(driver.read(&volume).value, Some(100.));
  press(driver, &volume, "Left", Modifiers::default());
  assert_eq!(driver.read(&volume).value, Some(99.));
  assert_eq!(probe(driver, "volume"), "99");

  let limited = role(driver, Role::Slider, "Limited");

  driver.act(&limited, &Action::Click);
  press(driver, &limited, "End", Modifiers::default());
  assert_eq!(driver.read(&limited).value, Some(40.));
  assert_eq!(probe(driver, "limited"), "40");
}

fn picker_chooses<D: Driver>(driver: &mut D) {
  driver.open("picker");
  click(driver, Role::ComboBox, "Size");
  assert_eq!(read(driver, Role::ComboBox, "Size").expanded, Some(true));
  click(driver, Role::Option, "Large");

  assert_eq!(probe(driver, "size"), "Some(2)");
  assert!(
    driver
      .find(&Query::Role(Role::Option, "Medium".into()), Duration::ZERO)
      .is_empty(),
    "the list closed on the choice"
  );
  assert_eq!(read(driver, Role::ComboBox, "Size").expanded, Some(false));
}

fn control_owns_its_press<D: Driver>(driver: &mut D) {
  driver.open("box");
  click(driver, Role::Button, "Inside");

  assert_eq!(probe(driver, "inside"), "1");
  assert_eq!(
    probe(driver, "box presses"),
    "0",
    "the button owns its press"
  );

  let body = text(driver, "Card body");

  driver.act(&body, &Action::Click);
  assert_eq!(probe(driver, "box presses"), "1");
  assert_eq!(probe(driver, "inside"), "1");
}

fn focus_order<D: Driver>(driver: &mut D) {
  driver.open("focus");

  let first = role(driver, Role::TextField, "First");

  driver.act(&first, &Action::Click);

  let mut order = Vec::new();

  for _ in 0..3 {
    press(driver, &first, "Tab", Modifiers::default());

    let focused = one(driver, &Query::Focused);

    order.push(driver.read(&focused).name.to_string());
  }

  press(
    driver,
    &first,
    "Tab",
    Modifiers {
      shift: true,
      ..Modifiers::default()
    },
  );

  let back = one(driver, &Query::Focused);

  assert_eq!(order, ["Second", "Third", "Fourth"]);
  assert_eq!(driver.read(&back).name.as_ref(), "Third");
}

/// The color at the center of `found`'s bounds, as `0xrrggbb`.
fn color_at<D: Driver>(driver: &mut D, found: &D::Found) -> u32 {
  let bounds = driver.read(found).bounds;
  let shot = driver.screenshot();
  let (x, y) = (
    (bounds.x + bounds.width / 2.) as usize,
    (bounds.y + bounds.height / 2.) as usize,
  );
  let at = (y * shot.width as usize + x) * 4;

  u32::from_be_bytes([0, shot.rgba[at], shot.rgba[at + 1], shot.rgba[at + 2]])
}

fn dark_reinks<D: Driver>(driver: &mut D) {
  driver.open("scheme");

  let field = type_into(driver, "Kept", "typed");
  let swatch = role(driver, Role::Region, "Swatch");
  let light = color_at(driver, &swatch);

  driver.set_environment(EnvironmentRequest::Scheme(Some(Scheme::Dark)));

  let dark = color_at(driver, &swatch);
  let same_field = role(driver, Role::TextField, "Kept");

  assert_eq!((light, dark), (0xffffff, 0x000000));
  assert!(
    same_field == field,
    "the field is the same node: nothing was rebuilt"
  );
  assert_eq!(driver.read(&same_field).text.as_deref(), Some("typed"));
  assert_eq!(probe(driver, "kept"), "typed");

  driver.set_environment(EnvironmentRequest::Scheme(None));
  assert_eq!(color_at(driver, &swatch), 0xffffff);
}

fn width_condition<D: Driver>(driver: &mut D) {
  driver.open("width");
  driver.set_environment(EnvironmentRequest::Size(400., 300.));

  let inset = |driver: &mut D| {
    let frame = read(driver, Role::Region, "Frame").bounds;
    let inner = read(driver, Role::Region, "Inner").bounds;

    inner.x - frame.x
  };

  assert_eq!(inset(driver), 8.);
  text(driver, "narrow");

  driver.set_environment(EnvironmentRequest::Size(800., 300.));
  assert_eq!(inset(driver), 32.);
  text(driver, "wide");
}

fn library_controlled<D: Driver>(driver: &mut D) {
  driver.open("library");
  assert!(!read(driver, Role::Region, "Due").name.is_empty());
  assert_eq!(probe(driver, "due"), "2026-10-14");

  click(driver, Role::Button, "20");
  assert_eq!(probe(driver, "due"), "2026-10-20");
  assert_eq!(probe(driver, "picks"), "1");

  click(driver, Role::Button, "17");
  assert_eq!(probe(driver, "due"), "2026-10-20", "a weekend is refused");
  assert_eq!(probe(driver, "picks"), "2");
}

fn focus_follows_scene<D: Driver>(driver: &mut D) {
  driver.open("order");

  let start = role(driver, Role::TextField, "Start");

  driver.act(&start, &Action::Click);

  let mut order = Vec::new();

  for _ in 0..3 {
    press(driver, &start, "Tab", Modifiers::default());

    let focused = one(driver, &Query::Focused);

    order.push(driver.read(&focused).name.to_string());
  }

  let (one_bounds, two_bounds) = (
    read(driver, Role::Button, "One").bounds,
    read(driver, Role::Button, "Two").bounds,
  );

  assert!(
    two_bounds.x < one_bounds.x,
    "the page draws Two before One: {one_bounds:?} {two_bounds:?}"
  );
  assert_eq!(
    order,
    ["One", "Two", "Three"],
    "the scene's order, not the drawing's"
  );
}

fn row_holds_controls<D: Driver>(driver: &mut D) {
  driver.open("rows");

  let row = role(driver, Role::Row, "Inbox");

  driver.act(&row, &Action::Click);
  assert_eq!(probe(driver, "opened"), "1");

  click(driver, Role::Switch, "Mute Inbox");
  assert_eq!(probe(driver, "muted"), "true");
  assert_eq!(probe(driver, "opened"), "1", "the switch owns its press");

  let start = role(driver, Role::TextField, "Before");

  driver.act(&start, &Action::Click);
  press(driver, &start, "Tab", Modifiers::default());
  assert!(driver.read(&row).focused, "the row is a tab stop");
  press(driver, &row, "Enter", Modifiers::default());
  assert_eq!(probe(driver, "opened"), "2");
  press(driver, &row, "space", Modifiers::default());
  assert_eq!(probe(driver, "opened"), "3");
  press(driver, &row, "Tab", Modifiers::default());

  let switch = role(driver, Role::Switch, "Mute Inbox");

  assert!(
    driver.read(&switch).focused,
    "the row's control is the next stop"
  );
}

fn shortcut<D: Driver>(driver: &mut D) {
  driver.open("shortcut");

  let field = type_into(driver, "Title", "Draft");

  press(
    driver,
    &field,
    "s",
    Modifiers {
      control: true,
      ..Modifiers::default()
    },
  );
  assert_eq!(probe(driver, "saved"), "1");
  assert_eq!(
    driver.read(&field).text.as_deref(),
    Some("Draft"),
    "the shortcut typed nothing"
  );
}

fn painted_description<D: Driver>(driver: &mut D) {
  driver.open("painted");
  role(driver, Role::Image, "Sales by half");

  let images = driver
    .outline()
    .lines()
    .filter(|line| line.trim_start().starts_with("image"))
    .count();

  assert_eq!(
    images, 1,
    "an undescribed picture is decoration a reader skips"
  );
}

fn audit<D: Driver>(driver: &mut D) {
  for page in pages::NAMES {
    driver.open(page);

    let issues = driver.audit();

    assert!(issues.is_empty(), "{page}: {issues:#?}");
  }
}
