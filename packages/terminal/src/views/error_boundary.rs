//! Fallback content when a subtree reports an error.

use skene_foundation::NoChild;

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

skene_foundation::impl_error_boundary!(ErrorBoundary, ActivePlatform);
