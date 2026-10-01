//! A child shown or hidden by a condition.

use skene_foundation::{AnyView, NoChild, View, VisibilityBase, Wraps};

use crate::ActivePlatform;

/// Shows or hides one child, preserving its state.
///
/// The child is built the first time it shows, then attached or detached between a
/// pair of anchors when the condition changes, and left where it stands while it
/// holds. While hidden its effects pause, catching up once it shows again.
///
/// Holds [`NoChild`] until `set_child` supplies the child; only then is it a view.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Visibility<C = NoChild>(skene_foundation::Visibility<ActivePlatform, C>);

impl Default for Visibility {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl VisibilityBase<ActivePlatform> for Visibility {}

impl Visibility {
  /// Sets the child.
  #[inline(always)]
  pub fn set_child(self, child: impl View<ActivePlatform>) -> Visibility<AnyView<ActivePlatform>> {
    Visibility(self.0.set_child(child))
  }
}
