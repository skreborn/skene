//! A child shown or hidden by a condition.

use skene_foundation::{AnyKind, AnyView, NoChild, NodeId, View};

use crate::ActivePlatform;

/// Shows or hides one child, preserving its state.
///
/// The child is built the first time it shows, then attached or detached between a
/// pair of anchors when the condition changes, and left where it stands while it
/// holds. While hidden its effects pause, catching up once it shows again.
///
/// Holds [`NoChild`] until `set_child` supplies the child; only then is it a view.
#[must_use]
pub struct Show<C = NoChild>(skene_foundation::Show<ActivePlatform, C>);

impl<C> std::fmt::Debug for Show<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(formatter)
  }
}

impl Default for Show {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Show {
  /// A `Show` with no child yet, shown until told otherwise.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Show::new())
  }

  /// Sets whether the child is shown.
  #[inline(always)]
  pub fn set_shown(self, shown: bool) -> Self {
    Self(self.0.set_shown(shown))
  }

  /// Shows the child while `shown` returns `true`, re-evaluated whenever a signal it
  /// reads changes.
  #[inline(always)]
  pub fn derive_shown(self, shown: impl Fn() -> bool + 'static) -> Self {
    Self(self.0.derive_shown(shown))
  }

  /// Sets the child.
  #[inline(always)]
  pub fn set_child(self, child: impl View<ActivePlatform>) -> Show<AnyView<ActivePlatform>> {
    Show(self.0.set_child(child))
  }
}

impl<C> AnyKind for Show<C> {}

impl View<ActivePlatform> for Show<AnyView<ActivePlatform>> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
