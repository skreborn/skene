//! Fallback content when a subtree reports an error.

use skene_foundation::{ErrorBoundaryBase, View, Wraps};

use crate::Active;

/// Shows its content while no errors are held, and the fallback once one is reported.
///
/// The boundary provides an [`Errors`](skene_foundation::Errors) handle to its content
/// through the reactive context; the fallback receives the same handle, to present the
/// errors and dismiss them. The content is built once and attached or detached between
/// anchors as errors arrive and clear, so clearing restores it as it was; the fallback
/// rebuilds whenever the held errors change; both are destroyed when the owner the view
/// was built under is disposed. The content is a builder closure
/// [`set_child_builder`](Self::set_child_builder) supplies, built under the owner the
/// handle is provided on; without it the view builds nothing, its fallback included.
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
pub struct ErrorBoundary(skene_foundation::ErrorBoundary<Active>);

impl Default for ErrorBoundary {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl ErrorBoundaryBase<Active> for ErrorBoundary {}
