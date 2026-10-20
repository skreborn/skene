//! A text field under a finger, as GTK 4's entry takes one on a touchscreen: a tap shows the
//! caret's handle below the caret, a finger selecting shows a handle at each end of the
//! selection, a handle dragged moves its end of the selection (never past the other) with the
//! magnifier above the line, a tap on the caret's handle soon after the tap that placed the
//! caret selects the word there and a later tap on a handle asks for the menu; a key or a
//! pointer's press hides the handles. What the handles and the magnifier look like is read from
//! the screen. A leg whose input sends no touch says so and tests nothing of it.

use skene_gui::{
  Color, ContextMenuEvent, MenuSource, Signal,
  prelude::*,
  testing::{self, Driver as _, Found, Role, publish},
};

/// Publishes `signal` as the probe `name`, as `Debug` writes it.
fn probe<T: Clone + std::fmt::Debug + 'static>(name: &'static str, signal: Signal<T>) {
  publish(name, move || format!("{:?}", signal.get_untracked()));
}

/// The words the field holds.
const WORDS: &str = "alpha beta gamma delta";

/// A field holding [`WORDS`] between two pale boxes, room for the magnifier above it and the
/// handles below it, in a box that hears its context menus.
fn page() -> impl View {
  let menus = Signal::new(Vec::<MenuSource>::new());

  probe("menus", menus);

  let pale = |height: i32| {
    Container::new()
      .set_height(height)
      .set_background(Color::rgb(0xf6d860))
      .set_child(Text::new("Pale"))
  };

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(pale(120))
    .add_child(
      // A field with no menu of its own, so the box around it hears its requests.
      Container::new()
        .handle_context_menu(move |event: &ContextMenuEvent| {
          menus.update(|menus| menus.push(event.source));
          Bubbling::Stop
        })
        .set_child(
          TextField::new("Words")
            .set_context_menu(ContextMenu::new())
            .set_text(WORDS),
        ),
    )
    .add_child(pale(160))
}

/// Whether the leg's input sends touch, saying so where it does not.
fn sends_touch(screen: &testing::Screen) -> bool {
  let sends = screen.driver().sends_touch();

  if !sends {
    skene_gui_linux::testing::not_tested_here(format_args!(
      "{} sends no touch",
      screen.driver().input_name()
    ));
  }

  sends
}

/// How many of the pointer's units a pixel of the window is.
fn unit(screen: &testing::Screen) -> f64 {
  screen.driver().units_per_pixel()
}

/// Where the caret before character `offset` of `field` stands, in the pointer's space: its x,
/// and its line's top and height.
fn caret_at(screen: &testing::Screen, field: &Found<'_>, offset: usize) -> (f64, f64, f64) {
  let driver = screen.driver();

  if offset < WORDS.len() {
    let (x, y, _, height) = driver.character_rect(field.element(), offset);

    (x, y, height)
  } else {
    let (x, y, width, height) = driver.character_rect(field.element(), offset - 1);

    (x + width, y, height)
  }
}

/// `field`'s selection: its anchor and its caret.
fn selection(field: &Found<'_>) -> (usize, usize) {
  let selection = field
    .read()
    .selection
    .expect("the field reports its selection");

  (selection.start, selection.end)
}

/// Where a finger takes the handle pointing at `x` on a line from `top`, `height` tall: on its
/// icon, which hangs below the line, `across` pixels from the point.
fn on_handle(
  screen: &testing::Screen,
  (x, top, height): (f64, f64, f64),
  across: f64,
) -> (f64, f64) {
  let unit = unit(screen);

  (x + across * unit, top + height + 13. * unit)
}

/// Hides the handles with a key that changes nothing in the field (`Shift` alone).
fn press_shift(screen: &testing::Screen) {
  screen.driver().hold_key("Shift_L");
  screen.driver().release_key("Shift_L");
}

