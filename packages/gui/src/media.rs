//! The environment as signals, and the color scheme the app puts in force.
//!
//! [`viewport`], [`scheme`] and [`reduced_motion`] are the driver's signals; [`watch`]
//! turns any declared [`Media`] condition into a memo. In the browser they are live;
//! on the server leaf they hold the client hints the render was given, else the
//! documented defaults; on the host they hold what the headless driver's seams set.
//!
//! The scheme in force is the document's, not a theme's: [`set_scheme`] and
//! [`derive_scheme`] tell the driver which scheme paints, touching no node, and `None`
//! returns to following the platform. [`stored_scheme`] reads the choice a previous
//! visit stored, so an app's setting starts where the page already is.

use skene_core::{ColorScheme, Driver};
use skene_reactive::{Effect, Memo, ReadSignal};

use crate::{ActiveDriver, Media, Size};

/// The viewport's size, in logical pixels.
#[inline(always)]
pub fn viewport() -> ReadSignal<Size> {
  ActiveDriver::default().media().viewport
}

/// The color scheme in force: the app's choice, else the platform's preference.
#[inline(always)]
pub fn scheme() -> ReadSignal<ColorScheme> {
  ActiveDriver::default().media().scheme
}

/// Whether the reader prefers reduced motion.
#[inline(always)]
pub fn reduced_motion() -> ReadSignal<bool> {
  ActiveDriver::default().media().reduced_motion
}

/// Whether `media` holds, as a memo following the environment.
///
/// In the browser the memo is built from the same query text the stylesheet renders,
/// so the signal and the sheet cannot disagree. A container condition never holds
/// here: it is answered per node, by the box it names.
pub fn watch(media: &Media) -> Memo<bool> {
  #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
  {
    skene_web::watch_media(media)
  }

  #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
  {
    use skene_core::{ContainerName, MediaEnv};
    use skene_reactive::Get;

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
}

/// Paints `scheme` throughout, overriding the reader's preference; `None` returns to
/// following it.
///
/// Every token keeps both values and the platform is told which is in force, so a
/// change re-inks the page without rebuilding anything: the state inside the tree,
/// the focus, the scroll position and a half-typed input all survive it. On the
/// server leaf it marks the page as one whose reader may choose, so its head carries
/// the blocking script applying the stored choice before the first paint.
#[inline(always)]
pub fn set_scheme(scheme: Option<ColorScheme>) {
  #[cfg(target_arch = "wasm32")]
  skene_web::set_scheme(scheme);

  #[cfg(not(target_arch = "wasm32"))]
  skene_headless::choose_scheme(scheme);
}

/// Paints the scheme `scheme` reports, following it as it changes; see [`set_scheme`].
///
/// Bind it to the signal holding the reader's setting.
#[inline(always)]
pub fn derive_scheme(scheme: impl Fn() -> Option<ColorScheme> + 'static) {
  Effect::new(move |_: Option<()>| set_scheme(scheme()));
}

/// The color scheme the reader chose on a previous visit, when there is one.
///
/// Initialize the setting bound to [`derive_scheme`] from this, so the choice the page
/// was already painted in is the one the app goes on to hold. A server never knows it,
/// and a reader who has chosen nothing gets `None`: the platform's own preference,
/// which the styling answers with no script at all.
#[inline(always)]
#[must_use]
pub fn stored_scheme() -> Option<ColorScheme> {
  #[cfg(target_arch = "wasm32")]
  {
    skene_web::stored_scheme()
  }

  #[cfg(not(target_arch = "wasm32"))]
  {
    skene_headless::chosen_scheme()
  }
}
