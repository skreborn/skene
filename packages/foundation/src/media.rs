//! The environment as signals, and the color scheme the app puts in force, over any
//! platform; a facade forwards to these with its medium's units.

use skene_reactive::{Effect, Memo, ReadSignal, Signal};

use crate::{ColorScheme, Inset, Media, Platform, PointerPrecision, Size};

/// The viewport's size, in the medium's unit.
pub fn viewport<P: Platform>() -> ReadSignal<Size<P::Scalar>> {
  P::media().viewport
}

/// The color scheme in force: the app's choice, else the platform's.
pub fn scheme<P: Platform>() -> ReadSignal<ColorScheme> {
  P::media().scheme
}

/// Whether the reader prefers reduced motion.
pub fn reduced_motion<P: Platform>() -> ReadSignal<bool> {
  P::media().reduced_motion
}

/// Whether the reader prefers reduced transparency.
pub fn reduced_transparency<P: Platform>() -> ReadSignal<bool> {
  P::media().reduced_transparency
}

/// Whether the reader prefers more contrast.
pub fn high_contrast<P: Platform>() -> ReadSignal<bool> {
  P::media().high_contrast
}

/// How precisely the reader's primary pointer points.
pub fn pointer<P: Platform>() -> ReadSignal<PointerPrecision> {
  P::media().pointer
}

/// Whether the reader's primary pointer hovers.
pub fn hover<P: Platform>() -> ReadSignal<bool> {
  P::media().hover
}

/// The insets the display keeps clear of the viewport's edges, in the medium's unit.
pub fn safe_area<P: Platform>() -> ReadSignal<Inset<P::Scalar>> {
  P::media().safe_area
}

/// Whether a render made without script has been hydrated.
///
/// False on the server leaf and while the browser leaf adopts the server's markup, true
/// once it has finished, and true from the start on every platform whose tree is live
/// from the start.
pub fn hydrated<P: Platform>() -> ReadSignal<bool> {
  P::hydration().unwrap_or_else(|| Signal::transient(true).read_only())
}

/// Whether `media` holds, as a memo following the environment.
pub fn watch<P: Platform>(media: &Media<P::Scalar>) -> Memo<bool> {
  P::watch_media(media)
}

/// Puts `scheme` in force throughout, overriding the platform's preference.
///
/// `None` returns to following it. Every token keeps both values and the platform is
/// told which is in force, so a change re-inks without rebuilding anything.
pub fn set_scheme<P: Platform>(scheme: Option<ColorScheme>) {
  P::set_scheme(scheme);
}

/// Puts the scheme `scheme` reports in force, following it as it changes; see
/// [`set_scheme`].
pub fn derive_scheme<P: Platform>(scheme: impl Fn() -> Option<ColorScheme> + 'static) {
  Effect::new(move |_: Option<()>| set_scheme::<P>(scheme()));
}

/// The scheme the app put in force, or a previous run stored, when there is one.
#[must_use]
pub fn chosen_scheme<P: Platform>() -> Option<ColorScheme> {
  P::chosen_scheme()
}
