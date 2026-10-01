//! Fallback content when a subtree reports an error.

use skene_foundation::{ChildBuilder, ErrorBoundaryBase, NoChild, View, Wraps};

use crate::ActivePlatform;

/// Shows its content while no errors are held, and the fallback once one is reported.
///
/// The boundary provides an [`Errors`](skene_foundation::Errors) handle to its content
/// through the reactive context; the fallback receives the same handle, to present the
/// errors and dismiss them. The content is built once and attached or detached between
/// anchors as errors arrive and clear, so clearing restores it as it was; the fallback
/// rebuilds whenever the held errors change; both are destroyed when the owner the view
/// was built under is disposed. Holds [`NoChild`] until
/// [`set_child_builder`](Self::set_child_builder) supplies the content as a builder
/// closure, built under the owner the handle is provided on.
///
/// ```
/// use skene_web::prelude::*;
///
/// let guarded = ErrorBoundary::new()
///   .set_fallback_builder(|_| Text::new("Something failed"))
///   .set_child_builder(|| Text::new("Fine"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct ErrorBoundary<C = NoChild>(skene_foundation::ErrorBoundary<ActivePlatform, C>);

impl Default for ErrorBoundary {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl ErrorBoundaryBase<ActivePlatform> for ErrorBoundary {}

impl ErrorBoundary {
  /// Sets the content, built by `child` under the boundary.
  #[inline(always)]
  pub fn set_child_builder<V: View<ActivePlatform>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> ErrorBoundary<ChildBuilder<ActivePlatform>> {
    ErrorBoundary(self.0.set_child_builder(child))
  }
}
