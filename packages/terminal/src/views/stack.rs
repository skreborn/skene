//! The stack layout.

use skene_foundation::{AnyKind, Children, HasChildren, NodeId, Seal, StackChild, View};

use crate::ActivePlatform;

/// Children overlapping in one cell, stacked in order with the last on top.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Pinned`](crate::Pinned)
/// wrapper as a child; a pinned child sits against the stack's edges instead of where
/// the alignment puts it.
#[derive(Debug, Default)]
#[must_use]
pub struct Stack(skene_foundation::Stack<ActivePlatform>);

impl Stack {
  /// An empty stack.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Stack::new())
  }

  skene_foundation::stack_surface! { 0, ActivePlatform }
}

impl HasChildren<ActivePlatform> for Stack {
  type Kind = StackChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<ActivePlatform, StackChild> {
    self.0.children_mut(seal)
  }
}

impl AnyKind for Stack {}

impl View<ActivePlatform> for Stack {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
