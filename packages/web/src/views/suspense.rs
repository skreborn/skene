//! Fallback content while a subtree's async work is pending.

use skene_foundation::NoChild;

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
#[must_use]
pub struct Suspense<C = NoChild>(skene_foundation::Suspense<ActivePlatform, C>);

skene_foundation::impl_suspense!(Suspense, ActivePlatform);
