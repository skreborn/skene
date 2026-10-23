//! The wrapper pinning a stack child to the stack's edges.

use std::marker::PhantomData;

use crate::{
  Annotation,
  AnyView,
  Condition,
  Dimension,
  Inset,
  IntoChildOf,
  Leaf,
  Realization,
  Scene,
  StackChild,
  StaticLeaf,
  Value,
  View,
  Wraps,
  empty,
};

/// Pins the child it wraps against the edges of a [`Stack`](crate::Stack), out of
/// flow.
///
/// Creates no node: the edges become the `pinned` property of the child's own style,
/// so they compile, condition and derive like any property. An edge left `Auto` puts
/// the child where the stack's alignment does on that axis; every edge is `Auto` until
/// [`set_edges`](Self::set_edges). Accepted only where a stack's children are; a
/// wrapper without its child renders nothing and takes no room.
#[must_use]
pub struct Pinned<P: Realization> {
  edges: Value<Inset<Dimension<P::Scalar>>, Condition<P::Scalar>>,
  child: Option<AnyView<P>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization> std::fmt::Debug for Pinned<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Pinned").finish_non_exhaustive()
  }
}

impl<P: Realization> Default for Pinned<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Pinned<P> {
  /// A wrapper with every edge `Auto` and no child yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      edges: Value::from(Inset::all(Dimension::Auto)),
      child: None,
      platform: PhantomData,
    }
  }
}

impl<P: Realization> Pinned<P> {
  /// Sets the edges the child is pinned to.
  pub fn set_edges(
    mut self,
    edges: impl Into<Value<Inset<Dimension<P::Scalar>>, Condition<P::Scalar>>>,
  ) -> Self {
    self.edges = edges.into();

    self
  }

  /// Derives the edges the child is pinned to from the signals `edges` reads: a value,
  /// a token or an expression.
  pub fn derive_edges<R: Into<StaticLeaf<Inset<Dimension<P::Scalar>>>>>(
    mut self,
    edges: impl Fn() -> R + 'static,
  ) -> Self {
    self.edges = Value::new(Leaf::derive(edges));

    self
  }

  /// Sets the child.
  pub fn set_child(mut self, child: impl View<P>) -> Self {
    self.child = Some(child.into_any());

    self
  }
}

impl<P: Realization> IntoChildOf<P, StackChild> for Pinned<P> {
  fn into_child(self) -> AnyView<P> {
    let Some(child) = self.child
    else {
      return empty::<P>().into_any();
    };

    AnyView::new(move || {
      let annotation = Annotation {
        pinned: Some(self.edges),
        ..Annotation::default()
      };

      Scene::<P>::annotate(annotation, || child.build())
    })
  }
}

/// What a facade's [`Pinned`] carries: its constructor and its setters.
pub trait PinnedBase<P: Realization>: Wraps<Inner = Pinned<P>> {
  /// A wrapper with every edge `Auto` and no child yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(Pinned::new())
  }

  /// Sets the edges the child is pinned to.
  #[must_use]
  fn set_edges(
    self,
    edges: impl Into<Value<Inset<Dimension<P::Scalar>>, Condition<P::Scalar>>>,
  ) -> Self {
    Self::wrap(self.into_inner().set_edges(edges))
  }

  /// Derives the edges the child is pinned to from the signals `edges` reads: a
  /// value, a token or an expression.
  #[must_use]
  fn derive_edges<R: Into<StaticLeaf<Inset<Dimension<P::Scalar>>>>>(
    self,
    edges: impl Fn() -> R + 'static,
  ) -> Self {
    Self::wrap(self.into_inner().derive_edges(edges))
  }

  /// Sets the child.
  #[must_use]
  fn set_child(self, child: impl View<P>) -> Self {
    Self::wrap(self.into_inner().set_child(child))
  }
}
