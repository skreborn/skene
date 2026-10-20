//! What reaches the screen, read back from the display server or the compositor: a change the
//! window draws while it answers a reader (which lays the window out at once, so the reader
//! reads current geometry) is presented as one a click makes is, rather than drawn into the
//! window's frame and never shown; a part of the window drawn again alone shows, pixel for
//! pixel, what the whole window drawn at once shows there, a shadow included; and a box's
//! shadow shows outside the box alone.

use skene_gui::{
  AccessRequest, Color, Inset, Media, Scheme, Shadow, Signal, When as _, media,
  prelude::*,
  testing::{self, Role, Screen, Screenshot},
};

/// The page's background in the light scheme.
const LIGHT: Color = Color::rgb(0xf5f5f7);

/// The page's background in the dark scheme.
const DARK: Color = Color::rgb(0x1c1c1e);

/// A page whose "Dark mode" switch chooses the scheme, recoloring the page's background, as
/// the notes app's does.
fn page() -> impl View {
  let dark = Signal::new(false);

  media::derive_scheme(move || dark.get().then_some(Scheme::Dark));

  Container::new()
    .set_background(LIGHT.when(Media::Dark, DARK))
    .set_padding(16)
    .set_child(
      Column::new()
        .set_spacing(12)
        .add_child(Text::new("Settings"))
        .add_child(Toggle::new("Dark mode").bind_on(dark)),
    )
}

/// The background as the screen shows it, at the content's top left corner.
fn background(screen: &Screen) -> Color {
  screen.pixel(4, 4)
}

#[skene_gui::test]
fn a_switch_a_reader_flips_recolors_the_screen() {
  let screen = testing::render(page);
  let switch = screen.find_by_role(Role::Switch, "Dark mode");

  assert_eq!(background(&screen), LIGHT);

  // A reader's action is carried out on the window's loop, which lays the window out (and so
  // draws it) before the reader hears it done; the frame it drew is presented after.
  switch.trigger_access(AccessRequest::Toggle);
  assert!(switch.is_on(), "the reader flipped the switch");
  assert_eq!(
    background(&screen),
    DARK,
    "the window drawn for the reader reaches the screen"
  );

  switch.trigger_access(AccessRequest::Toggle);
  assert!(!switch.is_on());
  assert_eq!(background(&screen), LIGHT);
}

#[skene_gui::test]
fn a_switch_clicked_recolors_the_screen() {
  let screen = testing::render(page);
  let switch = screen.find_by_role(Role::Switch, "Dark mode");

  assert_eq!(background(&screen), LIGHT);

  switch.click();
  assert!(switch.is_on());
  assert_eq!(background(&screen), DARK);

  switch.click();
  assert!(!switch.is_on());
  assert_eq!(background(&screen), LIGHT);
}

/// The page beneath everything: a mid gray, which a translucent paint over it shows the
/// rounding of.
const PAGE: Color = Color::rgb(0x808080);

/// A translucent white veil over the page, along the plate's row.
const VEIL: Color = Color::rgba(0xffff_ff80);

/// The plate unlit.
const PLATE: Color = Color::rgb(0x2f6fdf);

/// The plate lit.
const PLATE_LIT: Color = Color::rgb(0x1f57b8);

/// What the plate says.
const WORDS: &str = "Grumpy wizards make toxic brew";

/// The plate's padding: top, trailing, bottom, leading.
const PLATE_PADDING: (f32, f32, f32, f32) = (7., 11., 9., 5.);

/// A mid-gray page holding, 13 pixels in, a translucent veil along a row: a 31-pixel gap, a
/// blue plate holding white text, its left edge 53 pixels in (off any grid of 2 or 4 pixels),
/// and a switch that lights the plate.
pub(crate) fn plate() -> impl View {
  let lit = Signal::new(false);
  let (top, trailing, bottom, leading) = PLATE_PADDING;

  Container::new().set_background(PAGE).set_child(
    Column::new().set_padding(13).add_child(
      Container::new().set_background(VEIL).set_child(
        Row::new()
          .set_spacing(9)
          .set_item_alignment(AxisAlignment::Center)
          .add_child(Container::new().set_width(31).set_height(8))
          .add_child(
            Container::new()
              .set_padding(Inset::new(top, trailing, bottom, leading))
              .derive_background(move || if lit.get() { PLATE_LIT } else { PLATE })
              .set_child(Text::new(WORDS).set_color(Color::WHITE).set_font_size(15)),
          )
          .add_child(Toggle::new("Lit").bind_on(lit)),
      ),
    ),
  )
}

