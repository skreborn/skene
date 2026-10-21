//! A scroll on the real platform, through the driver: focus moving to a row below the fold
//! scrolls it into view by as little as shows it, the keys scroll (`Home`, `PageDown`, an
//! arrow, `End`), and the bars' parts scroll as a hand works them (a drag on the thumb, a press
//! on the track that jumps, one with `Shift` that pages), as a reader reads the rows' places;
//! a horizontal bar shows where the content is wider than its box, a part a builder restyles
//! draws as styled, and on X11 a desktop whose XSETTINGS say a primary press does not warp has
//! it page.

use skene_gui::{
  prelude::*,
  testing::{self, Role, Screen},
};
use skene_gui_linux::testing::xsettings::{Value as Setting, Xsettings};

/// A scroll 100 pixels tall at the content's top, holding ten activatable rows 40 pixels tall:
/// "Row 0" to "Row 9".
fn page() -> impl View {
  let rows = (0..10).fold(Column::new(), |column, index| {
    column.add_child(
      Container::new()
        .set_label(format!("Row {index}"))
        .set_height(40)
        .handle_click(|| {})
        .set_child(Text::new(format!("Row {index}"))),
    )
  });

  Column::new().add_child(
    Scroll::new(ScrollAxes::Vertical)
      .set_height(100)
      .set_child(rows),
  )
}

/// Where the first row's top is, in pixels from the content's top: how far the scroll has
/// scrolled, negated.
fn first_top(screen: &Screen) -> f32 {
  screen.find_by_role(Role::Row, "Row 0").read().bounds.y
}

/// Whether `actual` is `wanted`, within the pixel a reader's whole-pixel extents round away.
fn near(actual: f32, wanted: f32) -> bool {
  (actual - wanted).abs() <= 1.
}

#[skene_gui::test]
fn focus_scrolls_into_view_and_the_keys_scroll() {
  let screen = testing::render(page);

  for _ in 0..6 {
    screen.press("Tab");
  }

  let focused = screen.focused();

  assert_eq!(focused.name(), "Row 5");

  let bounds = focused.read().bounds;

  assert!(
    bounds.y >= -1. && bounds.y + bounds.height <= 101.,
    "the sixth row shows within the scroll: {bounds:?}"
  );
  assert!(
    near(first_top(&screen), -140.),
    "scrolled by as little as shows it: the first row at {}",
    first_top(&screen)
  );

  screen.press("Home");
  assert!(
    near(first_top(&screen), 0.),
    "Home scrolls to the start: {}",
    first_top(&screen)
  );

  screen.press("PageDown");
  assert!(
    near(first_top(&screen), -87.5),
    "PageDown scrolls most of the box: {}",
    first_top(&screen)
  );

  screen.press("Down");
  assert!(
    near(first_top(&screen), -127.5),
    "an arrow scrolls a line: {}",
    first_top(&screen)
  );

  screen.press("End");
  assert!(
    near(first_top(&screen), -300.),
    "End scrolls to the end: {}",
    first_top(&screen)
  );
}

/// A text editor of a hundred lines, 150 pixels tall.
fn editing() -> impl View {
  let notes = Signal::new(
    (1..=100)
      .map(|line| format!("Line {line}"))
      .collect::<Vec<_>>()
      .join("\n"),
  );

  Column::new().set_padding(16).add_child(
    TextEditor::new("Notes")
      .set_max_height(150)
      .bind_text(notes),
  )
}

#[skene_gui::test]
fn an_editor_the_wheel_scrolls_tells_a_reader_where_its_characters_went() {
  let screen = testing::render(editing);
  let editor = screen.find_by_role(Role::TextEditor, "Notes");
  let before = screen.driver().character_rect(editor.element(), 0).1;

  editor.scroll(0, 3);

  let after = screen.driver().character_rect(editor.element(), 0).1;

  assert!(
    before - after > 10.,
    "a reader finds the first line where the wheel scrolled it: from {before} to {after}"
  );
}

/// A scroll 200 by 100 pixels, the region "Rows", holding ten activatable rows 40 pixels tall:
/// its bar's track runs down its last 10 pixels, its thumb 4 wide, 3 in from the edge, 24 long
/// (96 inside the track's ends times 100 of 400 in view) and moving 72.
fn barred() -> impl View {
  let rows = (0..10).fold(Column::new(), |column, index| {
    column.add_child(
      Container::new()
        .set_label(format!("Row {index}"))
        .set_height(40)
        .handle_click(|| {})
        .set_child(Text::new(format!("Row {index}"))),
    )
  });

  Column::new().set_padding(16).add_child(
    Container::new()
      .set_label("Rows")
      .set_width(200)
      .set_height(100)
      .set_child(
        Scroll::new(ScrollAxes::Vertical)
          .set_width(200)
          .set_height(100)
          .set_child(rows),
      ),
  )
}

