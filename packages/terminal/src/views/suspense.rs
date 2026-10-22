//! Fallback content while a subtree's async work is pending.

use skene_foundation::{ChildBuilder, NoChild, SuspenseBase, View, Wraps};

use crate::Active;

/// Shows its content once that subtree's async work settles, and the fallback until
/// then.
///
/// The content builds at once under a fresh [`Boundary`](skene_foundation::Boundary),
/// so its tasks start right away and report there; content and fallback are each built
/// once and attached or detached between anchors as the boundary empties and refills,
/// so state inside survives the swaps, and both are destroyed when the owner the view
/// was built under is disposed. Toward an enclosing boundary the whole subtree counts
/// as one pending task. Holds [`NoChild`] until
/// [`set_child_builder`](Self::set_child_builder) supplies the content as a builder
/// closure, built under the owner the boundary is provided on.
///
/// ```
/// use skene_terminal::prelude::*;
///
/// let page = Suspense::new()
///   .set_fallback_builder(|| Text::new("Loading…"))
///   .set_child_builder(|| Text::new("Loaded"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Suspense<C = NoChild>(skene_foundation::Suspense<Active, C>);

impl Default for Suspense {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl SuspenseBase<Active> for Suspense {}

impl Suspense {
  /// Sets the content, built by `child` under the boundary.
  #[inline(always)]
  pub fn set_child_builder<V: View<Active>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Suspense<ChildBuilder<Active>> {
    Suspense(self.0.set_child_builder(child))
  }
}
