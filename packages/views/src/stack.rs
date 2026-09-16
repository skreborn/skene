//! The stack layout.

use skene_core::{
  Alignment,
  AnyKind,
  Children,
  Condition,
  DeclaredBoxStyle,
  Driver,
  HasChildren,
  Inset,
  Layout,
  NodeId,
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
#[must_use]
pub struct Stack<D: Driver> {
  inner: LayoutBox<D, StackChild>,
}

impl<D: Driver> Default for Stack<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> Stack<D> {
  /// An empty stack.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Stack),
    }
  }

  #[inline(always)]
  const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<D> {
    self.inner.style_mut()
  }

  properties! { style_mut;
    set_padding / derive_padding (padding) => padding: Inset<D::Scalar>,
      "the inset between the box and the children it arranges";
  }

  /// Sets the alignment of the unpinned children within the stack, on both axes.
  #[inline(always)]
  pub fn set_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<D::Scalar>>>,
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

impl<D: Driver> HasChildren<D> for Stack<D> {
  type Kind = StackChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<D, StackChild> {
    self.inner.children_mut(seal)
  }
}

impl<D: Driver> AnyKind for Stack<D> {}

impl<D: Driver> View<D> for Stack<D> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}
