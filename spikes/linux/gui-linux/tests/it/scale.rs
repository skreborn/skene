//! Fractional scale on the real platform, through the driver: the window takes the session's
//! scale (`SKENE_TEST_SCALE`), a field places its caret and its selection where the pointer
//! is and tells an input method where its caret is on the screen, and a picker's list opens
//! on its own surface under its face, a reader finding its options where they show and a
//! click there choosing, the screen showing both surfaces' device pixels one to one. Each
//! holds at any scale; the legs that scale fractionally run them at 1.5.

use std::{cell::RefCell, rc::Rc};

use skene_gui::{
  Color, Inset, LabelPlacement, Signal,
  painted::shapes::Rect,
  prelude::*,
  testing::{self, Driver as _, Role, publish},
};

/// Publishes `signal` as the probe `name`, as `Debug` writes it.
fn probe<T: Clone + std::fmt::Debug + 'static>(name: &'static str, signal: Signal<T>) {
  publish(name, move || format!("{:?}", signal.get_untracked()));
}

/// The scale the session was asked for: 1 where unset.
fn session_scale() -> f64 {
  std::env::var("SKENE_TEST_SCALE")
    .ok()
    .and_then(|scale| scale.parse().ok())
    .unwrap_or(1.)
}

/// A field holding "alpha beta gamma" and a picker of three sizes.
fn page() -> impl View {
  let words = Signal::new(String::from("alpha beta gamma"));
  let size = Signal::new("Small");

  probe("words", words);
  probe("size", size);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Words").bind_text(words))
    .add_child(
      Picker::new("Size")
        .bind_chosen(size)
        .set_options(["Small", "Medium", "Large"].map(|size| (size, size))),
    )
}

#[skene_gui::test]
fn the_window_takes_the_sessions_scale() {
  let screen = testing::render(page);
  let scale = screen.driver().scale();

  eprintln!("the window's scale is {scale}");
  assert!(
    (scale - session_scale()).abs() < 1e-6,
    "the window is drawn at the session's scale ({scale}, asked {})",
    session_scale()
  );
}

#[skene_gui::test]
fn a_click_in_a_field_places_the_caret_under_the_pointer() {
  let screen = testing::render(page);
  let field = screen.find_by_role(Role::TextField, "Words");

  for offset in [6, 11, 2] {
    let at = screen
      .driver()
      .character_point(field.element(), offset, 0.15);

    screen.driver().click_point(at);

    let caret = field.read().selection.expect("the field reports its caret");

    assert_eq!(
      (caret.start, caret.end),
      (offset, offset),
      "a click at the leading edge of character {offset} puts the caret before it"
    );
  }
}

#[skene_gui::test]
fn a_drag_in_a_field_selects_from_the_press_to_the_pointer() {
  let screen = testing::render(page);
  let field = screen.find_by_role(Role::TextField, "Words");
  let (from, to) = {
    let driver = screen.driver();

    (
      driver.character_point(field.element(), 6, 0.1),
      driver.character_point(field.element(), 9, 0.9),
    )
  };

  screen.driver().drag(from, to);

  let selection = field
    .read()
    .selection
    .expect("the field reports its selection");

  assert_eq!(
    (selection.start, selection.end),
    (6, 10),
    "\"beta\" selected"
  );
}

#[skene_gui::test]
fn a_pickers_list_opens_under_its_face_where_a_reader_finds_and_clicks_its_options() {
  let screen = testing::render(page);
  let face = screen.find_by_role(Role::ComboBox, "Size");
  let below = face.read().bounds;

  face.click();
  assert_eq!(face.read().expanded, Some(true));

  let list = screen.find_by_role(Role::ListBox, "").read().bounds;
  let large = screen.find_by_role(Role::Option, "Large");
  let small = screen.find_by_role(Role::Option, "Small").read().bounds;
  let bounds = large.read().bounds;

  eprintln!("the face at {below:?}, its list at {list:?}, Small at {small:?}, Large at {bounds:?}");
  assert!(
    list.y >= below.y + below.height - 1.,
    "the list opens under the face ({list:?} under {below:?})"
  );
  assert!(
    list.x >= below.x && list.x - below.x <= 8.,
    "aligned with its leading edge, inside the popup's frame ({} and {})",
    list.x,
    below.x
  );
  assert!(
    small.y >= list.y && small.x >= list.x && bounds.y >= small.y + small.height - 1.,
    "the options in the list, Large under Small"
  );

  large.click();
  assert_eq!(screen.probe("size"), "\"Large\"", "the click chose Large");
  assert_eq!(face.read().expanded, Some(false));
}

