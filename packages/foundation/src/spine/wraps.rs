//! A view of a facade's own over the foundation's: a type holding one other as its
//! whole content, and taking on what that one is.

use crate::spine::{Children, HasChild, HasChildren, Realization, Seal};

/// A type holding one other as its whole content: a facade's view over the
/// foundation's.
///
/// What the type held is, the holder is: a box with one child, a container of children,
/// and what the builder traits reach through (its box style, its layout box, its run).
/// `#[derive(Wraps)]` implements it for a tuple struct of one field; `#[derive(View)]`
/// makes that struct the view it holds.
pub trait Wraps: Sized {
  /// The type held.
  type Inner;

  /// Holds `inner`.
  fn wrap(inner: Self::Inner) -> Self;

  /// The type held.
  fn inner(&self) -> &Self::Inner;

  /// The type held, to change in place.
  fn inner_mut(&mut self) -> &mut Self::Inner;

  /// Gives up the type held.
  fn into_inner(self) -> Self::Inner;
}

impl<P: Realization, T: Wraps<Inner: HasChild<P>>> HasChild<P> for T {
  type Kind = <T::Inner as HasChild<P>>::Kind;

  #[inline(always)]
  fn child_mut(&mut self, seal: Seal) -> &mut Children<P, Self::Kind> {
    self.inner_mut().child_mut(seal)
  }
}

impl<P: Realization, T: Wraps<Inner: HasChildren<P>>> HasChildren<P> for T {
  type Kind = <T::Inner as HasChildren<P>>::Kind;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<P, Self::Kind> {
    self.inner_mut().children_mut(seal)
  }
}
