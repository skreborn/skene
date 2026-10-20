//! Touchpads, touchscreens and context menus on the real platform, through the driver: a fling
//! goes on once the fingers lift and a wheel never flings; a tap clicks, a swipe scrolls, a long
//! press asks for a context menu, as a secondary press and `Shift-F10` do; a text field takes a
//! finger as GTK's entry does. A leg whose input sends no touch or no touchpad says so and tests
//! nothing of it.

use skene_gui::{
  ContextMenuEvent, MenuSource, Signal,
  prelude::*,
  testing::{self, Driver as _, Found, Role, publish},
};

/// Publishes `signal` as the probe `name`, as `Debug` writes it.
fn probe<T: Clone + std::fmt::Debug + 'static>(name: &'static str, signal: Signal<T>) {
  publish(name, move || format!("{:?}", signal.get_untracked()));
}

/// A scroll box of a hundred rows, each 30 pixels tall.
fn rows() -> impl View {
  Column::new().set_padding(16).add_child(
    Scroll::new(ScrollAxes::Vertical)
      .set_height(300)
      .set_child(Column::new().add_children((1..=100).map(|row| {
        Container::new()
          .set_height(30)
          .set_child(Text::new(format!("Row {row}")))
      }))),
  )
}

/// How far `row` sits from where it sat at `before`, up positive.
fn scrolled(row: &Found<'_>, before: f32) -> f32 {
  before - row.read().bounds.y
}

