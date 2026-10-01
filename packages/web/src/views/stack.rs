//! The stack layout.

use skene_foundation::{StackBase, View, Wraps};

use crate::ActivePlatform;

/// Children overlapping in one cell, stacked in order with the last on top.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Pinned`](crate::Pinned)
/// wrapper as a child; a pinned child sits against the stack's edges instead of where
/// the alignment puts it.
///
/// ```
/// use skene_web::prelude::*;
///
/// let badge = Stack::new()
///   .set_item_alignment(Alignment::CENTER)
///   .set_children((Text::new("Inbox"), Text::new("3")));
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct Stack(skene_foundation::Stack<ActivePlatform>);

impl StackBase<ActivePlatform> for Stack {}
