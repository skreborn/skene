//! The environment as signals, and the color scheme the app puts in force.
//!
//! [`viewport`], [`scheme`] and [`reduced_motion`] are the platform's signals; [`watch`]
//! turns any declared [`Media`] condition into a memo. In the browser they are live and
//! a condition's memo is built from the same query text the stylesheet renders; on the
//! server leaf they hold the client hints the render was given, else the documented
//! defaults.
//!
//! The scheme in force is the document's, not a theme's: [`set_scheme`] and
//! [`derive_scheme`] tell the platform which scheme paints, touching no node, and `None`
//! returns to following the platform. [`chosen_scheme`] reads the choice in force, the
//! one a previous visit stored included, so an app's setting starts where the page
//! already is.

use skene_foundation::{ColorScheme, Inset, PointerPrecision};
use skene_reactive::{Memo, ReadSignal};

use crate::{Active, Media, Size};

/// The viewport's size, in logical pixels.
#[inline(always)]
pub fn viewport() -> ReadSignal<Size> {
  skene_foundation::media::viewport::<Active>()
}

/// The color scheme in force: the app's choice, else the platform's preference.
#[inline(always)]
pub fn scheme() -> ReadSignal<ColorScheme> {
  skene_foundation::media::scheme::<Active>()
}

/// Whether the reader prefers reduced motion.
#[inline(always)]
pub fn reduced_motion() -> ReadSignal<bool> {
  skene_foundation::media::reduced_motion::<Active>()
}

/// Whether the reader prefers reduced transparency.
#[inline(always)]
pub fn reduced_transparency() -> ReadSignal<bool> {
  skene_foundation::media::reduced_transparency::<Active>()
}

/// Whether the reader prefers more contrast.
#[inline(always)]
pub fn high_contrast() -> ReadSignal<bool> {
  skene_foundation::media::high_contrast::<Active>()
}

/// How precisely the reader's primary pointer points; a fine one before the bundle
/// arrives, no request header saying.
#[inline(always)]
pub fn pointer() -> ReadSignal<PointerPrecision> {
  skene_foundation::media::pointer::<Active>()
}

/// Whether the reader's primary pointer hovers; it does before the bundle arrives, no
/// request header saying.
#[inline(always)]
pub fn hover() -> ReadSignal<bool> {
  skene_foundation::media::hover::<Active>()
}

/// The insets the display keeps clear of the viewport's edges, in logical pixels.
///
/// None before the bundle arrives, no request header saying, while a length reading
/// them through `Expr::safe_area` holds them from the first paint.
#[inline(always)]
pub fn safe_area() -> ReadSignal<Inset<f32>> {
  skene_foundation::media::safe_area::<Active>()
}

/// Whether the page stands hydrated, as a signal.
///
/// False on the server leaf and while the browser leaf adopts the server's markup, true
/// once it has finished and on a native platform from the start, where no render is
/// made without script; what an [`Enhance`](crate::Enhance) over the bundle waits on.
#[inline(always)]
pub fn hydrated() -> ReadSignal<bool> {
  skene_foundation::media::hydrated::<Active>()
}

/// Whether `media` holds, as a memo following the environment. A container condition
/// never holds here: it is answered per node, by the box it names.
#[inline(always)]
pub fn watch(media: &Media) -> Memo<bool> {
  skene_foundation::media::watch::<Active>(media)
}

/// Paints `scheme` throughout, overriding the reader's preference; `None` returns to
/// following it.
///
/// A change re-inks the page without rebuilding anything: the state inside the tree,
/// the focus, the scroll position and a half-typed input all survive it. On the server
/// leaf it marks the page as one whose reader may choose, so its head carries the
/// blocking script applying the stored choice before the first paint.
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

/// The color scheme chosen for the document, when there is one: the app's choice, or
/// the reader's on a previous visit.
///
/// Initialize the setting bound to [`derive_scheme`] from this, so the choice the page
/// was already painted in is the one the app goes on to hold. A server never knows it,
/// and a reader who has chosen nothing gets `None`: the platform's own preference,
/// which the styling answers with no script at all.
#[inline(always)]
#[must_use]
pub fn chosen_scheme() -> Option<ColorScheme> {
  skene_foundation::media::chosen_scheme::<Active>()
}
