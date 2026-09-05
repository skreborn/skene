//! The environment as signals, and the color scheme the app puts in force.
//!
//! [`viewport`], [`scheme`] and [`reduced_motion`] are the driver's signals; [`watch`]
//! turns any declared [`Media`] condition into a memo. The viewport follows the last
//! rendered size, in cells; the scheme composes the app's choice with the background
//! the terminal detected; the terminal reports no reduced-motion preference of its
//! own, and a headless test sets one through `trigger_reduced_motion`.
//!
//! The scheme in force is the terminal's, not a theme's: [`set_scheme`] and
//! [`derive_scheme`] tell the driver which scheme paints, touching no node, and `None`
//! returns to following the detected background.

use skene_core::{ColorScheme, ContainerName, Driver, MediaEnv};
use skene_reactive::{Effect, Get, Memo, ReadSignal, Set};

use crate::{ActiveDriver, Media, Size, with_store};

/// The viewport's size, in cells.
#[inline(always)]
pub fn viewport() -> ReadSignal<Size> {
  ActiveDriver::default().media().viewport
}

/// The color scheme in force: the app's choice, else the detected background's.
#[inline(always)]
pub fn scheme() -> ReadSignal<ColorScheme> {
  ActiveDriver::default().media().scheme
}

/// Whether the reader prefers reduced motion; the terminal reports no preference of
/// its own.
#[inline(always)]
pub fn reduced_motion() -> ReadSignal<bool> {
  ActiveDriver::default().media().reduced_motion
}

/// Whether `media` holds, as a memo following the environment.
///
/// A container condition never holds here: it is answered per node, by the box it
/// names.
pub fn watch(media: &Media) -> Memo<bool> {
  let media = media.clone();
  let signals = ActiveDriver::default().media();

  Memo::new(move |_| {
    let container = |_: Option<ContainerName>| None;

    media.holds_in(&MediaEnv {
      viewport: signals.viewport.get(),
      scheme: signals.scheme.get(),
      reduced_motion: signals.reduced_motion.get(),
      reduced_transparency: false,
      high_contrast: false,
      container: &container,
    })
  })
}

/// Paints `scheme` throughout, overriding the background the terminal detected; `None`
/// returns to following it.
///
/// Every token keeps both values and the paint is told which is in force, so a change
/// re-inks the next frame without rebuilding anything: the state inside the tree, the
/// focus, the scroll position and a half-typed entry all survive it.
pub fn set_scheme(scheme: Option<ColorScheme>) {
  let write = with_store(|store| {
    let before = store.scheme();

    store.choice = scheme;

    let after = store.scheme();

    if before == after {
      return None;
    }

    store.invalidate_styles();

    Some((store.media().set_scheme, after))
  });

  if let Some((write, scheme)) = write {
    write.set(scheme);
  }
}

/// Paints the scheme `scheme` reports, following it as it changes; see [`set_scheme`].
///
/// Bind it to the signal holding the reader's setting.
#[inline(always)]
pub fn derive_scheme(scheme: impl Fn() -> Option<ColorScheme> + 'static) {
  Effect::new(move |_: Option<()>| set_scheme(scheme()));
}

/// The color scheme the app chose through [`set_scheme`], if any.
#[inline(always)]
#[must_use]
pub fn stored_scheme() -> Option<ColorScheme> {
  with_store(|store| store.choice)
}