#[skene_gui::test]
fn a_tap_shows_the_caret_handle_below_the_caret_and_a_key_hides_it() {
  let screen = testing::render(page);

  if !sends_touch(&screen) {
    return;
  }

  let field = screen.find_by_role(Role::TextField, "Words");

  field.tap_at(0.3, 0.5);

  let (anchor, caret) = selection(&field);

  assert_eq!(anchor, caret, "a tap places the caret");

  let unit = unit(&screen);
  let (x, top, height) = caret_at(&screen, &field, caret);
  let area = (
    x - 50. * unit,
    top + height - 8. * unit,
    100. * unit,
    50. * unit,
  );
  let shown = screen.driver().screen_area(area);

  press_shift(&screen);

  let hidden = screen.driver().screen_area(area);
  let handle = crate::changed(area, &hidden, &shown).expect("a tap shows the caret's handle");

  eprintln!("the caret at {x} on a line from {top}, {height} tall; its handle at {handle:?}");
  assert!(
    (handle.0 + handle.2 / 2. - x).abs() <= 2. * unit,
    "the handle is centered under the caret: {handle:?} against {x}"
  );
  assert!(
    (18. * unit..=24. * unit).contains(&handle.2) && (20. * unit..=28. * unit).contains(&handle.3),
    "the handle is GTK's, 20 by 24: {handle:?}"
  );
  assert!(
    (handle.1 - (top + height)).abs() <= 4. * unit,
    "the handle hangs below the line: {handle:?} against {}",
    top + height
  );
  assert_eq!(
    selection(&field),
    (anchor, caret),
    "the key changed nothing else"
  );
}

#[skene_gui::test]
fn the_caret_handle_moves_the_caret_with_the_magnifier_above_the_line() {
  let screen = testing::render(page);

  if !sends_touch(&screen) {
    return;
  }

  let field = screen.find_by_role(Role::TextField, "Words");

  field.tap_at(0.3, 0.5);

  let (_, caret) = selection(&field);
  let unit = unit(&screen);
  let point = caret_at(&screen, &field, caret);
  let grab = on_handle(&screen, point, 0.);
  // Into "delta", the caret before its "l".
  let (target, top, _) = caret_at(&screen, &field, 19);
  let area = (
    target - 90. * unit,
    top - 110. * unit,
    180. * unit,
    112. * unit,
  );

  screen.driver().hold_finger(grab);
  screen
    .driver()
    .move_finger((target + (grab.0 - point.0), grab.1));

  let held = selection(&field);
  let during = screen.driver().screen_area(area);

  screen.driver().lift_finger();

  let after = screen.driver().screen_area(area);
  let magnifier =
    crate::changed(area, &after, &during).expect("the magnifier shows while the finger moves");

  eprintln!("the finger moved the caret to {held:?}; the magnifier at {magnifier:?}");
  assert_eq!(held, (19, 19), "the caret follows the finger's handle");
  assert_eq!(selection(&field), (19, 19), "and stays as the finger lifts");
  assert!(
    (114. * unit..=122. * unit).contains(&magnifier.2)
      && (86. * unit..=94. * unit).contains(&magnifier.3),
    "the magnifier is GTK's, its frame and its tail 118 by 90: {magnifier:?}"
  );
  assert!(
    (magnifier.0 + magnifier.2 / 2. - target).abs() <= 3. * unit,
    "the magnifier is centered on the finger's point: {magnifier:?} against {target}"
  );
  assert!(
    (magnifier.1 + magnifier.3 - top).abs() <= 4. * unit,
    "the magnifier's tail points at the line's top: {magnifier:?} against {top}"
  );
}

#[skene_gui::test]
fn a_finger_selecting_shows_the_magnifier_then_a_handle_at_each_end() {
  let screen = testing::render(page);

  if !sends_touch(&screen) {
    return;
  }

  let field = screen.find_by_role(Role::TextField, "Words");
  let unit = unit(&screen);
  let (from, top, height) = caret_at(&screen, &field, 6);
  let (to, ..) = caret_at(&screen, &field, 16);
  let above = (to - 90. * unit, top - 110. * unit, 180. * unit, 112. * unit);

  screen
    .driver()
    .hold_finger((from + 2. * unit, top + height / 2.));
  screen
    .driver()
    .move_finger((to + 2. * unit, top + height / 2.));

  let during = screen.driver().screen_area(above);

  screen.driver().lift_finger();

  let after = screen.driver().screen_area(above);

  assert!(
    crate::changed(above, &after, &during).is_some(),
    "the magnifier shows while the finger selects"
  );
  assert_eq!(
    selection(&field),
    (6, 16),
    "the finger selects from where it went down to where it lifted"
  );

  let below = (
    from - 40. * unit,
    top + height - 8. * unit,
    to - from + 80. * unit,
    50. * unit,
  );
  let shown = screen.driver().screen_area(below);

  press_shift(&screen);

  let hidden = screen.driver().screen_area(below);
  let handles = crate::changed(below, &hidden, &shown).expect("the selection shows its handles");

  eprintln!("the selection from {from} to {to}; its handles across {handles:?}");
  assert!(
    (handles.0 - (from - 20. * unit)).abs() <= 2. * unit,
    "the start's handle ends at the start: {handles:?} against {from}"
  );
  assert!(
    (handles.0 + handles.2 - (to + 20. * unit)).abs() <= 2. * unit,
    "the end's handle starts at the end: {handles:?} against {to}"
  );
}