/// Columns a device pixel wide, black and white in turn from the box's leading edge, over
/// white: drawn in the device pixels the canvas records in.
fn stripes(canvas: &mut Canvas<'_>) {
  let size = canvas.size();

  canvas.fill(&Rect::new(0., 0., size.width, size.height), Color::WHITE);

  for column in (0..size.width as u32).step_by(2) {
    let left = f64::from(column);

    canvas.fill(&Rect::new(left, 0., left + 1., size.height), Color::BLACK);
  }
}

/// A box of [`stripes`] at the content's top left, and a picker whose options each lead with
/// one.
fn striped() -> impl View {
  Column::new()
    .set_spacing(12)
    .add_child(Painted::new(stripes).set_width(48).set_height(16))
    .add_child(
      Picker::new("Size")
        // A list with no border, so its options sit at whole device pixels.
        .set_list(Container::new().set_background(Color::WHITE).set_padding(4))
        .set_item_builder(|_, label| {
          PickerItem::new(label)
            .set_padding(Inset::new(0, 6, 0, 0))
            .set_label_placement(LabelPlacement::Trailing)
            .set_mark(Painted::new(stripes).set_width(24).set_height(12))
        })
        .set_options(["Small", "Medium", "Large"].map(|size| (size, size))),
    )
}

/// The longest run, in any row of `shot`, of device pixels black and white in turn (each
/// channel within 2 of 0 or 255): as long as a box of [`stripes`] where the screen shows its
/// surface's pixels one to one, and none where it resamples them.
fn longest_stripes(shot: &testing::Screenshot) -> u32 {
  let shade = |x: u32, y: u32| {
    let at = ((y * shot.width + x) * 4) as usize;
    let pixel = &shot.rgba[at..at + 3];

    if pixel.iter().all(|channel| *channel <= 2) {
      Some(false)
    } else if pixel.iter().all(|channel| *channel >= 253) {
      Some(true)
    } else {
      None
    }
  };
  let mut longest = 0;

  for y in 0..shot.height {
    let (mut run, mut last) = (0, None);

    for x in 0..shot.width {
      let now = shade(x, y);

      run = match (last, now) {
        (Some(before), Some(now)) if before != now => run + 1,
        (_, Some(_)) => 1,
        (_, None) => 0,
      };
      last = now;
      longest = longest.max(run);
    }
  }

  longest
}

#[skene_gui::test]
fn the_window_and_its_pickers_list_show_their_device_pixels_one_to_one() {
  let screen = testing::render(striped);

  place_on_the_grid(&screen);

  let scale = screen.driver().scale();
  let window = longest_stripes(&screen.driver().content());

  screen.find_by_role(Role::ComboBox, "Size").click();

  let (list, at) = screen
    .driver()
    .popup_content()
    .expect("the list is open on its own surface");
  let list = longest_stripes(&list);

  eprintln!(
    "at scale {scale}: the window shows {window} device pixels of stripes in a row, the list at {at:?} {list}"
  );
  assert!(
    f64::from(window) >= (48. * scale).floor() - 1.,
    "the screen shows the window's device pixels one to one ({window} in a row)"
  );
  assert!(
    f64::from(list) >= (24. * scale).floor() - 1.,
    "and the list's ({list} in a row)"
  );
}

#[skene_gui::test]
fn a_field_tells_an_input_method_where_its_caret_is_on_the_screen() {
  if std::env::var("SKENE_BACKEND").as_deref() != Ok("x11") {
    skene_gui_linux::testing::not_tested_here(
      "on Wayland the input method is the compositor's, over text-input-v3, and none here hands the caret's rectangle on to a client (sway 1.9 has no input method popups)",
    );

    return;
  }

  let input_method = skene_gui_linux::testing::input_method::InputMethod::serve()
    .expect("the stand-in input method takes the session bus's name");
  let screen = testing::render(page);
  let field = screen.find_by_role(Role::TextField, "Words");
  let at = screen.driver().character_point(field.element(), 6, 0.15);

  screen.driver().click_point(at);
  screen.driver().wait(std::time::Duration::from_millis(200));

  let told = input_method.told();
  let (x, y, width, height) = screen.driver().character_rect(field.element(), 6);

  eprintln!("told {told:?}; the character after the caret at {x}, {y} {width}x{height}");
  assert_eq!(
    told.contexts, 1,
    "the window asked the input method for a context"
  );
  assert!(told.focused, "and told it a field took focus");

  let (cx, cy, _, ch) = told.cursor.expect("and where the caret is");

  assert!(
    (f64::from(cx) - x).abs() <= 2.,
    "the caret's rectangle starts at the character's leading edge on the screen ({cx} and {x})"
  );
  assert!(
    (f64::from(cy) - y).abs() <= 3. && (f64::from(ch) - height).abs() <= 4.,
    "and spans its line ({cy}+{ch} and {y}+{height})"
  );
  assert_eq!(
    told.surrounding,
    Some(("alpha beta gamma".to_owned(), 6, 6)),
    "with the text around the caret"
  );
}

