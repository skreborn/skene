//! Fractional scale on the real platform, through the driver: the window takes the session's
//! scale (`SKENE_TEST_SCALE`), a field places its caret and its selection where the pointer
//! is and tells an input method where its caret is on the screen, and a picker's list opens
//! on its own surface under its face, a reader finding its options where they show and a
//! click there choosing, the compositor showing both surfaces' pixels one to one. Each holds
//! at any scale; the legs that scale fractionally run them at 1.5.

use skene_gui::{
  Signal,
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
  let size = Signal::new(Some(0_usize));

  probe("words", words);
  probe("size", size);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Words").bind_text(words))
    .add_child(
      Picker::new("Size")
        .set_options(["Small", "Medium", "Large"])
        .bind_chosen(size),
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
  assert_eq!(screen.probe("size"), "Some(2)", "the click chose Large");
  assert_eq!(face.read().expanded, Some(false));
}

/// The screen as the compositor shows it, through `grim` (wlroots' screencopy): its device
/// pixels as RGBA, its width and height; `None` where `grim` is missing or fails.
///
/// `grim` draws the outputs onto a canvas of their layout's logical size times the scale, so
/// the runner gives a scaled leg a screen whose logical size is whole pixels.
fn screen_capture() -> Option<(Vec<u8>, u32, u32)> {
  let path = std::env::temp_dir().join(format!("skene-scale-{}.png", std::process::id()));
  let captured = std::process::Command::new("grim")
    .args(["-t", "png"])
    .arg(&path)
    .status()
    .is_ok_and(|status| status.success());

  if !captured {
    return None;
  }

  let file = std::fs::File::open(&path).ok()?;
  let mut reader = png::Decoder::new(std::io::BufReader::new(file))
    .read_info()
    .ok()?;
  let mut bytes = vec![0; reader.output_buffer_size()?];
  let info = reader.next_frame(&mut bytes).ok()?;
  let channels = match info.color_type {
    png::ColorType::Rgb => 3,
    png::ColorType::Rgba => 4,
    _ => return None,
  };
  let rgba = bytes[..info.buffer_size()]
    .chunks_exact(channels)
    .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
    .collect();

  let _ = std::fs::remove_file(&path);

  Some((rgba, info.width, info.height))
}

/// The share of `shot`'s opaque pixels outside `covered` (a rectangle of `shot`'s device
/// pixels, `x, y, width, height`) that `screen` shows unchanged (each channel within 2) with
/// `shot`'s top left at `at` (device pixels), at the best of the offsets one pixel around it,
/// which rounding a fractional position may take.
fn shown_unchanged(
  screen: &(Vec<u8>, u32, u32),
  shot: &testing::Screenshot,
  at: (f64, f64),
  covered: (i64, i64, i64, i64),
) -> f64 {
  let (pixels, width, height) = screen;
  let mut best = 0.;

  for dy in -1..=1 {
    for dx in -1..=1 {
      let (left, top) = (at.0.round() as i64 + dx, at.1.round() as i64 + dy);
      let (mut same, mut opaque) = (0_u64, 0_u64);

      for row in 0..i64::from(shot.height) {
        for column in 0..i64::from(shot.width) {
          let index = ((row * i64::from(shot.width) + column) * 4) as usize;
          let ours = &shot.rgba[index..index + 4];

          let under = column >= covered.0
            && row >= covered.1
            && column < covered.0 + covered.2
            && row < covered.1 + covered.3;

          if ours[3] != 255 || under {
            continue;
          }

          opaque += 1;

          let (x, y) = (left + column, top + row);

          if x < 0 || y < 0 || x >= i64::from(*width) || y >= i64::from(*height) {
            continue;
          }

          let at = ((y * i64::from(*width) + x) * 4) as usize;
          let theirs = &pixels[at..at + 4];

          if ours[..3]
            .iter()
            .zip(&theirs[..3])
            .all(|(a, b)| a.abs_diff(*b) <= 2)
          {
            same += 1;
          }
        }
      }

      if opaque > 0 {
        best = f64::max(best, same as f64 / opaque as f64);
      }
    }
  }

  best
}

#[skene_gui::test]
fn the_window_and_its_pickers_list_show_their_pixels_one_to_one() {
  let screen = testing::render(page);

  if std::env::var("SKENE_TEST_SESSION").as_deref() != Ok("sway") {
    eprintln!("not tested here: the screen is read through wlroots' screencopy, on sway");

    return;
  }

  // Sway puts a window at whole logical pixels, which at a fractional scale may fall between
  // device pixels, where it filters what it shows; at an even position (a whole device pixel
  // at 1.5) it shows the buffers as they are.
  let moved = std::process::Command::new("swaymsg")
    .arg("[app_id=dev.skene.test] move absolute position 200 20")
    .stdout(std::process::Stdio::null())
    .status()
    .is_ok_and(|status| status.success());

  assert!(moved, "swaymsg moves the window");
  screen.settle();

  let face = screen.find_by_role(Role::ComboBox, "Size");

  face.click();

  let scale = screen.driver().scale();
  let window = screen.driver().window_rect();
  let content = screen.driver().content();
  let (list, at) = screen
    .driver()
    .popup_content()
    .expect("the list is open on its own surface");
  let Some(captured) = screen_capture() else {
    eprintln!("not tested here: grim cannot read the screen");

    return;
  };
  let window_at = (window.0 * scale, window.1 * scale);
  let list_at = (at.0 * scale, at.1 * scale);
  // The list covers the window where it is open.
  let under = (
    (list_at.0 - window_at.0).round() as i64 - 1,
    (list_at.1 - window_at.1).round() as i64 - 1,
    i64::from(list.width) + 2,
    i64::from(list.height) + 2,
  );
  let window_shown = shown_unchanged(&captured, &content, window_at, under);
  let list_shown = shown_unchanged(&captured, &list, list_at, (0, 0, 0, 0));

  eprintln!("the window at {window_at:?}, the list at {list_at:?} (device pixels)");

  eprintln!(
    "at scale {scale}: the window's {}x{} pixels shown {:.4} unchanged, the list's {}x{} {:.4}",
    content.width, content.height, window_shown, list.width, list.height, list_shown
  );
  assert_eq!(
    (list.width, list.height),
    (
      (f64::from(face.read().bounds.width) * scale).round() as u32,
      list.height
    ),
    "the list's surface is drawn in device pixels, as wide as its face"
  );
  assert!(
    window_shown > 0.99,
    "the compositor shows the window's buffer one to one ({window_shown})"
  );
  assert!(
    list_shown > 0.99,
    "the compositor shows the list's buffer one to one ({list_shown})"
  );
}

#[skene_gui::test]
fn a_field_tells_an_input_method_where_its_caret_is_on_the_screen() {
  if std::env::var("SKENE_BACKEND").as_deref() != Ok("x11") {
    eprintln!(
      "not tested here: on Wayland the input method is the compositor's, over text-input-v3, and none here hands the caret's rectangle on to a client (sway 1.9 has no input method popups)"
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
