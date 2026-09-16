//! The environment as signals, and the color scheme the app puts in force, over any
//! driver; a facade forwards to these with its medium's units.

use skene_core::{ColorScheme, Driver, Media, Size};
use skene_reactive::{Effect, Memo, ReadSignal, Signal};

/// The viewport's size, in the medium's unit.
#[inline(always)]
pub fn viewport<D: Driver>() -> ReadSignal<Size<D::Scalar>> {
  D::default().media().viewport
}

/// The color scheme in force: the app's choice, else the platform's.
#[inline(always)]
pub fn scheme<D: Driver>() -> ReadSignal<ColorScheme> {
  D::default().media().scheme
}

/// Whether the reader prefers reduced motion.
#[inline(always)]
pub fn reduced_motion<D: Driver>() -> ReadSignal<bool> {
  D::default().media().reduced_motion
}

/// Whether a render made without script has been hydrated: false on the server leaf
/// and while the browser leaf adopts the server's markup, true once it has finished,
/// and true from the start on every driver whose tree is live from the start.
#[inline(always)]
pub fn hydrated<D: Driver>() -> ReadSignal<bool> {
  D::default()
    .hydration()
    .unwrap_or_else(|| Signal::opaque(true).read_only())
}

/// Whether `media` holds, as a memo following the environment.
#[inline(always)]
pub fn watch<D: Driver>(media: &Media<D::Scalar>) -> Memo<bool> {
  D::default().watch_media(media)
}

/// Puts `scheme` in force throughout, overriding the platform's preference; `None`
/// returns to following it. Every token keeps both values and the driver is told which
/// is in force, so a change re-inks without rebuilding anything.
#[inline(always)]
pub fn set_scheme<D: Driver>(scheme: Option<ColorScheme>) {
  D::default().set_scheme(scheme);
}

/// Puts the scheme `scheme` reports in force, following it as it changes; see
/// [`set_scheme`].
#[inline(always)]
pub fn derive_scheme<D: Driver>(scheme: impl Fn() -> Option<ColorScheme> + 'static) {
  Effect::new(move |_: Option<()>| set_scheme::<D>(scheme()));
}

/// The scheme the app put in force, or a previous run stored, when there is one.
#[inline(always)]
#[must_use]
pub fn stored_scheme<D: Driver>() -> Option<ColorScheme> {
  D::default().stored_scheme()
}