#[skene_gui::test]
fn a_selection_handle_moves_its_end_and_never_past_the_other() {
  let screen = testing::render(page);

  if !sends_touch(&screen) {
    return;
  }

  let field = screen.find_by_role(Role::TextField, "Words");
  let (from, top, height) = caret_at(&screen, &field, 6);
  let (to, ..) = caret_at(&screen, &field, 10);

  screen.driver().hold_finger((from + 1., top + height / 2.));
  screen.driver().move_finger((to + 1., top + height / 2.));
  screen.driver().lift_finger();
  assert_eq!(selection(&field), (6, 10), "a finger selects \"beta\"");

  // The end's handle, its icon right of the end, dragged to the end of "gamma".
  let end = caret_at(&screen, &field, 10);
  let grab = on_handle(&screen, end, 10.);
  let (gamma, ..) = caret_at(&screen, &field, 16);

  screen.driver().hold_finger(grab);
  screen
    .driver()
    .move_finger((gamma + (grab.0 - end.0), grab.1));
  screen.driver().lift_finger();
  assert_eq!(
    selection(&field),
    (6, 16),
    "the end moves with its handle, the start stays"
  );

  // The start's handle, its icon left of the start, dragged far past the end.
  let start = caret_at(&screen, &field, 6);
  let grab = on_handle(&screen, start, -10.);
  let (beyond, ..) = caret_at(&screen, &field, 22);

  screen.driver().hold_finger(grab);
  screen.driver().move_finger((beyond, grab.1));
  screen.driver().lift_finger();
  assert_eq!(
    selection(&field),
    (16, 15),
    "the start stops a character short of the end, the caret with it"
  );
}

#[skene_gui::test]
fn a_tap_on_the_caret_handle_selects_the_word_soon_after_and_asks_for_the_menu_later() {
  let screen = testing::render(page);

  if !sends_touch(&screen) {
    return;
  }

  let field = screen.find_by_role(Role::TextField, "Words");

  // Into "beta".
  field.tap_at(0.3, 0.5);

  let (_, caret) = selection(&field);
  let grab = on_handle(&screen, caret_at(&screen, &field, caret), 0.);

  screen.driver().hold_finger(grab);
  screen.driver().lift_finger();

  let word = selection(&field);

  eprintln!("the tap placed the caret at {caret}; the tap on its handle selected {word:?}");
  assert!(
    word.0 < word.1
      && word.0 <= caret
      && caret <= word.1
      && WORDS[word.0..word.1].trim() == &WORDS[word.0..word.1],
    "a tap on the caret's handle right after the tap that placed it selects the word there: {word:?}"
  );
  assert_eq!(screen.probe("menus"), "[]");

  // Long after, a tap on the end's handle asks for the menu and keeps the selection.
  screen.driver().wait(std::time::Duration::from_millis(600));

  let end = on_handle(&screen, caret_at(&screen, &field, word.1), 10.);

  screen.driver().hold_finger(end);
  screen.driver().lift_finger();
  assert_eq!(screen.probe("menus"), "[Touch]");
  assert_eq!(selection(&field), word, "the selection stays");
}

#[skene_gui::test]
fn a_pointer_press_hides_the_handles() {
  let screen = testing::render(page);

  if !sends_touch(&screen) {
    return;
  }

  let field = screen.find_by_role(Role::TextField, "Words");

  field.tap_at(0.3, 0.5);

  let (_, caret) = selection(&field);
  let unit = unit(&screen);
  let (x, top, height) = caret_at(&screen, &field, caret);
  let area = (
    x - 50. * unit,
    top + height + 4. * unit,
    100. * unit,
    30. * unit,
  );
  let shown = screen.driver().screen_area(area);

  // A click where the caret is: it stays, the handles go.
  screen.driver().click_point((x, top + height / 2.));

  let clicked = screen.driver().screen_area(area);

  assert!(
    crate::changed(area, &clicked, &shown).is_some(),
    "the handle showed, and goes as the pointer presses"
  );

  press_shift(&screen);
  assert_eq!(
    crate::changed(area, &clicked, &screen.driver().screen_area(area)),
    None,
    "nothing more goes as a key is pressed"
  );
}
