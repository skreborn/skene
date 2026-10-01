//! A view with two forms: a fallback and the form a signal makes possible.

use skene_foundation::{AnyKind, ChildBuilder, NoChild, NodeId, View};
use skene_reactive::ReadSignal;

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
#[derive(Debug)]
#[must_use]
pub struct Enhance<C = NoChild>(skene_foundation::Enhance<ActivePlatform, C>);

impl Enhance {
  /// An enhancement waiting on `live`, with no forms yet.
  #[inline(always)]
  pub const fn new(live: ReadSignal<bool>) -> Self {
    Self(skene_foundation::Enhance::new(live))
  }

  /// Sets the form built while `live` is false; nothing by default.
  #[inline(always)]
  pub fn set_fallback<V: View<ActivePlatform>>(
    self,
    fallback: impl FnOnce() -> V + 'static,
  ) -> Self {
    Self(self.0.set_fallback(fallback))
  }

  /// Sets the form built once `live` is true.
  #[inline(always)]
  pub fn set_child_builder<V: View<ActivePlatform>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Enhance<ChildBuilder<ActivePlatform>> {
    Enhance(self.0.set_child_builder(child))
  }
}

impl<C> AnyKind for Enhance<C> {}

impl View<ActivePlatform> for Enhance<ChildBuilder<ActivePlatform>> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
