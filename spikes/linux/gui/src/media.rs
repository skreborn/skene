//! The environment the app is shown in, as signals, and the choices an app makes about it.
//!
//! Each reader is a [`Memo`] of what the platform reports (on Linux: the window's size and
//! scale, the XDG settings portal's scheme, contrast, accent and reduced motion, and XSETTINGS
//! where no portal answers), so a view that reads one follows it. A value that only varies
//! with the environment needs none of them: it declares its condition
//! (`16.when(Media::min_width(900), 24)`).

use skene_foundation::{Effect, Environment, seam::Realization as _};

use crate::{Active, Media, Memo, Scheme};

/// The window's width, in pixels.
#[must_use]
pub fn width() -> Memo<f32> {
  let width = Environment::current().width;

  Memo::new(move || width.get())
}

/// The window's height, in pixels.
#[must_use]
pub fn height() -> Memo<f32> {
  let height = Environment::current().height;

  Memo::new(move || height.get())
}

/// The scheme in force: the app's choice where it made one ([`set_scheme`]), the reader's
/// otherwise.
#[must_use]
pub fn scheme() -> Memo<Scheme> {
  let dark = Environment::current().dark;

  Memo::new(move || {
    if dark.get() {
      Scheme::Dark
    } else {
      Scheme::Light
    }
  })
}

/// Whether the reader asked for more contrast.
#[must_use]
pub fn high_contrast() -> Memo<bool> {
  let high_contrast = Environment::current().high_contrast;

  Memo::new(move || high_contrast.get())
}

/// Whether the reader asked for reduced motion; transitions are cut where they did.
#[must_use]
pub fn reduced_motion() -> Memo<bool> {
  let reduced_motion = Environment::current().reduced_motion;

  Memo::new(move || reduced_motion.get())
}

/// Device pixels per pixel, fractional where the display is.
#[must_use]
pub fn scale() -> Memo<f64> {
  let scale = Environment::current().scale;

  Memo::new(move || scale.get())
}

/// Whether the layout direction is right to left.
#[must_use]
pub fn right_to_left() -> Memo<bool> {
  let right_to_left = Environment::current().right_to_left;

  Memo::new(move || right_to_left.get())
}

/// Whether `condition` holds, kept current: any condition a value could declare.
#[must_use]
pub fn watch(condition: Media) -> Memo<bool> {
  Memo::new(move || Environment::current().holds(&condition))
}

/// Chooses the scheme for the whole app, or follows the reader's again (`None`). The
/// platform resolves it: nothing is rebuilt, and [`scheme`] follows.
pub fn set_scheme(choice: Option<Scheme>) {
  Active::set_scheme(choice);
}

/// Chooses the scheme `choice` returns, kept current.
pub fn derive_scheme(choice: impl Fn() -> Option<Scheme> + 'static) {
  Effect::new(move || {
    let choice = choice();

    skene_foundation::untrack(|| set_scheme(choice));
  });
}
