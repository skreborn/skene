//! The wrapper giving a flex child its flexibility.

use skene_foundation::NoChild;

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

skene_foundation::impl_flexible!(Flexible, ActivePlatform);
