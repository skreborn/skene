//! The wrapper giving a flex child its flexibility.

use skene_foundation::{AnyView, FlexChild, IntoChildOf, NoChild, View};

use crate::ActivePlatform;

/// Gives the child it wraps its flexibility in a [`Row`](crate::Row) or a
/// [`Column`](crate::Column).
///
/// Creates no node: the flexibility becomes the `flex` property of the child's own
/// style, and the alignment its `self_alignment`, so they compile, condition and derive
/// like any property. Accepted only where a flex container's children are; holds
/// [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct Flexible<C = NoChild>(skene_foundation::Flexible<ActivePlatform, C>);

impl<C> std::fmt::Debug for Flexible<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(formatter)
  }
}

impl Default for Flexible {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Flexible {
  /// A wrapper with the default flexibility and no child yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Flexible::new())
  }

  skene_foundation::flexible_surface! { 0, ActivePlatform }

  /// Sets the child.
  pub fn set_child(self, child: impl View<ActivePlatform>) -> Flexible<AnyView<ActivePlatform>> {
    Flexible(self.0.set_child(child))
  }
}

impl IntoChildOf<ActivePlatform, FlexChild> for Flexible<AnyView<ActivePlatform>> {
  #[inline(always)]
  fn into_child(self) -> AnyView<ActivePlatform> {
    self.0.into_child()
  }
}
