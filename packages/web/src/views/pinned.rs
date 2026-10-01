//! The wrapper pinning a stack child to the stack's edges.

use skene_foundation::NoChild;

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

skene_foundation::impl_pinned!(Pinned, ActivePlatform);
