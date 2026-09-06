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

use skene_core::ColorScheme;
use skene_reactive::{Memo, ReadSignal};

use crate::{ActiveDriver, Media, Size};

/// The viewport's size, in cells.
#[inline(always)]
pub fn viewport() -> ReadSignal<Size> {
  skene_views::media::viewport::<ActiveDriver>()
}

/// The color scheme in force: the app's choice, else the detected background's.
#[inline(always)]
pub fn scheme() -> ReadSignal<ColorScheme> {
  skene_views::media::scheme::<ActiveDriver>()
}

/// Whether the reader prefers reduced motion; the terminal reports no preference of
/// its own.
#[inline(always)]
pub fn reduced_motion() -> ReadSignal<bool> {
  skene_views::media::reduced_motion::<ActiveDriver>()
}

/// Whether `media` holds, as a memo following the environment. A container condition
/// never holds here: it is answered per node, by the box it names.
#[inline(always)]
pub fn watch(media: &Media) -> Memo<bool> {
  skene_views::media::watch::<ActiveDriver>(media)
}

/// Paints `scheme` throughout, overriding the background the terminal detected; `None`
/// returns to following it.
///
/// Every token keeps both values and the paint is told which is in force, so a change
/// re-inks the next frame without rebuilding anything: the state inside the tree, the
/// focus, the scroll position and a half-typed entry all survive it.
#[inline(always)]
pub fn set_scheme(scheme: Option<ColorScheme>) {
  skene_views::media::set_scheme::<ActiveDriver>(scheme);
}

/// Paints the scheme `scheme` reports, following it as it changes; see [`set_scheme`].
///
/// Bind it to the signal holding the reader's setting.
#[inline(always)]
pub fn derive_scheme(scheme: impl Fn() -> Option<ColorScheme> + 'static) {
  skene_views::media::derive_scheme::<ActiveDriver>(scheme);
}

/// The color scheme the app chose through [`set_scheme`], if any.
#[inline(always)]
#[must_use]
pub fn stored_scheme() -> Option<ColorScheme> {
  skene_views::media::stored_scheme::<ActiveDriver>()
}
