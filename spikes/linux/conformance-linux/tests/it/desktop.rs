//! The desktop's font settings on the real platform, through the driver: the settings portal's
//! `font-antialiasing` and `font-hinting` reach the text drawn, and on X11 the XSETTINGS
//! manager's `Xft/*` win over them where they state a value, as the screen shows.
//!
//! The desktop's settings sit beneath fontconfig's own rules, so each test draws under a
//! configuration that states none (`fonts.conf` beside it).

use std::path::Path;

use skene_gui::{
  Color,
  prelude::*,
  testing::{self, Role, Screen, Screenshot},
};
use skene_gui_linux::testing::{
  portal::Setting,
  xsettings::{Value, Xsettings},
};

/// The system's fonts under no rendering rule.
const PLAIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/fonts.conf");

/// The page rendered under [`PLAIN`].
fn render() -> Screen {
  let screen = testing::render(page);

  screen.driver().set_fontconfig(Some(Path::new(PLAIN)));

  screen
}

/// Black text on white.
fn page() -> impl View {
  Container::new()
    .set_background(Color::WHITE)
    .set_padding(16)
    .set_child(
      Text::new("Rendering 0123 Wavy agility")
        .set_font_size(20)
        .set_color(Color::BLACK),
    )
}

/// The text's pixels as the screen shows them, RGB row by row.
fn text_pixels(screen: &Screen) -> Vec<[u8; 3]> {
  let bounds = screen
    .find_by_text("Rendering 0123 Wavy agility")
    .read()
    .bounds;
  let shot: Screenshot = screen.render_to_image();
  let mut pixels = Vec::new();

  for y in bounds.y as u32..(bounds.y + bounds.height) as u32 {
    for x in bounds.x as u32..(bounds.x + bounds.width) as u32 {
      let [red, green, blue, _] = shot.rgba_at(x as f32, y as f32);

      pixels.push([red, green, blue]);
    }
  }

  pixels
}

/// How many of the text's pixels are neither the paper nor the ink: the edges antialiasing
/// shades.
fn shaded(screen: &Screen) -> usize {
  text_pixels(screen)
    .into_iter()
    .filter(|pixel| pixel.iter().any(|channel| (24..=231).contains(channel)))
    .count()
}

#[skene_gui::test]
fn the_portals_antialiasing_and_hinting_reach_the_text_drawn() {
  let screen = render();

  screen
    .driver()
    .set_portal(Setting::FontAntialiasing("none"));

  let none = shaded(&screen);

  screen
    .driver()
    .set_portal(Setting::FontAntialiasing("grayscale"));

  let grayscale = shaded(&screen);

  eprintln!("shaded pixels: none {none}, grayscale {grayscale}");
  assert_eq!(none, 0, "no antialiasing: every pixel paper or ink");
  assert!(grayscale > 50, "grayscale antialiasing shades the edges");

  screen.driver().set_portal(Setting::FontHinting("none"));

  let unhinted = text_pixels(&screen);

  screen.driver().set_portal(Setting::FontHinting("full"));

  let hinted = text_pixels(&screen);
  let differing = unhinted
    .iter()
    .zip(&hinted)
    .filter(|(one, other)| one != other)
    .count();

  eprintln!("pixels hinting changed: {differing}");
  assert!(differing > 20, "full hinting draws the glyphs otherwise");
}

#[skene_gui::test]
fn xsettings_win_over_the_portal_where_they_state_a_value() {
  if std::env::var("SKENE_WINDOW_SYSTEM").as_deref() != Ok("x11") {
    skene_gui_linux::testing::not_tested_here(
      "XSETTINGS are an X server's, and the window is on Wayland",
    );

    return;
  }

  let mut xsettings =
    Xsettings::serve(&[("Xft/Antialias", Value::Integer(1))]).expect("the manager serves");
  let screen = render();

  screen
    .driver()
    .set_portal(Setting::FontAntialiasing("none"));
  assert!(
    shaded(&screen) > 50,
    "XSETTINGS' antialiasing wins over the portal's"
  );

  xsettings.set(&[]).expect("the manager publishes");
  screen.settle();
  assert_eq!(
    shaded(&screen),
    0,
    "the portal's, where XSETTINGS state none"
  );

  // A reader finds the text all along.
  screen.find_by_role(Role::Text, "Rendering 0123 Wavy agility");
}
