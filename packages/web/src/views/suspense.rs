//! Fallback content while a subtree's async work is pending.

use skene_foundation::{AnyKind, ChildBuilder, NoChild, NodeId, View};

use crate::ActivePlatform;

/// Shows its content once that subtree's async work settles, and the fallback until
/// then.
///
/// The content builds at once under a fresh [`Boundary`](skene_foundation::Boundary), so its tasks
/// start right away and report there; content and fallback are each built once and attached or
/// detached between anchors as the boundary empties and refills, so state inside survives the
/// swaps, and both are destroyed when the owner the view was built under is disposed.
/// Toward an enclosing boundary the whole subtree counts as one pending task. Holds
/// [`NoChild`] until [`set_child_builder`](Self::set_child_builder) supplies the
/// content; it is a builder closure because it is built under the owner the boundary
/// is provided on.
#[derive(Debug)]
#[must_use]
pub struct Suspense<C = NoChild>(skene_foundation::Suspense<ActivePlatform, C>);

impl Default for Suspense {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Suspense {
  /// A suspense with no fallback and no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self(skene_foundation::Suspense::new())
  }

  /// Sets the fallback shown while the content's work is pending; nothing by default.
  #[inline(always)]
  pub fn set_fallback(self, fallback: impl View<ActivePlatform>) -> Self {
    Self(self.0.set_fallback(fallback))
  }

  /// Sets the content, built by `child` under the boundary.
  #[inline(always)]
  pub fn set_child_builder<V: View<ActivePlatform>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Suspense<ChildBuilder<ActivePlatform>> {
    Suspense(self.0.set_child_builder(child))
  }
}

impl<C> AnyKind for Suspense<C> {}

impl View<ActivePlatform> for Suspense<ChildBuilder<ActivePlatform>> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
