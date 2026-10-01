//! The stack layout.

use crate::ActivePlatform;

/// Children overlapping in one cell, stacked in order with the last on top.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Pinned`](crate::Pinned)
/// wrapper as a child; a pinned child sits against the stack's edges instead of where
/// the alignment puts it.
#[must_use]
pub struct Stack(skene_foundation::Stack<ActivePlatform>);

skene_foundation::impl_stack!(Stack, ActivePlatform);
