//! The stack layout.

use crate::{
  AnyKind,
  Children,
  HasChildren,
  Layout,
  LayoutBox,
  NodeId,
  Platform,
  Seal,
  StackChild,
  View,
};

/// Generates a [`Stack`]'s builder surface on a type holding one in `$field`.
///
/// The surface is the declared style, the padding and the alignment of the unpinned
/// children, on the platform `$platform`. The foundation's stack expands it over its
/// box, and a facade's newtype over the stack it wraps.
#[macro_export]
macro_rules! stack_surface {
  ($field:tt, $platform:ty) => {
    /// The declared style, for a facade's medium-specific setters to write into.
    #[inline(always)]
    pub const fn style_mut(&mut self) -> &mut $crate::DeclaredBoxStyle<$platform> {
      self.$field.style_mut()
    }

    $crate::properties! { style_mut, $platform;
      set_padding / derive_padding (padding) => padding: $crate::Inset<<$platform as $crate::Platform>::Scalar>,
        "the inset between the box and the children it arranges";
    }

    $crate::forward! { $field;
      /// Sets the alignment of the unpinned children within the stack, on both axes.
      fn set_alignment(alignment: impl Into<$crate::Value<$crate::Alignment, $crate::Condition<<$platform as $crate::Platform>::Scalar>>>);
      /// Derives the alignment of the unpinned children from the signals `alignment` reads.
      fn derive_alignment[R: Into<$crate::StaticLeaf<$crate::Alignment>>](alignment: impl Fn() -> R + 'static);
    }
  };
}

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

  crate::stack_surface! { inner, P }
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
