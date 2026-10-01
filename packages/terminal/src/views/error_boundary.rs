//! Fallback content when a subtree reports an error.

use skene_foundation::{AnyKind, ChildBuilder, Errors, NoChild, NodeId, View};

use crate::ActivePlatform;

/// Shows its content while no errors are held, and the fallback once one is reported.
///
/// The boundary provides an [`Errors`](skene_foundation::Errors) handle to its content through the
/// reactive context; the fallback receives the same handle, to present the errors and dismiss
/// them. The content is built once and attached or detached between anchors as errors
/// arrive and clear, so clearing restores it as it was; the fallback rebuilds whenever
/// the held errors change; both are destroyed when the owner the view was built under
/// is disposed. Holds [`NoChild`] until [`set_child_builder`](Self::set_child_builder)
/// supplies the content; it is a builder closure because it is built under the owner
/// the handle is provided on.
#[must_use]
pub struct ErrorBoundary<C = NoChild>(skene_foundation::ErrorBoundary<ActivePlatform, C>);

impl<C> std::fmt::Debug for ErrorBoundary<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(formatter)
  }
}

impl Default for ErrorBoundary {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl ErrorBoundary {
  /// A boundary with no fallback and no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self(skene_foundation::ErrorBoundary::new())
  }

  /// Sets the fallback shown while errors are held, built by `fallback` from the
  /// boundary's handle whenever the held errors change; nothing by default.
  #[inline(always)]
  pub fn set_fallback_builder<V: View<ActivePlatform>>(
    self,
    fallback: impl Fn(Errors) -> V + 'static,
  ) -> Self {
    Self(self.0.set_fallback_builder(fallback))
  }

  /// Sets the content, built by `child` under the boundary.
  #[inline(always)]
  pub fn set_child_builder<V: View<ActivePlatform>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> ErrorBoundary<ChildBuilder<ActivePlatform>> {
    ErrorBoundary(self.0.set_child_builder(child))
  }
}

impl<C> AnyKind for ErrorBoundary<C> {}

impl View<ActivePlatform> for ErrorBoundary<ChildBuilder<ActivePlatform>> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
