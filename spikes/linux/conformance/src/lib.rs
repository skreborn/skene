//! # skene-conformance
//!
//! One suite of scenarios every realization passes, written once against the testing
//! [`Driver`]: each opens a page of the suite (a view built with skene-gui's shared views,
//! publishing probes of the signals it binds), acts as a user acts, and checks what a reader
//! finds. A divergence a scenario finds is a bug in the realization, never a special case in
//! the suite. On Linux the suite runs through the Linux driver (skene-conformance-linux), on the
//! window Skene draws, in each of the display runner's sessions (X11, headless mutter, sway,
//! weston, KWin).
//!
//! The pages ([`pages`]) and the scenarios ([`scenarios`]) are public, so another driver runs
//! the same suite against the same pages.

pub mod pages;

use std::time::Duration;

use skene_gui::testing::{
  AccessSnapshot, Action, Checked, Driver, EnvironmentRequest, Frame, Query, Role, Screenshot,
};
use skene_gui::{AccessRequest, Modifiers, Rect, Scheme};

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
    (
      "a_painted_view_draws_in_its_first_frame",
      painted_draws::<D>,
    ),
    (
      "a_button_acts_on_enter_at_once_and_on_space_at_its_release",
      button_keys::<D>,
    ),
    ("a_checkbox_face_follows_its_control", checkbox_face::<D>),
    ("a_switch_thumb_moves_by_paint_alone", switch_thumb::<D>),
    (
      "a_focus_ring_is_a_style_a_button_can_change",
      ring_style::<D>,
    ),
    (
      "a_disabled_control_takes_no_focus_and_dims",
      disabled_control::<D>,
    ),
    (
      "a_library_restyles_a_built_in_from_its_palette",
      library_restyles::<D>,
    ),
    ("a_theme_redefines_the_system_accent", theme_accent::<D>),
    (
      "a_picker_opens_real_options_a_reader_focuses",
      picker_opens::<D>,
    ),
    (
      "a_picker_is_as_wide_as_its_widest_option",
      picker_width::<D>,
    ),
    ("a_caption_beside_a_field_focuses_it", caption_focuses::<D>),
    ("a_slider_drag_moves_its_fill_and_thumb", slider_drag::<D>),
    ("an_editor_scrolls_itself", editor_scrolls::<D>),
    ("a_third_party_control_is_first_class", third_party::<D>),
    (
      "what_is_drawn_later_covers_a_text_drawn_before",
      stacking::<D>,
    ),
    (
      "a_gradient_lies_over_its_box_and_follows_its_tokens",
      gradients::<D>,
    ),
    (
      "a_condition_flipping_runs_its_transition_on_the_frame_clock",
      transition_on_condition::<D>,
    ),
    (
      "a_redefined_token_runs_its_transition",
      transition_on_token::<D>,
    ),
    (
      "a_derived_write_runs_its_transition_outside_the_graph",
      transition_on_derived::<D>,
    ),
    (
      "a_text_color_transition_repaints_its_text",
      transition_on_text::<D>,
    ),
    ("reduced_motion_cuts_a_transition", reduced_motion::<D>),
    (
      "a_keyframe_animation_plays_once_from_when_its_box_shows",
      keyframe_animation::<D>,
    ),
    ("nothing_in_motion_paints_no_frame", idle::<D>),
    (
      "a_caption_hovers_its_control_and_a_box_hovers_alone",
      caption_hover::<D>,
    ),
    (
      "a_window_opens_with_its_autofocus_control_focused_or_none",
      autofocus::<D>,
    ),
    (
      "a_popup_focuses_its_autofocus_control_and_closing_returns_focus",
      popup_focus::<D>,
    ),
    (
      "a_press_on_content_overflowing_its_control_reaches_the_control",
      overflow::<D>,
    ),
    (
      "a_picker_holding_a_value_no_option_carries_shows_none",
      unchosen::<D>,
    ),
    (
      "a_derived_list_reorders_while_open_focus_staying_on_its_option",
      reorder::<D>,
    ),
    (
      "a_list_box_is_one_tab_stop_choosing_one_or_several",
      listbox::<D>,
    ),
    (
      "a_lazy_list_tells_the_whole_and_tab_walks_past_its_window",
      lazy_list::<D>,
    ),
    (
      "a_long_list_box_chooses_options_it_has_not_built",
      long_listbox::<D>,
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

  assert_eq!(probe(driver, "size"), "Large");
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

  driver.screenshot().color_at(
    (bounds.x + bounds.width / 2.).trunc(),
    (bounds.y + bounds.height / 2.).trunc(),
  )
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

/// The color at `x`, `y` of the window drawn, as `0xrrggbb`.
fn pixel<D: Driver>(driver: &mut D, x: f32, y: f32) -> u32 {
  driver.screenshot().color_at(x.trunc(), y.trunc())
}

fn painted_draws<D: Driver>(driver: &mut D) {
  driver.open("painted");

  let chart = role(driver, Role::Image, "Sales by half");
  let bounds = driver.read(&chart).bounds;
  let middle = bounds.y + bounds.height / 2.;

  assert_eq!(
    pixel(driver, bounds.x + bounds.width / 4., middle),
    0xff0000,
    "the left half is red"
  );
  assert_eq!(
    pixel(driver, bounds.x + bounds.width * 3. / 4., middle),
    0x0000ff,
    "the right half is blue, an image brush"
  );
}

/// The platform's accent where the reader states none, as `0xrrggbb`.
const ACCENT: u32 = 0x35_84_e4;
/// A thumb's color, as `0xrrggbb`.
const KNOB: u32 = 0xff_ff_ff;
/// A switch's track while off and a slider's track, as `0xrrggbb`.
const TRACK: u32 = 0xd1_d1_d6;

/// The vertical middle of `found`'s bounds.
fn middle_of<D: Driver>(driver: &mut D, found: &D::Found) -> f32 {
  let bounds = driver.read(found).bounds;

  bounds.y + bounds.height / 2.
}

/// Moves focus to `found` from the field `start` by `Tab`, as a user does.
fn tab_from<D: Driver>(driver: &mut D, start: &str, found: &D::Found) {
  let field = role(driver, Role::TextField, start);

  driver.act(&field, &Action::Click);
  press(driver, &field, "Tab", Modifiers::default());
  assert!(driver.read(found).focused, "Tab reaches it");
}

fn button_keys<D: Driver>(driver: &mut D) {
  driver.open("buttons");

  let act = role(driver, Role::Button, "Act");

  tab_from(driver, "Start", &act);
  press(driver, &act, "Enter", Modifiers::default());
  assert_eq!(probe(driver, "acts"), "1", "Enter activates it once");
  press(driver, &act, "space", Modifiers::default());
  assert_eq!(
    probe(driver, "acts"),
    "2",
    "Space activates it once, at its release"
  );
}

fn checkbox_face<D: Driver>(driver: &mut D) {
  driver.open("checkbox");

  let check = role(driver, Role::Checkbox, "Remember me");
  let bounds = driver.read(&check).bounds;

  // The face's top leading corner, inside its border and clear of its mark.
  let (face_x, face_y) = (bounds.x + 4., bounds.y + 4.);

  assert_eq!(driver.read(&check).checked, Some(Checked::Off));
  assert_eq!(pixel(driver, face_x, face_y), 0xff_ff_ff);

  driver.act(&check, &Action::ClickAt(0.05, 0.5));
  assert_eq!(driver.read(&check).checked, Some(Checked::On));
  assert_eq!(probe(driver, "remember"), "true");
  assert_eq!(
    pixel(driver, face_x, face_y),
    ACCENT,
    "the face follows the control's check"
  );

  let caption = text(driver, "Remember me");

  driver.act(&caption, &Action::Click);
  assert_eq!(
    driver.read(&check).checked,
    Some(Checked::Off),
    "a click on the caption flips it"
  );
}

fn switch_thumb<D: Driver>(driver: &mut D) {
  driver.open("toggles");

  let wifi = role(driver, Role::Switch, "Wi-Fi");
  let bounds = driver.read(&wifi).bounds;
  let middle = middle_of(driver, &wifi);
  let right = bounds.x + bounds.width;
  let (leading, trailing) = (right - 32., right - 12.);

  assert_eq!(pixel(driver, leading, middle), KNOB, "off, the thumb leads");
  assert_eq!(pixel(driver, trailing, middle), TRACK);

  driver.act(&wifi, &Action::Click);
  assert_eq!(driver.read(&wifi).checked, Some(Checked::On));
  press(driver, &wifi, "space", Modifiers::default());
  assert_eq!(driver.read(&wifi).checked, Some(Checked::Off));
  assert_eq!(pixel(driver, leading, middle), KNOB);

  press(driver, &wifi, "space", Modifiers::default());
  assert_eq!(driver.read(&wifi).checked, Some(Checked::On));
  assert_eq!(
    pixel(driver, trailing, middle),
    KNOB,
    "on, the thumb trails"
  );
  assert_eq!(pixel(driver, leading, middle), ACCENT);
}

fn ring_style<D: Driver>(driver: &mut D) {
  driver.open("rings");

  let ringed = role(driver, Role::Button, "Ringed");

  tab_from(driver, "Start", &ringed);

  let bounds = driver.read(&ringed).bounds;
  let middle = middle_of(driver, &ringed);

  assert_eq!(
    pixel(driver, bounds.x - 1., middle),
    ACCENT,
    "the default ring shows where focus does"
  );

  let start = role(driver, Role::TextField, "Start");

  press(driver, &start, "Tab", Modifiers::default());

  let plain = role(driver, Role::Button, "Plain");

  assert!(driver.read(&plain).focused);

  let bounds = driver.read(&plain).bounds;
  let middle = middle_of(driver, &plain);
  let page = pixel(driver, bounds.x - 8., middle);

  assert_eq!(
    pixel(driver, bounds.x - 1., middle),
    page,
    "a ring set to none shows none"
  );
}

fn disabled_control<D: Driver>(driver: &mut D) {
  driver.open("disabled");

  let off = role(driver, Role::Checkbox, "Off");
  let on = role(driver, Role::Checkbox, "On");

  assert!(!driver.read(&off).enabled, "a reader finds it disabled");
  tab_from(driver, "Start", &on);

  let (off_bounds, on_bounds) = (driver.read(&off).bounds, driver.read(&on).bounds);
  let (off_middle, on_middle) = (middle_of(driver, &off), middle_of(driver, &on));
  let dimmed = pixel(driver, off_bounds.x, off_middle).to_be_bytes();
  let enabled = pixel(driver, on_bounds.x, on_middle).to_be_bytes();

  assert!(
    dimmed[1..]
      .iter()
      .zip(&enabled[1..])
      .all(|(dimmed, enabled)| dimmed > enabled),
    "the disabled face's edge is lighter: {dimmed:?} {enabled:?}"
  );
}

fn library_restyles<D: Driver>(driver: &mut D) {
  driver.open("simple");

  let read_inks = |driver: &mut D| {
    let save = role(driver, Role::Button, "Save");
    let agree = role(driver, Role::Checkbox, "Agree");
    let plain = role(driver, Role::Switch, "Plain");
    let (save_bounds, agree_bounds, plain_bounds) = (
      driver.read(&save).bounds,
      driver.read(&agree).bounds,
      driver.read(&plain).bounds,
    );
    let middles = (
      middle_of(driver, &save),
      middle_of(driver, &agree),
      middle_of(driver, &plain),
    );

    [
      pixel(driver, save_bounds.x + 4., middles.0),
      pixel(driver, agree_bounds.x + 4., agree_bounds.y + 4.),
      pixel(driver, plain_bounds.x + plain_bounds.width - 40., middles.2),
    ]
  };

  assert_eq!(
    read_inks(driver),
    [0x0a_84_ff; 3],
    "Simple's button, checkbox and a plain switch take Simple's accent"
  );

  driver.set_environment(EnvironmentRequest::Scheme(Some(Scheme::Dark)));
  assert_eq!(read_inks(driver), [0x40_9c_ff; 3], "and its dark variant");
  driver.set_environment(EnvironmentRequest::Scheme(None));
}

fn theme_accent<D: Driver>(driver: &mut D) {
  driver.open("accent");

  let field = type_into(driver, "Note", "kept");
  let inks = |driver: &mut D| {
    let sync = role(driver, Role::Switch, "Sync");
    let mail = role(driver, Role::Checkbox, "Mail");
    let level = role(driver, Role::Slider, "Level");
    let (sync_bounds, mail_bounds, level_bounds) = (
      driver.read(&sync).bounds,
      driver.read(&mail).bounds,
      driver.read(&level).bounds,
    );
    let middles = (
      middle_of(driver, &sync),
      middle_of(driver, &mail),
      middle_of(driver, &level),
    );

    [
      pixel(driver, sync_bounds.x + sync_bounds.width - 40., middles.0),
      pixel(driver, mail_bounds.x + 4., mail_bounds.y + 4.),
      pixel(driver, level_bounds.x + level_bounds.width / 2., middles.2),
    ]
  };

  assert_eq!(
    inks(driver),
    [0xff_95_00; 3],
    "the theme's accent inks them"
  );
  click(driver, Role::Button, "Flip accent");
  assert_eq!(
    inks(driver),
    [0x34_c7_59; 3],
    "a theme's change re-inks them"
  );

  let same = role(driver, Role::TextField, "Note");

  assert!(
    same == field,
    "the field is the same node: nothing was rebuilt"
  );
  assert_eq!(driver.read(&same).text.as_deref(), Some("kept"));
  assert_eq!(probe(driver, "note"), "kept");
}

/// The name of the node with focus.
fn focused_name<D: Driver>(driver: &mut D) -> String {
  let focused = one(driver, &Query::Focused);

  driver.read(&focused).name.to_string()
}

fn picker_opens<D: Driver>(driver: &mut D) {
  driver.open("fruit");

  let face = role(driver, Role::ComboBox, "Fruit");

  driver.act(&face, &Action::Click);
  assert_eq!(driver.read(&face).expanded, Some(true));
  role(driver, Role::ListBox, "");

  for name in ["Apple", "Banana", "Cherry", "Date", "Elderberry"] {
    role(driver, Role::Option, name);
  }

  assert_eq!(
    focused_name(driver),
    "Banana",
    "focus is on the chosen option"
  );
  press(driver, &face, "Down", Modifiers::default());
  assert_eq!(focused_name(driver), "Cherry");
  assert_eq!(probe(driver, "fruit"), "Banana", "a move chooses nothing");
  press(driver, &face, "Enter", Modifiers::default());
  assert_eq!(probe(driver, "fruit"), "Cherry");
  assert_eq!(driver.read(&face).expanded, Some(false));
  assert_eq!(focused_name(driver), "Fruit", "focus is back on the face");

  driver.act(&face, &Action::Click);
  press(driver, &face, "Down", Modifiers::default());
  press(driver, &face, "Escape", Modifiers::default());
  assert_eq!(driver.read(&face).expanded, Some(false));
  assert_eq!(probe(driver, "fruit"), "Cherry", "Escape chooses nothing");
  assert_eq!(focused_name(driver), "Fruit");

  press(driver, &face, "e", Modifiers::default());
  assert_eq!(
    probe(driver, "fruit"),
    "Elderberry",
    "typing on the closed face chooses by name"
  );

  driver.act(&face, &Action::Click);
  assert_eq!(driver.read(&face).expanded, Some(true));

  let outside = text(driver, "Pick one");

  driver.act(&outside, &Action::Click);
  assert_eq!(
    driver.read(&face).expanded,
    Some(false),
    "a press outside closes the list"
  );
  assert_eq!(probe(driver, "fruit"), "Elderberry", "and chooses nothing");
}

fn picker_width<D: Driver>(driver: &mut D) {
  driver.open("fruit");

  let face = role(driver, Role::ComboBox, "Fruit");
  let before = driver.read(&face).bounds.width;

  driver.act(&face, &Action::Click);
  press(driver, &face, "End", Modifiers::default());
  press(driver, &face, "Enter", Modifiers::default());
  assert_eq!(probe(driver, "fruit"), "Elderberry");

  let after = driver.read(&face).bounds.width;

  assert_eq!(
    before, after,
    "the face is as wide as its widest option whichever it shows"
  );
  assert!(before > 100., "{before}");
}

fn caption_focuses<D: Driver>(driver: &mut D) {
  driver.open("caption");

  let caption = text(driver, "Email");

  driver.act(&caption, &Action::Click);

  let field = role(driver, Role::TextField, "Email");

  assert!(
    driver.read(&field).focused,
    "a press on the caption focuses its field"
  );
  driver.act(&field, &Action::Type("a".into()));
  assert_eq!(probe(driver, "email"), "a");
}

fn slider_drag<D: Driver>(driver: &mut D) {
  driver.open("sliders");

  let volume = role(driver, Role::Slider, "Volume");
  let bounds = driver.read(&volume).bounds;
  let middle = middle_of(driver, &volume);
  let past = bounds.x + 9. + (bounds.width - 18.) * 0.42;
  let thumb = bounds.x + 9. + (bounds.width - 18.) * 0.3;

  assert_eq!(pixel(driver, past, middle), TRACK, "the fill ends at 30");
  assert_eq!(
    pixel(driver, thumb, middle - 1.),
    KNOB,
    "the thumb is centered on the track, covering it at 30"
  );
  driver.act(&volume, &Action::Drag(60., 0.));

  let value = driver.read(&volume).value.unwrap_or_default();

  assert!(value > 50., "the drag moved it past the middle: {value}");
  assert_eq!(pixel(driver, past, middle), ACCENT, "the fill followed");
}

fn editor_scrolls<D: Driver>(driver: &mut D) {
  driver.open("editor");

  let notes = role(driver, Role::TextEditor, "Notes");
  let caption = text(driver, "Notes");
  let lines: String = (1..=30).map(|line| format!("line {line}\n")).collect();

  driver.act(&notes, &Action::Click);
  driver.act(&notes, &Action::Type(lines.trim_end().into()));

  let snapshot = driver.read(&notes);
  let length = snapshot
    .text
    .as_deref()
    .map_or(0, |text| text.chars().count());

  assert_eq!(
    snapshot.selection.map(|selection| selection.end),
    Some(length),
    "the caret is at the end"
  );
  assert_eq!(
    snapshot.bounds.height, 120.,
    "it stops growing at its greatest height"
  );

  let caption_before = driver.read(&caption).bounds;

  let inside = |driver: &mut D| {
    let shot = driver.screenshot();
    let mut rows = Vec::new();

    for y in (snapshot.bounds.y as usize + 10)..(snapshot.bounds.y as usize + 100) {
      for x in (snapshot.bounds.x as usize + 10)..(snapshot.bounds.x as usize + 70) {
        rows.extend_from_slice(&shot.rgba_at(x as f32, y as f32));
      }
    }

    rows
  };
  let at_end = inside(driver);

  driver.act(&notes, &Action::Scroll(0, -5));
  assert!(inside(driver) != at_end, "the wheel scrolls its text");
  assert_eq!(
    driver.read(&notes).bounds,
    snapshot.bounds,
    "its box stays put"
  );
  assert_eq!(
    driver.read(&caption).bounds,
    caption_before,
    "its caption stays put"
  );
}

fn third_party<D: Driver>(driver: &mut D) {
  driver.open("rating");

  let rating = role(driver, Role::Slider, "Rating");
  let before = role(driver, Role::Button, "Before");

  driver.act(&before, &Action::Click);
  press(driver, &before, "Tab", Modifiers::default());
  assert!(driver.read(&rating).focused, "it is a tab stop");

  for _ in 0..3 {
    press(driver, &rating, "Right", Modifiers::default());
  }

  assert_eq!(driver.read(&rating).value, Some(3.), "the slider's keys");
  assert_eq!(probe(driver, "stars"), "3");

  driver.act(&rating, &Action::ClickAt(0.9, 0.5));
  assert_eq!(
    driver.read(&rating).value,
    Some(5.),
    "a press on the fifth star"
  );

  driver.act(&rating, &Action::Access(AccessRequest::SetValue(2.)));
  assert_eq!(driver.read(&rating).value, Some(2.), "a reader's request");

  press(driver, &rating, "Tab", Modifiers::default());
  assert_eq!(focused_name(driver), "After", "Tab skips the disabled copy");
  assert!(!read(driver, Role::Slider, "Locked").enabled);
}

fn audit<D: Driver>(driver: &mut D) {
  for page in pages::NAMES {
    driver.open(page);

    let issues = driver.audit();

    assert!(issues.is_empty(), "{page}: {issues:#?}");
  }
}

/// The colors of the screenshot's device pixels inside `bounds`, inset by `inset` on every
/// side, as `0xRRGGBB`.
fn colors_in(shot: &Screenshot, bounds: Rect, inset: f32) -> Vec<u32> {
  let scale = shot.scale as f32;
  let (left, top) = (
    ((bounds.x + inset) * scale).ceil() as usize,
    ((bounds.y + inset) * scale).ceil() as usize,
  );
  let right =
    (((bounds.x + bounds.width - inset) * scale).floor() as usize).min(shot.width as usize);
  let bottom =
    (((bounds.y + bounds.height - inset) * scale).floor() as usize).min(shot.height as usize);
  let mut colors = Vec::new();

  for y in top..bottom {
    for x in left..right {
      let at = (y * shot.width as usize + x) * 4;

      colors.push(u32::from_be_bytes([
        0,
        shot.rgba[at],
        shot.rgba[at + 1],
        shot.rgba[at + 2],
      ]));
    }
  }

  colors
}

fn stacking<D: Driver>(driver: &mut D) {
  driver.open("stacking");

  let covered = read(driver, Role::Region, "Covered").bounds;
  let cover = read(driver, Role::Region, "Cover").bounds;
  let shot = driver.screenshot();
  let under = colors_in(&shot, cover, 1.);
  let beside = colors_in(
    &shot,
    Rect {
      x: cover.x + cover.width + 1.,
      width: covered.width - cover.width - 1.,
      ..covered
    },
    0.,
  );

  assert!(!under.is_empty() && !beside.is_empty());
  assert!(
    under.iter().all(|&color| color == 0x0000ff),
    "the opaque box hides the glyphs beneath it: {} of {} pixels are not its blue",
    under.iter().filter(|&&color| color != 0x0000ff).count(),
    under.len()
  );
  assert!(
    beside.contains(&0xff0000),
    "the text shows where the box does not cover it"
  );

  let scrim = read(driver, Role::Region, "Scrim").bounds;
  let dimmed = colors_in(&shot, scrim, 1.);
  let red = |color: u32| color >> 16;

  assert!(
    dimmed.iter().all(|&color| red(color) <= 0x81),
    "the scrim dims the glyphs beneath it: the brightest red is {:#x}",
    dimmed.iter().map(|&color| red(color)).max().unwrap_or(0)
  );
  assert!(
    dimmed
      .iter()
      .any(|&color| color == 0x7f0000 || color == 0x800000),
    "the dimmed text shows through the scrim"
  );
}

/// Whether `color` is within `tolerance` of `wanted` in every channel.
fn near(color: u32, wanted: u32, tolerance: u32) -> bool {
  (0..3).all(|channel| {
    let (a, b) = (
      (color >> (channel * 8)) & 0xff,
      (wanted >> (channel * 8)) & 0xff,
    );

    a.abs_diff(b) <= tolerance
  })
}

fn gradients<D: Driver>(driver: &mut D) {
  driver.open("gradients");

  let at = |driver: &mut D, name: &str, x: f32, y: f32| {
    let bounds = read(driver, Role::Region, name).bounds;

    pixel(
      driver,
      bounds.x + bounds.width * x,
      bounds.y + bounds.height * y,
    )
  };
  let check = |color: u32, wanted: u32, what: &str| {
    assert!(
      near(color, wanted, 10),
      "{what}: {color:06x}, not {wanted:06x}"
    );
  };

  check(at(driver, "Linear", 0.005, 0.5), 0xff0000, "a line's start");
  check(at(driver, "Linear", 0.5, 0.5), 0x800080, "a line's middle");
  check(at(driver, "Linear", 0.995, 0.5), 0x0000ff, "a line's end");
  check(at(driver, "Radial", 0.5, 0.5), 0xffffff, "the center");
  check(
    at(driver, "Radial", 0.5, 0.005),
    0x4b4b4b,
    "the middle of the edge",
  );
  check(
    at(driver, "Radial", 0.002, 0.002),
    0x000000,
    "the farthest corner",
  );
  check(
    at(driver, "Sweep", 0.52, 0.02),
    0xff0000,
    "the turn's start",
  );
  // Fifty pixels right of and above the center of a box 200 by 120: an eighth of a turn.
  check(
    at(driver, "Sweep", 0.75, 0.5 - 50. / 120.),
    0xbf0040,
    "an eighth of a turn",
  );
  check(at(driver, "Sweep", 0.98, 0.5), 0x800080, "a quarter turn");
  check(at(driver, "Sweep", 0.5, 0.98), 0x0000ff, "half a turn");
  check(
    at(driver, "Bordered", 0.01, 0.5),
    0xfa0005,
    "a border's leading edge",
  );
  check(
    at(driver, "Bordered", 0.99, 0.5),
    0x0500fa,
    "a border's trailing edge",
  );
  check(
    at(driver, "Bordered", 0.5, 0.5),
    0xffffff,
    "inside the border",
  );
  check(
    at(driver, "Themed", 0.005, 0.5),
    0xffffff,
    "a token stop, light",
  );

  driver.set_environment(EnvironmentRequest::Scheme(Some(Scheme::Dark)));
  check(
    at(driver, "Themed", 0.005, 0.5),
    0x000000,
    "a token stop, dark",
  );
  check(
    at(driver, "Themed", 0.995, 0.5),
    0xff0000,
    "a fixed stop, dark",
  );
}

/// The color at the middle of `bounds` in each frame, with when the frame was presented.
fn colors_over(frames: &[Frame], bounds: Rect) -> Vec<(Duration, u32)> {
  frames
    .iter()
    .map(|frame| {
      let (x, y) = (
        (bounds.x + bounds.width / 2.).trunc(),
        (bounds.y + bounds.height / 2.).trunc(),
      );

      (frame.at, frame.shot.color_at(x, y))
    })
    .collect()
}

/// How far `color` is from `other`: the largest difference of a channel.
fn distance(color: u32, other: u32) -> u32 {
  (0..3)
    .map(|channel| ((color >> (channel * 8)) & 0xff).abs_diff((other >> (channel * 8)) & 0xff))
    .max()
    .unwrap_or(0)
}

/// The frames presented while `act` runs (it settles, so a transition runs out), as the colors
/// at the middle of the box named `name`.
fn recorded<D: Driver>(
  driver: &mut D,
  name: &str,
  act: impl FnOnce(&mut D),
) -> Vec<(Duration, u32)> {
  let bounds = read(driver, Role::Region, name).bounds;

  driver.set_recording(true);
  act(driver);

  let frames = driver.take_frames();

  driver.set_recording(false);

  colors_over(&frames, bounds)
}

/// Checks that `colors` moved from `from` to `to` through in-betweens, each nearer `to` than
/// the one before, on the platform's frame clock: the frames between the first in-between and
/// the end come at least 8 and at most 500 a second, half of them at least 2 ms apart, where a
/// loop painting back to back paints a frame in a fraction of a millisecond.
fn moved(colors: &[(Duration, u32)], from: u32, to: u32, what: &str) {
  let summary = || {
    colors
      .iter()
      .map(|(at, color)| format!("{:.0}ms {color:06x}", at.as_secs_f64() * 1000.))
      .collect::<Vec<_>>()
      .join(", ")
  };
  let between: Vec<&(Duration, u32)> = colors
    .iter()
    .filter(|(_, color)| distance(*color, from) > 8 && distance(*color, to) > 8)
    .collect();

  eprintln!("{what}: {}", summary());

  assert_eq!(
    colors.last().map(|(_, color)| *color),
    Some(to),
    "{what}: it ends at {to:06x}: {}",
    summary()
  );
  assert!(
    between.len() >= 3,
    "{what}: it moves through in-betweens: {}",
    summary()
  );
  assert!(
    between
      .windows(2)
      .all(|pair| distance(pair[1].1, to) <= distance(pair[0].1, to) + 2),
    "{what}: each frame nearer its end: {}",
    summary()
  );

  let first = between[0].0;
  let end = colors
    .iter()
    .find(|(at, color)| *at > first && *color == to)
    .map_or(first, |(at, _)| *at);
  let span = end.saturating_sub(first).as_secs_f64();
  let painted = colors
    .iter()
    .filter(|(at, _)| *at >= first && *at <= end)
    .count();
  let rate = painted as f64 / span.max(0.001);

  assert!(
    span >= f64::from(pages::MOTION) / 1000. * 0.5,
    "{what}: it takes its duration, not a cut: {span:.3}s: {}",
    summary()
  );
  assert!(
    (8. ..=500.).contains(&rate),
    "{what}: {painted} frames in {span:.3}s, {rate:.0} a second: {}",
    summary()
  );

  let mut gaps: Vec<Duration> = colors
    .windows(2)
    .filter(|pair| pair[0].0 >= first && pair[1].0 <= end)
    .map(|pair| pair[1].0.saturating_sub(pair[0].0))
    .collect();

  gaps.sort();

  assert!(
    gaps
      .get(gaps.len() / 2)
      .is_some_and(|gap| *gap >= Duration::from_millis(2)),
    "{what}: frames paced by the clock, not painted back to back: {gaps:?}"
  );
}

fn transition_on_condition<D: Driver>(driver: &mut D) {
  driver.open("motion");

  // The pointer starts off the box, wherever the window opened under it, and the box at rest.
  let warm = role(driver, Role::Button, "Warm");

  driver.act(&warm, &Action::Hover);
  driver.wait(Duration::from_millis(u64::from(pages::MOTION) + 200));

  let hovered = role(driver, Role::Region, "Hovered");
  let colors = recorded(driver, "Hovered", |driver| {
    driver.act(&hovered, &Action::Hover);
  });

  moved(&colors, 0xffffff, 0x000000, "the pointer over the box");
}

fn transition_on_token<D: Driver>(driver: &mut D) {
  driver.open("motion");

  let retheme = role(driver, Role::Button, "Retheme");
  let before = driver.probe("themed");
  let colors = recorded(driver, "Themed", |driver| {
    driver.act(&retheme, &Action::Click);
  });

  moved(&colors, 0xffffff, 0x000000, "the swatch token redefined");
  assert_eq!(
    driver
      .probe("themed")
      .and_then(|count| count.parse::<u32>().ok()),
    before
      .and_then(|count| count.parse::<u32>().ok())
      .map(|count| count + 1),
    "the theme derived its palette once, not once a frame"
  );
}

fn transition_on_derived<D: Driver>(driver: &mut D) {
  driver.open("motion");

  let warm = role(driver, Role::Button, "Warm");
  let before = driver.probe("derived");
  let colors = recorded(driver, "Derived", |driver| {
    driver.act(&warm, &Action::Click);
  });

  moved(&colors, 0xffffff, 0xff0000, "a derived fill written");
  assert_eq!(
    driver
      .probe("derived")
      .and_then(|count| count.parse::<u32>().ok()),
    before
      .and_then(|count| count.parse::<u32>().ok())
      .map(|count| count + 1),
    "the fill was derived once, not once a frame"
  );
}

fn reduced_motion<D: Driver>(driver: &mut D) {
  driver.open("motion");
  driver.set_environment(EnvironmentRequest::ReducedMotion(true));

  let warm = role(driver, Role::Button, "Warm");
  let colors = recorded(driver, "Derived", |driver| {
    driver.act(&warm, &Action::Click);
  });

  driver.set_environment(EnvironmentRequest::ReducedMotion(false));
  assert_eq!(
    colors.last().map(|(_, color)| *color),
    Some(0xff0000),
    "{colors:?}"
  );
  assert!(
    colors
      .iter()
      .all(|(_, color)| *color == 0xffffff || *color == 0xff0000),
    "a reader preferring reduced motion sees the change as a cut: {colors:?}"
  );
}

fn keyframe_animation<D: Driver>(driver: &mut D) {
  driver.set_recording(true);
  driver.open("keyframes");

  let frames = driver.take_frames();

  driver.set_recording(false);

  let bounds = read(driver, Role::Region, "Fading").bounds;
  let colors = colors_over(&frames, bounds);

  moved(&colors, 0x000000, 0xffffff, "a keyframe animation");
}

fn idle<D: Driver>(driver: &mut D) {
  driver.open("motion");

  let hovered = role(driver, Role::Region, "Hovered");

  driver.act(&hovered, &Action::Hover);
  driver.set_recording(true);
  driver.wait(Duration::from_millis(600));

  let frames = driver.take_frames();

  driver.set_recording(false);
  assert!(
    frames.is_empty(),
    "a window with nothing in motion paints nothing: {} frames in 600 ms",
    frames.len()
  );
}

fn transition_on_text<D: Driver>(driver: &mut D) {
  driver.open("motion");

  let bounds = read(driver, Role::Region, "Inked").bounds;
  let warm = role(driver, Role::Button, "Warm");

  driver.set_recording(true);
  driver.act(&warm, &Action::Click);

  let frames = driver.take_frames();

  driver.set_recording(false);

  // The most inked pixel of the text: green stays nothing where black and red cover it all.
  let colors: Vec<(Duration, u32)> = frames
    .iter()
    .map(|frame| {
      let inked = colors_in(&frame.shot, bounds, 0.)
        .into_iter()
        .min_by_key(|color| ((color >> 8) & 0xff, color & 0xff))
        .unwrap_or(0xffffff);

      (frame.at, inked)
    })
    .collect();

  moved(&colors, 0x000000, 0xff0000, "a text's inherited color");
}

/// The platform's field fill, as `0xrrggbb`.
const FIELD: u32 = 0xff_ff_ff;
/// The platform's control fill, as `0xrrggbb`.
const CONTROL: u32 = 0xe8_e8_ed;
/// The platform's control fill under the pointer, as `0xrrggbb`.
const CONTROL_HOVER: u32 = 0xdc_dc_e2;

fn caption_hover<D: Driver>(driver: &mut D) {
  driver.open("hover");

  let check = role(driver, Role::Checkbox, "Remember me");
  let bounds = driver.read(&check).bounds;
  let (face_x, face_y) = (bounds.x + 4., bounds.y + 4.);

  assert_eq!(pixel(driver, face_x, face_y), FIELD);

  let caption = text(driver, "Remember me");

  driver.act(&caption, &Action::Hover);
  assert_eq!(
    pixel(driver, face_x, face_y),
    CONTROL_HOVER,
    "the face shows the control hovered while the pointer is over its caption"
  );

  let picker = role(driver, Role::ComboBox, "Size");
  let bounds = driver.read(&picker).bounds;
  let (face_x, face_y) = (bounds.x + 4., middle_of(driver, &picker));

  assert_eq!(pixel(driver, face_x, face_y), CONTROL);

  let beside = text(driver, "Size");

  driver.act(&beside, &Action::Hover);
  assert_eq!(
    pixel(driver, face_x, face_y),
    CONTROL_HOVER,
    "a caption beside its control hovers the control's own node"
  );

  let cool = role(driver, Role::Radio, "Cool");
  let bounds = driver.read(&cool).bounds;
  let (face_x, face_y) = (bounds.x + 9., middle_of(driver, &cool) - 4.);
  let group = text(driver, "Tone");

  driver.act(&group, &Action::Hover);
  assert_eq!(
    pixel(driver, face_x, face_y),
    FIELD,
    "a radio's face follows its radio, not the group whose caption is hovered"
  );

  let plain = role(driver, Role::Region, "Plain box");
  let bounds = driver.read(&plain).bounds;
  let (x, y) = (bounds.x + 4., bounds.y + 4.);

  assert_eq!(pixel(driver, x, y), FIELD);
  driver.act(&plain, &Action::Hover);
  assert_eq!(
    pixel(driver, x, y),
    0x00_a0_ff,
    "a box outside every control keeps its own hover"
  );
}

fn autofocus<D: Driver>(driver: &mut D) {
  driver.open("focus");

  for (kind, name) in [
    (Role::TextField, "First"),
    (Role::Switch, "Second"),
    (Role::Button, "Third"),
    (Role::Slider, "Fourth"),
  ] {
    assert!(
      !read(driver, kind, name).focused,
      "a window none of whose controls asks for focus opens with nothing focused: {name}"
    );
  }

  driver.open("autofocus");

  let second = role(driver, Role::TextField, "Second");

  assert!(
    driver.read(&second).focused,
    "the control asking for focus has it as the window opens"
  );
  assert!(!read(driver, Role::TextField, "First").focused);

  driver.act(&second, &Action::Type("hi".into()));
  assert_eq!(probe(driver, "second"), "hi", "what is typed arrives there");
}

fn popup_focus<D: Driver>(driver: &mut D) {
  driver.open("popup");
  click(driver, Role::Button, "Open");

  let note = role(driver, Role::TextField, "Note");

  assert!(
    driver.read(&note).focused,
    "the popup's control asking for focus has it as the popup opens"
  );

  driver.act(&note, &Action::Type("x".into()));
  assert_eq!(probe(driver, "note"), "x");

  click(driver, Role::Button, "Done");
  assert_eq!(probe(driver, "open"), "false");
  assert!(
    read(driver, Role::Button, "Open").focused,
    "closing the popup returns focus to what held it when it opened"
  );
}

fn overflow<D: Driver>(driver: &mut D) {
  driver.open("overflow");

  for (name, probed) in [("Overflowing", "overflowing"), ("Clipped", "clipped")] {
    let command = role(driver, Role::Button, name);
    let bounds = driver.read(&command).bounds;
    // The mark's middle, drawn below the command's box.
    let (x, y) = (20. / bounds.width, 1.5);

    driver.act(&command, &Action::ClickAt(x, y));

    let expected = if name == "Overflowing" { "1" } else { "0" };

    assert_eq!(
      probe(driver, probed),
      expected,
      "{name}: a press on its mark below its box reaches it unless a box clips it"
    );
  }

  let round = role(driver, Role::Row, "Round");

  driver.act(&round, &Action::ClickAt(0.05, 0.05));
  assert_eq!(
    (probe(driver, "round"), probe(driver, "beneath")),
    ("0".to_owned(), "1".to_owned()),
    "a press in a round box's corner, outside its circle, reaches what lies beneath"
  );

  driver.act(&round, &Action::Click);
  assert_eq!(
    (probe(driver, "round"), probe(driver, "beneath")),
    ("1".to_owned(), "1".to_owned())
  );
}

/// How many pixels of the face `found` differ from its fill along its middle, from its leading
/// padding to where its mark starts: the ink of the name it shows.
fn inked<D: Driver>(driver: &mut D, found: &D::Found) -> usize {
  let bounds = driver.read(found).bounds;
  let middle = bounds.y + bounds.height / 2.;
  let shot = driver.screenshot();
  let fill = shot.color_at(bounds.x + 4., middle);

  (14..(bounds.width as i32 - 34))
    .filter(|offset| shot.color_at(bounds.x + *offset as f32, middle) != fill)
    .count()
}

fn unchosen<D: Driver>(driver: &mut D) {
  driver.open("unchosen");

  let face = role(driver, Role::ComboBox, "Fruit");

  assert_eq!(
    inked(driver, &face),
    0,
    "a picker holding a value no option carries shows no name"
  );

  driver.act(&face, &Action::Click);
  assert_eq!(driver.read(&face).expanded, Some(true));
  assert_eq!(
    focused_name(driver),
    "Apple",
    "opened with nothing chosen, focus is on the first option"
  );
  click(driver, Role::Option, "Cherry");
  assert_eq!(probe(driver, "fruit"), "Some(Cherry)");
  assert_eq!(driver.read(&face).expanded, Some(false));

  let outside = text(driver, "Pick one");

  driver.act(&outside, &Action::Hover);
  assert!(
    inked(driver, &face) > 0,
    "once chosen, the face shows the option's name"
  );
}

fn reorder<D: Driver>(driver: &mut D) {
  driver.open("reorder");

  let face = role(driver, Role::ComboBox, "Fruit");

  driver.act(&face, &Action::Click);
  assert_eq!(
    focused_name(driver),
    "Banana",
    "focus is on the chosen option"
  );

  let before = read(driver, Role::Option, "Banana").bounds;

  press(
    driver,
    &face,
    "r",
    Modifiers {
      control: true,
      ..Modifiers::default()
    },
  );
  assert_eq!(
    probe(driver, "order"),
    "[Elderberry, Date, Cherry, Banana, Apple]",
    "the shortcut reversed the list while it was open"
  );
  assert_eq!(
    driver.read(&face).expanded,
    Some(true),
    "the list stays open"
  );

  let after = read(driver, Role::Option, "Banana").bounds;

  assert!(
    after.y > before.y,
    "the option moved with its value: from {before:?} to {after:?}"
  );
  assert_eq!(
    focused_name(driver),
    "Banana",
    "focus stays on the option, which was kept"
  );

  press(driver, &face, "Down", Modifiers::default());
  assert_eq!(
    focused_name(driver),
    "Apple",
    "the arrows move in the new order"
  );
  press(driver, &face, "Enter", Modifiers::default());
  assert_eq!(probe(driver, "fruit"), "Apple");
}

fn listbox<D: Driver>(driver: &mut D) {
  driver.open("listbox");

  let favorite = role(driver, Role::ListBox, "Favorite");

  press(driver, &favorite, "Tab", Modifiers::default());
  assert_eq!(
    focused_name(driver),
    "Apple",
    "a list box with nothing chosen is a tab stop on its first option"
  );
  press(driver, &favorite, "Down", Modifiers::default());
  assert_eq!(focused_name(driver), "Banana");
  assert_eq!(probe(driver, "favorite"), "None", "a move chooses nothing");
  press(driver, &favorite, " ", Modifiers::default());
  assert_eq!(probe(driver, "favorite"), "Some(Banana)");

  let basket = role(driver, Role::ListBox, "Basket");

  // The pointer over a list box in the page focuses none of its options: only a popup's list
  // follows the pointer.
  driver.act(&basket, &Action::Hover);
  assert_eq!(
    focused_name(driver),
    "Banana",
    "the pointer over a list box moves no focus"
  );
  press(driver, &favorite, "Tab", Modifiers::default());
  assert_eq!(
    focused_name(driver),
    "Apple",
    "Tab leaves the list box for the next one"
  );

  press(driver, &basket, " ", Modifiers::default());
  press(driver, &basket, "Down", Modifiers::default());
  press(driver, &basket, "Down", Modifiers::default());
  press(driver, &basket, " ", Modifiers::default());
  assert_eq!(probe(driver, "basket"), "{Apple, Cherry}");
  press(driver, &basket, " ", Modifiers::default());
  assert_eq!(
    probe(driver, "basket"),
    "{Apple}",
    "choosing a chosen option of several takes it away"
  );

  press(
    driver,
    &basket,
    "Tab",
    Modifiers {
      shift: true,
      ..Modifiers::default()
    },
  );
  assert_eq!(
    focused_name(driver),
    "Banana",
    "back in a list box, focus returns to the option it was last on"
  );
}

fn lazy_list<D: Driver>(driver: &mut D) {
  driver.open("lazy");

  let items: Vec<AccessSnapshot> = driver
    .find(&Query::Every(Role::ListItem), FIND)
    .iter()
    .map(|item| driver.read(item))
    .collect();
  let places: Vec<Option<usize>> = items.iter().map(|item| item.position_in_set).collect();

  assert!(
    (8..=16).contains(&items.len()),
    "a reader finds about the rows in view, not ten thousand: {places:?}"
  );
  assert!(
    items
      .iter()
      .enumerate()
      .all(|(at, item)| item.position_in_set == Some(at + 1) && item.set_size == Some(10_000)),
    "each realized row tells its place in the whole list: {places:?}"
  );

  let before = role(driver, Role::Button, "Before");

  driver.act(&before, &Action::Click);

  // Twenty stops on, past the rows the list has realized.
  for _ in 0..20 {
    press(driver, &before, "Tab", Modifiers::default());
  }

  assert_eq!(focused_name(driver), "Open 19", "Tab walks past the window");

  let focused = one(driver, &Query::Focused);
  let shown = driver.read(&focused).bounds;
  let list = read(driver, Role::List, "Actions").bounds;

  assert!(
    shown.y >= list.y - 1. && shown.y + shown.height <= list.y + list.height + 1.,
    "the focused row scrolled into view: {shown:?} in {list:?}"
  );

  press(
    driver,
    &focused,
    "Tab",
    Modifiers {
      shift: true,
      ..Modifiers::default()
    },
  );
  assert_eq!(focused_name(driver), "Open 18");
}

fn long_listbox<D: Driver>(driver: &mut D) {
  driver.open("long");

  let built = driver.find(&Query::Every(Role::Option), FIND).len();

  assert!(
    (8..=20).contains(&built),
    "about the options in view are built: {built}"
  );
  assert_eq!(read(driver, Role::Option, "Number 0").set_size, Some(5000));

  click(driver, Role::Option, "Number 0");
  assert_eq!(probe(driver, "chosen"), "1");

  let numbers = role(driver, Role::ListBox, "Numbers");

  press(driver, &numbers, "End", Modifiers::default());
  assert_eq!(focused_name(driver), "Number 4999");

  let focused = one(driver, &Query::Focused);

  assert_eq!(driver.read(&focused).position_in_set, Some(5000));

  press(
    driver,
    &numbers,
    "a",
    Modifiers {
      control: true,
      ..Modifiers::default()
    },
  );
  assert_eq!(
    probe(driver, "chosen"),
    "5000",
    "the primary modifier and A choose every option, built or not"
  );
}
