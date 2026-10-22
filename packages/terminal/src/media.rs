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

use skene_foundation::{ColorScheme, Inset, PointerPrecision};
use skene_reactive::{Memo, ReadSignal};

use crate::{Active, Media, Size};

/// The viewport's size, in cells.
#[inline(always)]
pub fn viewport() -> ReadSignal<Size> {
  skene_foundation::media::viewport::<Active>()
}

/// The color scheme in force: the app's choice, else the detected background's.
#[inline(always)]
pub fn scheme() -> ReadSignal<ColorScheme> {
  skene_foundation::media::scheme::<Active>()
}

/// Whether the reader prefers reduced motion; the terminal reports no preference of
/// its own.
#[inline(always)]
pub fn reduced_motion() -> ReadSignal<bool> {
  skene_foundation::media::reduced_motion::<Active>()
}

/// Whether the reader prefers reduced transparency; the terminal reports no
/// preference, so it never holds.
#[inline(always)]
pub fn reduced_transparency() -> ReadSignal<bool> {
  skene_foundation::media::reduced_transparency::<Active>()
}

/// Whether the reader prefers more contrast; the terminal reports no preference, so
/// it never holds.
#[inline(always)]
pub fn high_contrast() -> ReadSignal<bool> {
  skene_foundation::media::high_contrast::<Active>()
}

/// How precisely the reader's primary pointer points: finely, the mouse, where the
/// app takes the mouse, and absent where it does not.
#[inline(always)]
pub fn pointer() -> ReadSignal<PointerPrecision> {
  skene_foundation::media::pointer::<Active>()
}

/// Whether the reader's primary pointer hovers: where the app takes the mouse.
#[inline(always)]
pub fn hover() -> ReadSignal<bool> {
  skene_foundation::media::hover::<Active>()
}

/// The insets the display keeps clear of the viewport's edges; a terminal keeps none.
#[inline(always)]
pub fn safe_area() -> ReadSignal<Inset<u32>> {
  skene_foundation::media::safe_area::<Active>()
}

/// Whether `media` holds, as a memo following the environment. A container condition
/// never holds here: it is answered per node, by the box it names.
#[inline(always)]
pub fn watch(media: &Media) -> Memo<bool> {
  skene_foundation::media::watch::<Active>(media)
}

/// Paints `scheme` throughout, overriding the background the terminal detected; `None`
/// returns to following it.
///
/// A change re-inks the next frame without rebuilding anything: the state inside the
/// tree, the focus, the scroll position and a half-typed entry all survive it.
#[inline(always)]
pub fn set_scheme(scheme: Option<ColorScheme>) {
  skene_foundation::media::set_scheme::<Active>(scheme);
}

/// Paints the scheme `scheme` reports, following it as it changes; see [`set_scheme`].
///
/// Bind it to the signal holding the reader's setting.
pub fn derive_scheme(scheme: impl Fn() -> Option<ColorScheme> + 'static) {
  skene_foundation::media::derive_scheme::<Active>(scheme);
}

/// The color scheme the app chose through [`set_scheme`], if any.
#[inline(always)]
#[must_use]
pub fn chosen_scheme() -> Option<ColorScheme> {
  skene_foundation::media::chosen_scheme::<Active>()
}
