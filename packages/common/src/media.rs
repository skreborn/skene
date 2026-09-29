//! The environment as signals, and the color scheme the app puts in force, over any
//! platform; a facade forwards to these with its medium's units.

use skene_core::{ColorScheme, Media, Platform, PointerPrecision, Size};
use skene_reactive::{Effect, Memo, ReadSignal, Signal};

/// The viewport's size, in the medium's unit.
#[inline(always)]
pub fn viewport<P: Platform>() -> ReadSignal<Size<P::Scalar>> {
  P::default().media().viewport
}

/// The color scheme in force: the app's choice, else the platform's.
#[inline(always)]
pub fn scheme<P: Platform>() -> ReadSignal<ColorScheme> {
  P::default().media().scheme
}

/// Whether the reader prefers reduced motion.
#[inline(always)]
pub fn reduced_motion<P: Platform>() -> ReadSignal<bool> {
  P::default().media().reduced_motion
}

/// Whether the reader prefers reduced transparency.
#[inline(always)]
pub fn reduced_transparency<P: Platform>() -> ReadSignal<bool> {
  P::default().media().reduced_transparency
}

/// Whether the reader prefers more contrast.
#[inline(always)]
pub fn high_contrast<P: Platform>() -> ReadSignal<bool> {
  P::default().media().high_contrast
}

/// How precisely the reader's primary pointer points.
#[inline(always)]
pub fn pointer<P: Platform>() -> ReadSignal<PointerPrecision> {
  P::default().media().pointer
}

/// Whether the reader's primary pointer hovers.
#[inline(always)]
pub fn hover<P: Platform>() -> ReadSignal<bool> {
  P::default().media().hover
}

/// Whether a render made without script has been hydrated.
///
/// False on the server leaf and while the browser leaf adopts the server's markup, true
/// once it has finished, and true from the start on every platform whose tree is live
/// from the start.
#[inline(always)]
pub fn hydrated<P: Platform>() -> ReadSignal<bool> {
  P::default()
    .hydration()
    .unwrap_or_else(|| Signal::opaque(true).read_only())
}

/// Whether `media` holds, as a memo following the environment.
#[inline(always)]
pub fn watch<P: Platform>(media: &Media<P::Scalar>) -> Memo<bool> {
  P::default().watch_media(media)
}

/// Puts `scheme` in force throughout, overriding the platform's preference.
///
/// `None` returns to following it. Every token keeps both values and the platform is
/// told which is in force, so a change re-inks without rebuilding anything.
#[inline(always)]
pub fn set_scheme<P: Platform>(scheme: Option<ColorScheme>) {
  P::default().set_scheme(scheme);
}

/// Puts the scheme `scheme` reports in force, following it as it changes; see
/// [`set_scheme`].
#[inline(always)]
pub fn derive_scheme<P: Platform>(scheme: impl Fn() -> Option<ColorScheme> + 'static) {
  Effect::new(move |_: Option<()>| set_scheme::<P>(scheme()));
}

/// The scheme the app put in force, or a previous run stored, when there is one.
#[inline(always)]
#[must_use]
pub fn stored_scheme<P: Platform>() -> Option<ColorScheme> {
  P::default().stored_scheme()
}
