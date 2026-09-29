//! The environment as signals, and the color scheme the app puts in force.
//!
//! [`viewport`], [`scheme`] and [`reduced_motion`] are the platform's signals; [`watch`]
//! turns any declared [`Media`] condition into a memo. The viewport follows the last
//! rendered size, in cells; the scheme composes the app's choice with the background
//! the terminal detected; the terminal reports no reduced-motion preference of its
//! own, and a headless test sets one through `trigger_reduced_motion`.
//!
//! The scheme in force is the terminal's, not a theme's: [`set_scheme`] and
//! [`derive_scheme`] tell the platform which scheme paints, touching no node, and `None`
//! returns to following the detected background.

use skene_core::{ColorScheme, PointerPrecision};
use skene_reactive::{Memo, ReadSignal};

use crate::{ActivePlatform, Media, Size};

/// The viewport's size, in cells.
#[inline(always)]
pub fn viewport() -> ReadSignal<Size> {
  skene_common::media::viewport::<ActivePlatform>()
}

/// The color scheme in force: the app's choice, else the detected background's.
#[inline(always)]
pub fn scheme() -> ReadSignal<ColorScheme> {
  skene_common::media::scheme::<ActivePlatform>()
}

/// Whether the reader prefers reduced motion; the terminal reports no preference of
/// its own.
#[inline(always)]
pub fn reduced_motion() -> ReadSignal<bool> {
  skene_common::media::reduced_motion::<ActivePlatform>()
}

/// Whether the reader prefers reduced transparency; the terminal reports no
/// preference, so it never holds.
#[inline(always)]
pub fn reduced_transparency() -> ReadSignal<bool> {
  skene_common::media::reduced_transparency::<ActivePlatform>()
}

/// Whether the reader prefers more contrast; the terminal reports no preference, so
/// it never holds.
#[inline(always)]
pub fn high_contrast() -> ReadSignal<bool> {
  skene_common::media::high_contrast::<ActivePlatform>()
}

/// How precisely the reader's primary pointer points: finely, the mouse, where the
/// app takes the mouse, and absent where it does not.
#[inline(always)]
pub fn pointer() -> ReadSignal<PointerPrecision> {
  skene_common::media::pointer::<ActivePlatform>()
}

/// Whether the reader's primary pointer hovers: where the app takes the mouse.
#[inline(always)]
pub fn hover() -> ReadSignal<bool> {
  skene_common::media::hover::<ActivePlatform>()
}

/// Whether `media` holds, as a memo following the environment. A container condition
/// never holds here: it is answered per node, by the box it names.
#[inline(always)]
pub fn watch(media: &Media) -> Memo<bool> {
  skene_common::media::watch::<ActivePlatform>(media)
}

/// Paints `scheme` throughout, overriding the background the terminal detected; `None`
/// returns to following it.
///
/// Every token keeps both values and the paint is told which is in force, so a change
/// re-inks the next frame without rebuilding anything: the state inside the tree, the
/// focus, the scroll position and a half-typed entry all survive it.
#[inline(always)]
pub fn set_scheme(scheme: Option<ColorScheme>) {
  skene_common::media::set_scheme::<ActivePlatform>(scheme);
}

/// Paints the scheme `scheme` reports, following it as it changes; see [`set_scheme`].
///
/// Bind it to the signal holding the reader's setting.
#[inline(always)]
pub fn derive_scheme(scheme: impl Fn() -> Option<ColorScheme> + 'static) {
  skene_common::media::derive_scheme::<ActivePlatform>(scheme);
}

/// The color scheme the app chose through [`set_scheme`], if any.
#[inline(always)]
#[must_use]
pub fn stored_scheme() -> Option<ColorScheme> {
  skene_common::media::stored_scheme::<ActivePlatform>()
}
