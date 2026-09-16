//! The wrapper pinning a stack child to the stack's edges.

use std::marker::PhantomData;

use skene_core::{
  Annotation,
  AnyView,
  Condition,
  Dimension,
  Driver,
  Inset,
  IntoChildOf,
  IntoView,
  Leaf,
  NoChild,
  Scene,
  StackChild,
  StaticLeaf,
  Value,
  View,
};

/// Pins the child it wraps against the edges of a [`Stack`](crate::Stack), out of
/// flow.
///
/// Creates no node: the edges become the `pinned` property of the child's own style,
/// so they compile, condition and derive like any property. An edge left `Auto` puts
/// the child where the stack's alignment does on that axis; every edge is `Auto` until
/// [`set_edges`](Self::set_edges). Accepted only where a stack's children are; holds
/// [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct Pinned<D: Driver, C = NoChild> {
  edges: Value<Inset<Dimension<D::Scalar>>, Condition<D::Scalar>>,
  child: C,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> Default for Pinned<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> Pinned<D> {
  /// A wrapper with every edge `Auto` and no child yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      edges: Value::from(Inset::all(Dimension::Auto)),
      child: NoChild,
      driver: PhantomData,
    }
  }
}

impl<D: Driver, C> Pinned<D, C> {
  /// Sets the edges the child is pinned to.
  #[inline(always)]
  pub fn set_edges(
    mut self,
    edges: impl Into<Value<Inset<Dimension<D::Scalar>>, Condition<D::Scalar>>>,
  ) -> Self {
    self.edges = edges.into();

    self
  }

  /// Derives the edges the child is pinned to from the signals `edges` reads: a value,
  /// a token or an expression.
  #[inline(always)]
  pub fn derive_edges<R: Into<StaticLeaf<Inset<Dimension<D::Scalar>>>>>(
    mut self,
    edges: impl Fn() -> R + 'static,
  ) -> Self {
    self.edges = Value::new(Leaf::derive(edges));

    self
  }

  /// Sets the child.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<D>) -> Pinned<D, AnyView<D>> {
    Pinned {
      edges: self.edges,
      child: child.into_view(),
      driver: PhantomData,
    }
  }
}

impl<D: Driver> IntoChildOf<D, StackChild> for Pinned<D, AnyView<D>> {
  fn into_child(self) -> AnyView<D> {
    AnyView::new(move || {
      Scene::<D>::with(|scene| {
        scene.set_pending_annotation(Annotation {
          pinned: Some(self.edges),
          ..Annotation::default()
        });
      });

      self.child.build()
    })
  }
}