/// A box of [`stripes`] a pixel in from the content's top left: off the device's pixel grid
/// at a fractional scale.
fn offset_stripes() -> impl View {
  Container::new()
    .set_padding(Inset::new(1, 0, 0, 1))
    .set_child(Painted::new(stripes).set_width(48).set_height(16))
}

/// Moves the window to an even position where the session is sway: sway puts a window at
/// whole logical pixels, which at a fractional scale may fall between device pixels, where it
/// filters what it shows; at an even position (a whole device pixel at 1.5) it shows the
/// buffers as they are.
fn place_on_the_grid(screen: &testing::Screen) {
  if std::env::var("SKENE_TEST_SESSION").as_deref() == Ok("sway") {
    let moved = std::process::Command::new("swaymsg")
      .arg("[app_id=dev.skene.test] move absolute position 200 20")
      .stdout(std::process::Stdio::null())
      .status()
      .is_ok_and(|status| status.success());

    assert!(moved, "swaymsg moves the window");
    screen.driver().window_moved();
  }
}

#[skene_gui::test]
fn a_painted_box_draws_in_device_pixels_wherever_it_sits() {
  let screen = testing::render(offset_stripes);

  place_on_the_grid(&screen);

  let scale = screen.driver().scale();
  let shown = longest_stripes(&screen.driver().content());

  eprintln!("at scale {scale}: {shown} device pixels of stripes in a row");
  assert!(
    f64::from(shown) >= (48. * scale).floor() - 1.,
    "a box off the device's grid draws one device pixel to a column ({shown} in a row)"
  );
}

/// A box of [`stripes`] a pixel in from the content's top left, publishing the device size and
/// the scale it last recorded at as the probe `recorded` (`"96x32 at 2"`).
fn recorded_stripes() -> impl View {
  let recorded = Rc::new(RefCell::new(String::new()));

  {
    let recorded = Rc::clone(&recorded);

    publish("recorded", move || recorded.borrow().clone());
  }

  Container::new()
    .set_padding(Inset::new(1, 0, 0, 1))
    .set_child(
      Painted::new(move |canvas| {
        let (size, scale) = (canvas.size(), canvas.scale());

        *recorded.borrow_mut() = format!("{}x{} at {}", size.width, size.height, scale.x);
        stripes(canvas);
      })
      .set_width(48)
      .set_height(16),
    )
}

/// Puts sway's outputs back at `scale` when dropped, so a test that changes it leaves the
/// session as it found it for the tests after it.
struct OutputScale(f64);

impl OutputScale {
  /// Sets sway's outputs to `scale`: whether swaymsg did.
  fn set(scale: f64) -> bool {
    std::process::Command::new("swaymsg")
      .args(["output", "*", "scale", &scale.to_string()])
      .stdout(std::process::Stdio::null())
      .status()
      .is_ok_and(|status| status.success())
  }
}

impl Drop for OutputScale {
  fn drop(&mut self) {
    Self::set(self.0);
  }
}

/// Turns the window's loop until the probe `name` reads `wanted`, for up to five seconds; what
/// it reads then.
fn probe_until(screen: &testing::Screen, name: &str, wanted: &str) -> String {
  let start = std::time::Instant::now();

  loop {
    let read = screen.probe(name);

    if read == wanted || start.elapsed() > std::time::Duration::from_secs(5) {
      return read;
    }

    screen.driver().wait(std::time::Duration::from_millis(20));
  }
}

#[skene_gui::test]
fn a_painted_box_records_again_at_its_windows_new_scale() {
  let screen = testing::render(recorded_stripes);
  let before = screen.driver().scale();
  let device = |scale: f64| {
    format!(
      "{}x{} at {scale}",
      (48. * scale).round(),
      (16. * scale).round()
    )
  };

  assert_eq!(
    probe_until(&screen, "recorded", &device(before)),
    device(before),
    "the box records at its window's scale"
  );

  if std::env::var("SKENE_TEST_SESSION").as_deref() != Ok("sway") {
    eprintln!("only the sway leg changes an output's scale while a window shows");

    return;
  }

  let wanted = if (before - 2.).abs() < 1e-6 { 1. } else { 2. };
  let _restore = OutputScale(before);

  assert!(
    OutputScale::set(wanted),
    "swaymsg changes the output's scale"
  );
  assert_eq!(
    probe_until(&screen, "recorded", &device(wanted)),
    device(wanted),
    "the box records again at the window's new scale ({})",
    screen.driver().scale()
  );
}