/// The point `x`, `y` pixels from the top leading corner of the region `name`, in the
/// pointer's space.
fn point_of(screen: &Screen, name: &str, x: f32, y: f32) -> (f64, f64) {
  let region = screen.find_by_role(Role::Region, name);
  let bounds = region.read().bounds;

  screen.driver().point_in(
    region.element(),
    f64::from(x / bounds.width),
    f64::from(y / bounds.height),
  )
}

/// How far the region "Rows" has scrolled: its first row's top above the region's.
fn scrolled_by(screen: &Screen) -> f32 {
  screen.find_by_role(Role::Region, "Rows").read().bounds.y - first_top(screen)
}

/// Whether `actual` is `wanted`, within the two pointer pixels a bar's travel multiplies by
/// the four content pixels each stands for.
fn about(actual: f32, wanted: f32) -> bool {
  (actual - wanted).abs() <= 9.
}

#[skene_gui::test]
fn a_drag_on_the_thumb_scrolls_the_box() {
  let screen = testing::render(barred);
  let from = point_of(&screen, "Rows", 195., 14.);
  let to = point_of(&screen, "Rows", 195., 50.);

  screen.driver().drag(from, to);

  // Half the thumb's travel is half the box's scroll: 150 of 300.
  assert!(
    about(scrolled_by(&screen), 150.),
    "the drag scrolled the box with the thumb: by {}",
    scrolled_by(&screen)
  );
}

/// The scroll "Rows" of `count` rows 40 tall, 200 by 100.
fn growing(count: Signal<usize>) -> impl View {
  Column::new().set_padding(16).add_child(
    Container::new()
      .set_label("Rows")
      .set_width(200)
      .set_height(100)
      .set_child(
        Scroll::new(ScrollAxes::Vertical)
          .set_width(200)
          .set_height(100)
          .set_child(Column::new().derive_children(
            move || (0..count.get()).collect::<Vec<_>>(),
            |index| *index,
            |index| {
              Container::new()
                .set_label(format!("Row {index}"))
                .set_height(40)
                .handle_click(|| {})
                .set_child(Text::new(format!("Row {index}")))
            },
          )),
      ),
  )
}

#[skene_gui::test]
fn a_thumb_held_while_the_content_grows_moves_it_from_where_it_is() {
  let count = Signal::new(10);
  let screen = testing::render(move || growing(count));
  let from = point_of(&screen, "Rows", 195., 14.);
  let half = point_of(&screen, "Rows", 195., 32.);

  screen.driver().hold_button(from);
  screen.driver().move_pointer(half);
  count.set(30);
  screen.settle();

  let grown = scrolled_by(&screen);
  let further = point_of(&screen, "Rows", 195., 33.);

  screen.driver().move_pointer(further);

  let after = scrolled_by(&screen);

  screen.driver().release_button();
  assert!(
    (after - grown).abs() < 30.,
    "a one-pixel move of the held thumb after the content grew moves the content by about the \
     new ratio, not to where the old geometry put it: {grown} then {after}"
  );
}

#[skene_gui::test]
fn a_press_on_the_track_jumps_and_one_with_shift_pages() {
  let screen = testing::render(barred);

  screen.press("Tab");
  assert_eq!(screen.focused().name(), "Row 0");

  // A primary press centers the thumb on it, GTK's default: its start at 50 of a travel from
  // 2 to 74, two thirds of the way, so 200 of 300.
  let jump = point_of(&screen, "Rows", 195., 62.);

  screen.driver().click_point(jump);
  assert!(
    about(scrolled_by(&screen), 200.),
    "a primary press jumped the thumb to it: scrolled by {}",
    scrolled_by(&screen)
  );

  let jumped = scrolled_by(&screen);

  // A press with Shift above the thumb pages up by most of the box, as `PageUp` does.
  let page = point_of(&screen, "Rows", 195., 5.);

  screen.driver().shift_click_point(page);
  assert!(
    near(scrolled_by(&screen), jumped - 87.5),
    "a press with Shift paged up: from {jumped} to {}",
    scrolled_by(&screen)
  );
  assert_eq!(
    screen.focused().name(),
    "Row 0",
    "a press on the bar moves no focus"
  );
}

/// A scroll 200 by 60 pixels, the region "Strip", holding a box 600 wide, the region "Wide":
/// its bar runs along its last 10 pixels, its thumb 4 tall, 3 up from the edge, 65 long (196
/// inside the track's ends times 200 of 600 in view) and moving 131.
fn wide() -> impl View {
  Column::new().set_padding(16).add_child(
    Container::new()
      .set_label("Strip")
      .set_width(200)
      .set_height(60)
      .set_child(
        Scroll::new(ScrollAxes::Horizontal)
          .set_width(200)
          .set_height(60)
          .set_child(
            Container::new()
              .set_label("Wide")
              .set_width(600)
              .set_height(60)
              .set_background(Color::WHITE),
          ),
      ),
  )
}