/// Whether the leg's input sends a touchpad's scrolling, saying so where it does not.
fn sends_touchpad(screen: &testing::Screen) -> bool {
  let sends = screen.driver().sends_touchpad_scrolling();

  if !sends {
    skene_gui_linux::testing::not_tested_here(format_args!(
      "{} sends no touchpad scrolling",
      screen.driver().input_name()
    ));
  }

  sends
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

#[skene_gui::test]
fn a_touchpad_fling_scrolls_on_once_the_fingers_lift() {
  let screen = testing::render(rows);

  if !sends_touchpad(&screen) {
    return;
  }

  let row = screen.find_by_text("Row 1");
  let before = row.read().bounds.y;

  screen.find_by_text("Row 3").fling(0, 200);

  let moved = scrolled(&row, before);

  eprintln!("the fingers moved 200 pixels, the list {moved}");
  assert!(
    moved > 300.,
    "the list goes on past the fingers' 200 pixels, slowing down (moved {moved})"
  );

  let rested = row.read().bounds.y;

  screen.driver().wait(std::time::Duration::from_millis(300));
  assert_eq!(row.read().bounds.y, rested, "a fling comes to rest");
}

#[skene_gui::test]
fn a_wheel_never_flings() {
  let screen = testing::render(rows);
  let row = screen.find_by_text("Row 1");
  let before = row.read().bounds.y;
  let over = screen.find_by_text("Row 3");

  // On X11 the first smooth step after the pointer's device changes only seeds the
  // valuators (as GDK's does): the wheel turns twice.
  over.scroll(0, 3);
  over.scroll(0, 3);

  let moved = scrolled(&row, before);

  screen.driver().wait(std::time::Duration::from_millis(300));

  let after = scrolled(&row, before);

  eprintln!("six wheel steps moved the list {moved}, then {after}");
  assert!(moved > 0., "the wheel moves the list");
  assert_eq!(after, moved, "and nothing moves after it");
}

/// A button that counts its activations, a box that counts its clicks and hears context menus,
/// a field in a box that hears context menus, and the hundred rows.
fn touch() -> impl View {
  let taps = Signal::new(0_u32);
  let clicks = Signal::new(0_u32);
  let menus = Signal::new(Vec::<(MenuSource, &'static str)>::new());

  probe("taps", taps);
  probe("clicks", clicks);
  probe("menus", menus);

  let heard = move |what: &'static str| {
    move |event: &ContextMenuEvent| {
      menus.update(|menus| menus.push((event.source, what)));
      Bubbling::Stop
    }
  };

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Button::new("Tap me").handle_activate(move || taps.update(|taps| *taps += 1)))
    .add_child(
      Container::new()
        .set_padding(12)
        .handle_click(move || clicks.update(|clicks| *clicks += 1))
        .handle_context_menu(heard("box"))
        .set_child(Text::new("Hold me")),
    )
    .add_child(
      // A field with no menu of its own, so the box around it hears its requests.
      Container::new()
        .handle_context_menu(heard("field"))
        .set_child(
          TextField::new("Words")
            .set_context_menu(ContextMenu::new())
            .set_text("alpha beta gamma delta"),
        ),
    )
    .add_child(Scroll::new(ScrollAxes::Vertical).set_height(200).set_child(
      Column::new().add_children((1..=60).map(|row| {
        Container::new()
          .set_height(30)
          .set_child(Text::new(format!("Line {row}")))
      })),
    ))
}

#[skene_gui::test]
fn a_tap_clicks() {
  let screen = testing::render(touch);

  if !sends_touch(&screen) {
    return;
  }

  screen.find_by_role(Role::Button, "Tap me").tap();
  assert_eq!(screen.probe("taps"), "1");
  screen.find_by_text("Hold me").tap();
  assert_eq!(
    screen.probe("clicks"),
    "1",
    "a tap on an acting box clicks it"
  );
  assert_eq!(screen.probe("menus"), "[]");
}

#[skene_gui::test]
fn a_long_press_asks_for_a_context_menu_and_clicks_nothing() {
  let screen = testing::render(touch);

  if !sends_touch(&screen) {
    return;
  }

  screen.find_by_text("Hold me").long_press();
  assert_eq!(screen.probe("menus"), "[(Touch, \"box\")]");
  assert_eq!(screen.probe("clicks"), "0", "a long press is no click");
}

#[skene_gui::test]
fn a_secondary_press_and_shift_f10_ask_for_a_context_menu() {
  let screen = testing::render(touch);

  screen.find_by_text("Hold me").context_click();
  assert_eq!(screen.probe("menus"), "[(Pointer, \"box\")]");
  screen
    .find_by_role(Role::TextField, "Words")
    .press("shift+F10");
  assert_eq!(
    screen.probe("menus"),
    "[(Pointer, \"box\"), (Keyboard, \"field\")]"
  );
  screen.press("Menu");
  assert_eq!(
    screen.probe("menus"),
    "[(Pointer, \"box\"), (Keyboard, \"field\"), (Keyboard, \"field\")]",
    "the menu key at the focus"
  );
}

#[skene_gui::test]
fn a_swipe_scrolls_the_box_under_the_finger_and_flings_on() {
  let screen = testing::render(touch);

  if !sends_touch(&screen) {
    return;
  }

  let line = screen.find_by_text("Line 1");
  let before = line.read().bounds.y;

  screen.find_by_text("Line 4").swipe_by(0, -120);

  let moved = scrolled(&line, before);

  eprintln!("the finger moved 120 pixels, the list {moved}");
  assert!(
    moved >= 110.,
    "the list follows the finger and flings on as it lifts (moved {moved})"
  );
  assert_eq!(screen.probe("taps"), "0");
  assert_eq!(screen.probe("clicks"), "0");
  assert_eq!(screen.probe("menus"), "[]", "a swipe asks for no menu");
}

#[skene_gui::test]
fn a_finger_in_a_field_places_the_caret_selects_and_asks_for_the_menu_on_a_selection() {
  let screen = testing::render(touch);

  if !sends_touch(&screen) {
    return;
  }

  let field = screen.find_by_role(Role::TextField, "Words");

  field.tap_at(0.15, 0.5);

  let tapped = field.read();

  assert!(tapped.focused, "a tap focuses the field");

  let caret = tapped.selection.expect("the field reports its selection");

  assert_eq!(caret.start, caret.end, "a tap places the caret");
  assert!(
    caret.end > 0 && caret.end < 22,
    "where the finger is, within the text (at {})",
    caret.end
  );

  // A drag from where the finger goes down (past the text's end, at the field's center)
  // selects to where it lifts (past its start), as GTK's entry does.
  field.swipe_by(-240, 0);

  let selected = field.read().selection.expect("a selection");

  assert_eq!(
    (selected.start, selected.end),
    (22, 0),
    "the selection runs from where the finger went down to where it lifted"
  );
  assert_eq!(screen.probe("menus"), "[]");

  // A tap on the selection asks for the context menu (GTK's selection popover), and keeps it.
  field.tap_at(0.15, 0.5);
  assert_eq!(screen.probe("menus"), "[(Touch, \"field\")]");

  let kept = field.read().selection.expect("a selection");

  assert_eq!((kept.start, kept.end), (22, 0), "the selection stays");
}
