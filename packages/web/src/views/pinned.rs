//! The wrapper pinning a stack child to the stack's edges.

use skene_foundation::{AnyView, IntoChildOf, NoChild, StackChild, View};

use crate::ActivePlatform;

/// Pins the child it wraps against the edges of a [`Stack`](crate::Stack), out of flow.
///
/// Creates no node: the edges become the `pinned` property of the child's own style, so
/// they compile, condition and derive like any property. An edge left `Auto` puts the
/// child where the stack's alignment does on that axis; every edge is `Auto` until
/// [`set_edges`](Self::set_edges). Accepted only where a stack's children are; holds
/// [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct Pinned<C = NoChild>(skene_foundation::Pinned<ActivePlatform, C>);

impl<C> std::fmt::Debug for Pinned<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(formatter)
  }
}

impl Default for Pinned {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Pinned {
  /// A wrapper with every edge `Auto` and no child yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Pinned::new())
  }

  skene_foundation::pinned_surface! { 0, ActivePlatform }

  /// Sets the child.
  pub fn set_child(self, child: impl View<ActivePlatform>) -> Pinned<AnyView<ActivePlatform>> {
    Pinned(self.0.set_child(child))
  }
}

impl IntoChildOf<ActivePlatform, StackChild> for Pinned<AnyView<ActivePlatform>> {
  #[inline(always)]
  fn into_child(self) -> AnyView<ActivePlatform> {
    self.0.into_child()
  }
}