/// How dark a pixel is: its channels' shortfall from white, summed.
fn darkness(color: Color) -> u32 {
  [color.red, color.green, color.blue]
    .into_iter()
    .map(|channel| u32::from(255 - channel))
    .sum()
}

#[skene_gui::test]
fn a_horizontal_bar_shows_where_the_content_is_wider_and_its_thumb_drags() {
  let screen = testing::render(wide);
  let strip = screen.find_by_role(Role::Region, "Strip").read().bounds;
  let on_thumb = screen.pixel(strip.x + 30., strip.y + 55.);
  let past_thumb = screen.pixel(strip.x + 150., strip.y + 55.);

  assert!(
    darkness(on_thumb) > darkness(past_thumb) + 60,
    "the thumb shows at the bottom, the track past it clear: {on_thumb:?} and {past_thumb:?}"
  );

  let start = screen.find_by_role(Role::Region, "Wide").read().bounds.x;
  let from = point_of(&screen, "Strip", 30., 55.);
  let to = point_of(&screen, "Strip", 95., 55.);

  screen.driver().drag(from, to);

  // 65 of 131 is about half the travel: about 199 of 400.
  let moved = start - screen.find_by_role(Role::Region, "Wide").read().bounds.x;

  assert!(
    about(moved, 199.),
    "the drag scrolled the content sideways: by {moved}"
  );
}

/// The rows of [`barred`], their track filled blue and their thumb green by the bar's builders.
fn restyled() -> impl View {
  let rows = (0..10).fold(Column::new(), |column, index| {
    column.add_child(
      Container::new()
        .set_height(40)
        .set_child(Text::new(format!("Row {index}"))),
    )
  });

  Column::new().set_padding(16).add_child(
    Container::new()
      .set_label("Rows")
      .set_width(200)
      .set_height(100)
      .set_child(
        Scroll::new(ScrollAxes::Vertical)
          .set_width(200)
          .set_height(100)
          .set_track_builder(|axis| {
            Scroll::default_track(axis).set_background(Color::rgb(0x0000c0))
          })
          .set_thumb_builder(|axis| {
            Scroll::default_thumb(axis)
              .set_corner_radius(0)
              .set_background(Color::rgb(0x00c000))
          })
          .set_child(rows),
      ),
  )
}

#[skene_gui::test]
fn a_bar_restyled_by_its_builders_draws_as_styled() {
  let screen = testing::render(restyled);
  let rows = screen.find_by_role(Role::Region, "Rows").read().bounds;
  let thumb = screen.pixel(rows.x + 195., rows.y + 14.);
  let track = screen.pixel(rows.x + 191., rows.y + 60.);

  assert!(
    thumb.green > 0xa0 && thumb.red < 0x40 && thumb.blue < 0x40,
    "the thumb is the builder's green: {thumb:?}"
  );
  assert!(
    track.blue > 0xa0 && track.red < 0x40 && track.green < 0x40,
    "the track is the builder's blue: {track:?}"
  );
}

#[skene_gui::test]
fn the_thumb_darkens_while_the_pointer_is_over_its_box() {
  let screen = testing::render(barred);
  let rows = screen.find_by_role(Role::Region, "Rows").read().bounds;
  let away = point_of(&screen, "Rows", 100., 140.);

  screen.driver().move_pointer(away);

  let resting = screen.pixel(rows.x + 195., rows.y + 14.);

  screen.find_by_role(Role::Region, "Rows").hover();

  let hovered = screen.pixel(rows.x + 195., rows.y + 14.);

  assert!(
    darkness(hovered) > darkness(resting) + 30,
    "the scroll box's hover darkens its thumb: from {resting:?} to {hovered:?}"
  );
}

#[skene_gui::test]
fn a_desktop_whose_primary_press_does_not_warp_has_it_page() {
  if std::env::var("SKENE_WINDOW_SYSTEM").as_deref() != Ok("x11") {
    skene_gui_linux::testing::not_tested_here(
      "XSETTINGS are an X server's, and the window is on Wayland",
    );

    return;
  }

  let _xsettings = Xsettings::serve(&[("Gtk/PrimaryButtonWarpsSlider", Setting::Integer(0))])
    .expect("the manager serves");
  let screen = testing::render(barred);
  let below = point_of(&screen, "Rows", 195., 62.);

  screen.driver().click_point(below);
  assert!(
    near(scrolled_by(&screen), 87.5),
    "a primary press below the thumb paged down: by {}",
    scrolled_by(&screen)
  );

  // The thumb now runs from 23 to 47: a press with Shift below it jumps it there.
  screen.driver().shift_click_point(below);
  assert!(
    about(scrolled_by(&screen), 200.),
    "a press with Shift jumped the thumb to it: by {}",
    scrolled_by(&screen)
  );
}
