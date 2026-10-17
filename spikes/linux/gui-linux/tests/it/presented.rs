//! What reaches the screen, read back from the display server or the compositor: a change the
//! window draws while it answers a reader (which lays the window out at once, so the reader
//! reads current geometry) is presented as one a click makes is, rather than drawn into the
//! window's frame and never shown.

use skene_gui::{
  AccessRequest, Color, Media, Scheme, Signal, When as _, media,
  prelude::*,
  testing::{self, Role, Screen},
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
