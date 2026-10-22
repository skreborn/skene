//! The stack layout.

use crate::{
  Alignment,
  AnyKind,
  Children,
  Condition,
  DeclaredBoxStyle,
  HasBoxStyle,
  HasChildren,
  HasLayoutBox,
  Inset,
  Layout,
  LayoutBox,
  NodeId,
  Realization,
  Seal,
  StackChild,
  StaticLeaf,
  Value,
  View,
  Wraps,
};

/// Children overlapping in one cell, stacked in order with the last on top.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Pinned`](crate::Pinned)
/// wrapper as a child; a pinned child sits against the stack's edges instead of where
/// the alignment puts it.
#[derive(Debug)]
#[must_use]
pub struct Stack<P: Realization> {
  inner: LayoutBox<P, StackChild>,
}

impl<P: Realization> Default for Stack<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Stack<P> {
  /// An empty stack.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Stack),
    }
  }
}

impl<P: Realization> HasChildren<P> for Stack<P> {
  type Kind = StackChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<P, StackChild> {
    self.inner.children_mut(seal)
  }
}

impl<P: Realization> AnyKind for Stack<P> {}

impl<P: Realization> View<P> for Stack<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}

impl<P: Realization> HasBoxStyle<P> for Stack<P> {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.inner.style_mut()
  }
}

impl<P: Realization> HasLayoutBox<P> for Stack<P> {
  type Kind = StackChild;

  #[inline(always)]
  fn map_layout_box(
    self,
    map: impl FnOnce(LayoutBox<P, StackChild>) -> LayoutBox<P, StackChild>,
  ) -> Self {
    Self {
      inner: map(self.inner),
    }
  }
}

impl<P: Realization> StackBase<P> for Stack<P> {}

/// What a [`Stack`] carries, with its constructor on a facade's newtype.
///
/// The inset around its children and the alignment of the unpinned ones.
pub trait StackBase<P: Realization>: HasBoxStyle<P> + HasLayoutBox<P, Kind = StackChild> {
  /// An empty stack.
  #[must_use]
  fn new() -> Self
  where
    Self: Wraps<Inner = Stack<P>>,
  {
    Self::wrap(Stack::new())
  }

  crate::trait_properties! { box_style_mut, Condition<P::Scalar>;
    set_padding / derive_padding (padding) => padding: Inset<P::Scalar>,
      "the inset between the box and the children it arranges";
  }

  /// Sets the alignment of the unpinned children within the stack, on both axes.
  #[must_use]
  fn set_item_alignment(
    self,
    alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.map_layout_box(|layout| layout.set_alignment(alignment))
  }

  /// Derives the alignment of the unpinned children from the signals `alignment` reads.
  #[must_use]
  fn derive_item_alignment<R: Into<StaticLeaf<Alignment>>>(
    self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.map_layout_box(|layout| layout.derive_alignment(alignment))
  }
}
