//! Fallback content while a subtree's async work is pending.

use skene_foundation::{SuspenseBase, View, Wraps};

use crate::Active;

/// Shows its content once that subtree's async work settles, and the fallback until
/// then.
///
/// The content builds at once under a fresh [`Boundary`](skene_foundation::Boundary),
/// so its tasks start right away and report there; content and fallback are each built
/// once and attached or detached between anchors as the boundary empties and refills,
/// so state inside survives the swaps, and both are destroyed when the owner the view
/// was built under is disposed. Toward an enclosing boundary the whole subtree counts
/// as one pending task. The content is a builder closure
/// [`set_child_builder`](Self::set_child_builder) supplies, built under the owner the
/// boundary is provided on; without it the view builds nothing, its fallback included.
///
/// ```
/// use skene_web::prelude::*;
///
/// let page = Suspense::new()
///   .set_fallback_builder(|| Text::new("Loading…"))
///   .set_child_builder(|| Text::new("Loaded"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Suspense(skene_foundation::Suspense<Active>);

impl Default for Suspense {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl SuspenseBase<Active> for Suspense {}
