//! A view with two forms: a fallback and the form a signal makes possible.

use skene_foundation::NoChild;

use crate::ActivePlatform;

/// Builds one of two forms between its anchors: the fallback while `live` is false,
/// the child once it is true.
///
/// Only the form in force is built, and each at most once: the fallback stands where
/// the signal never turns, and where it turns later the fallback is destroyed and the
/// child built in its place, under a replacement owner, so state bound beneath keeps
/// its snapshot identity. Once built, the child stands whatever the signal does after.
/// A form builds untracked, so a signal it reads while building never rebuilds it.
/// With no fallback the segment stays empty until the signal turns. Holds [`NoChild`]
/// until [`set_child_builder`](Self::set_child_builder) supplies the child; both forms
/// are builder closures because only one of them is built.
///
/// The signals the built-ins wait on are the web's own: [`media::hydrated`](crate::media::hydrated)
/// for a render made without script, and [`Painting::available`](crate::Painting::available)
/// for a painted box's pixels.
#[must_use]
pub struct Enhance<C = NoChild>(skene_foundation::Enhance<ActivePlatform, C>);

skene_foundation::impl_enhance!(Enhance, ActivePlatform);
