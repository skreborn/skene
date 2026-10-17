//! The title bar and the shadow Skene draws where the compositor asks the window to decorate
//! itself (mutter, weston), on the real platform, through the driver: a floating window casts
//! its shadow around it and a maximized one fills the screen with none; a drag from just past
//! a corner resizes the window and one on the title bar moves it; the title bar's buttons act.
//! Where the compositor decorates the window (sway, KWin, X11), each says so and returns.

use skene_gui::{
  Color,
  prelude::*,
  testing::{self, Role, Screen, publish},
};
use skene_gui_linux::testing::TitleButton;

/// A white page with a button as wide as the window at its top, counting its activations as
/// "wide".
fn page() -> impl View {
  let wide = Signal::new(0_u32);

  publish("wide", move || wide.get_untracked().to_string());

  Container::new().set_background(Color::WHITE).set_child(
    Column::new()
      .set_item_alignment(AxisAlignment::Stretch)
      .add_child(Button::new("Wide").handle_activate(move || wide.update(|wide| *wide += 1))),
  )
}

/// The rendered page where the window draws its own title bar; `None`, said so, elsewhere.
fn decorated() -> Option<Screen> {
  let screen = testing::render(page);

  if screen.driver().is_decorated() {
    Some(screen)
  } else {
    eprintln!("not tested here: the compositor decorates the window itself");

    None
  }
}

/// The color the screen shows at `at` (the pointer's space).
fn shown_at(screen: &Screen, at: (f64, f64)) -> u32 {
  screen
    .driver()
    .screen_area((at.0, at.1, 1., 1.))
    .color_at(0., 0.)
}

#[skene_gui::test]
fn a_floating_window_casts_a_shadow_and_a_maximized_one_fills_the_screen() {
  let Some(screen) = decorated() else {
    return;
  };
  let (x, y, width, height) = screen.driver().window_rect();
  let (screen_width, screen_height) = screen.driver().screen_extent();
  let middle = y + height / 2.;
  let near = shown_at(&screen, (x - 3., middle));
  let far = shown_at(&screen, (x - 40., middle));

  eprintln!("beside the window: {near:?} within its shadow, {far:?} past it");
  assert!(
    x >= 40.,
    "the window floats clear of the screen's edge: {x}"
  );
  assert_ne!(near, far, "its shadow shows just past its edge");
  assert!(width > 0. && height > 0.);

  let maximize = screen
    .driver()
    .title_button_point(TitleButton::Maximize)
    .expect("the title bar has a maximize button");

  screen.driver().click_point(maximize);
  screen.driver().window_moved();

  let (x, y, width, height) = screen.driver().window_rect();

  eprintln!("maximized: {x}, {y} {width}x{height} of {screen_width}x{screen_height}");
  assert!(
    x.abs() <= 1. && (width - screen_width).abs() <= 1.,
    "maximized, it spans the screen's width"
  );
  assert!(
    height >= screen_height - 64.,
    "and its height, what the desktop keeps aside"
  );

  let restore = screen
    .driver()
    .title_button_point(TitleButton::Maximize)
    .expect("the title bar has a restore button");

  screen.driver().click_point(restore);
  screen.driver().window_moved();
  assert!(
    (screen.driver().window_rect().2 - width).abs() > 1.,
    "the same button restores it"
  );
}

#[skene_gui::test]
fn a_drag_past_a_corner_resizes_and_one_on_the_title_bar_moves() {
  let Some(screen) = decorated() else {
    return;
  };
  let (width, height) = screen.driver().size();
  let (x, y, geometry_width, geometry_height) = screen.driver().window_rect();
  let corner = (x + geometry_width + 6., y + geometry_height + 6.);

  // Inward, so the pointer stays on the screen wherever the window opened.
  screen
    .driver()
    .drag(corner, (corner.0 - 40., corner.1 - 30.));

  let (resized_width, resized_height) = screen.driver().size();

  assert!(
    (width - resized_width - 40.).abs() <= 2. && (height - resized_height - 30.).abs() <= 2.,
    "the corner's drag resizes the window: {width}x{height} to {resized_width}x{resized_height}"
  );

  screen.driver().window_moved();

  let (x, y, ..) = screen.driver().window_rect();
  let title = (x + 60., y + 12.);

  screen.driver().drag(title, (title.0 + 50., title.1 + 20.));
  screen.driver().window_moved();

  let (moved_x, moved_y, ..) = screen.driver().window_rect();

  assert!(
    (moved_x - x - 50.).abs() <= 2. && (moved_y - y - 20.).abs() <= 2.,
    "the title bar's drag moves the window: from {x}, {y} to {moved_x}, {moved_y}"
  );

  // The content is still where a reader reads it.
  screen.find_by_role(Role::Button, "Wide").click();
}

#[skene_gui::test]
fn the_close_button_asks_the_app_to_close() {
  let Some(screen) = decorated() else {
    return;
  };
  let close = screen
    .driver()
    .title_button_point(TitleButton::Close)
    .expect("the title bar has a close button");

  assert!(!screen.driver().is_closing());
  screen.driver().click_point(close);
  assert!(
    screen.driver().is_closing(),
    "a click on the close button asks the app to close"
  );
}

#[skene_gui::test]
fn a_tiled_edge_casts_no_shadow_and_a_press_at_it_reaches_the_content() {
  let Some(screen) = decorated() else {
    return;
  };

  if std::env::var("SKENE_TEST_SESSION").as_deref() != Ok("mutter") {
    eprintln!("not tested here: tiling is mutter's (Super with an arrow)");

    return;
  }

  screen.press("super+Left");
  screen.driver().window_moved();

  let (x, ..) = screen.driver().window_rect();

  assert!(
    x.abs() <= 1.,
    "tiled to the left, the window meets the screen's edge: {x}"
  );

  let wide = screen.find_by_role(Role::Button, "Wide");
  let (_, middle) = screen.driver().point_in(wide.element(), 0.5, 0.5);

  screen.driver().click_point((1., middle));
  assert_eq!(
    screen.probe("wide"),
    "1",
    "a press at the tiled edge reaches the content rather than resizing"
  );
}
