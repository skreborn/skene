//! The stack layout.

use skene_core::{
  Alignment,
  AnyKind,
  Children,
  Condition,
  DeclaredBoxStyle,
  HasChildren,
  Inset,
  Layout,
  NodeId,
  Platform,
  Seal,
  StackChild,
  StaticLeaf,
  Value,
  View,
};

use crate::{LayoutBox, properties};

/// Children overlapping in one cell, stacked in order with the last on top.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Pinned`](crate::Pinned)
/// wrapper as a child; a pinned child sits against the stack's edges instead of where
/// the alignment puts it.
#[derive(Debug)]
#[must_use]
pub struct Stack<P: Platform> {
  inner: LayoutBox<P, StackChild>,
}

impl<P: Platform> Default for Stack<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Stack<P> {
  /// An empty stack.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Stack),
    }
  }

  #[inline(always)]
  const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.inner.style_mut()
  }

  properties! { style_mut;
    set_padding / derive_padding (padding) => padding: Inset<P::Scalar>,
      "the inset between the box and the children it arranges";
  }

  /// Sets the alignment of the unpinned children within the stack, on both axes.
  #[inline(always)]
  pub fn set_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.inner = self.inner.set_alignment(alignment);

    self
  }

  /// Derives the alignment of the unpinned children from the signals `alignment` reads.
  #[inline(always)]
  pub fn derive_alignment<R: Into<StaticLeaf<Alignment>>>(
    mut self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.inner = self.inner.derive_alignment(alignment);

    self
  }
}

impl<P: Platform> HasChildren<P> for Stack<P> {
  type Kind = StackChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<P, StackChild> {
    self.inner.children_mut(seal)
  }
}

impl<P: Platform> AnyKind for Stack<P> {}

impl<P: Platform> View<P> for Stack<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}