#[skene_gui::test]
fn a_box_drawn_again_alone_shows_what_the_whole_window_drew() {
  let screen = testing::render(plate);
  let text = screen.find_by_text(WORDS).read().bounds;
  let switch = screen.find_by_role(Role::Switch, "Lit");
  let (top, _, _, leading) = PLATE_PADDING;
  let corner = (text.x - leading + 2., text.y - top + 2.);

  // The window's first frame, drawn whole.
  let whole = screen.render_to_image();

  assert_eq!(screen.pixel(corner.0, corner.1), PLATE, "the plate unlit");

  // A reader's flips, which move no pointer: each draws the plate and the switch again, alone,
  // over the page and the veil beside them.
  switch.trigger_access(AccessRequest::Toggle);
  assert_eq!(screen.pixel(corner.0, corner.1), PLATE_LIT, "the plate lit");
  switch.trigger_access(AccessRequest::Toggle);
  assert_eq!(
    screen.pixel(corner.0, corner.1),
    PLATE,
    "the plate unlit again"
  );

  assert_drawn_alike(&whole, &screen.render_to_image());
}

/// Asserts that `again`, the window drawn again in part, shows pixel for pixel what `whole`,
/// the window drawn whole, showed.
pub(crate) fn assert_drawn_alike(whole: &Screenshot, again: &Screenshot) {
  let (before, _) = whole.rgba.as_chunks::<4>();
  let (after, _) = again.rgba.as_chunks::<4>();
  let mut differing = 0;
  let mut most = 0;
  let mut first = None;

  for (index, (was, is)) in before.iter().zip(after).enumerate() {
    for (was, is) in was.iter().zip(is) {
      most = most.max(was.abs_diff(*is));
    }

    if was != is {
      differing += 1;
      first = first.or(Some((
        index % whole.width as usize,
        index / whole.width as usize,
      )));
    }
  }

  assert_eq!(
    (whole.width, whole.height, differing, most),
    (again.width, again.height, 0, 0),
    "the window drawn again in part shows {differing} pixels unlike the window drawn whole, a \
     channel at most {most} apart, the first at {first:?} (device pixels)"
  );
}

/// A glass pane's height.
const PANE: f32 = 60.;

/// A mid-gray page holding a pane of translucent white casting a black shadow 16 pixels down,
/// and beside it a plate a switch lights.
fn glass() -> impl View {
  let lit = Signal::new(false);

  Container::new().set_background(PAGE).set_child(
    Column::new().set_padding(24).add_child(
      Row::new()
        .set_spacing(24)
        .add_child(
          Container::new()
            .set_width(120)
            .set_height(PANE)
            .set_background(VEIL)
            .set_corner_radius(8)
            .set_shadow(Shadow {
              x: 0.,
              y: 16.,
              blur: 6.,
              spread: 0.,
              color: Color::BLACK,
              inset: false,
            })
            .set_child(Text::new("Glass")),
        )
        .add_child(
          Container::new()
            .set_padding(8)
            .derive_background(move || if lit.get() { PLATE_LIT } else { PLATE })
            .set_child(Toggle::new("Lit").bind_on(lit)),
        ),
    ),
  )
}

#[skene_gui::test]
fn a_shadow_shows_outside_its_box_alone_and_drawn_again_shows_as_it_did() {
  let screen = testing::render(glass);
  let pane = screen.find_by_text("Glass").read().bounds;
  let switch = screen.find_by_role(Role::Switch, "Lit");
  // The pane's top left: the caption's, the pane holding nothing else.
  let (x, y) = (pane.x + 60., pane.y);
  let whole = screen.render_to_image();

  // As a CSS box shadow: the pane's own area is cut out of its shadow, so the shadow cast
  // beneath the translucent pane does not show through it.
  assert_eq!(
    screen.pixel(x, y + PANE - 3.),
    screen.pixel(x, y + 3.),
    "the pane shows the page alike near its foot and near its head"
  );

  let below = screen.pixel(x, y + PANE + 8.);

  assert!(
    below.red < PAGE.red / 2,
    "the shadow shows below the pane: {below:?}"
  );

  // A reader's flips draw the plate again alone, the band from the window's leading edge
  // passing over the shadow.
  switch.trigger_access(AccessRequest::Toggle);
  switch.trigger_access(AccessRequest::Toggle);
  assert!(!switch.is_on());
  assert_drawn_alike(&whole, &screen.render_to_image());
}
