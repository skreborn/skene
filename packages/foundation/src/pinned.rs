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
  NoChild,
  Platform,
  Scene,
  StackChild,
  StaticLeaf,
  Value,
  View,
};

/// Generates a [`Pinned`]'s setters on a facade's newtype holding one in `$field`, each
/// forwarding to the wrapper's own, on the platform `$platform`.
#[macro_export]
macro_rules! pinned_surface {
  ($field:tt, $platform:ty) => {
    $crate::forward! { $field;
      /// Sets the edges the child is pinned to.
      fn set_edges(
        edges: impl Into<
          $crate::Value<
            $crate::Inset<$crate::Dimension<<$platform as $crate::Platform>::Scalar>>,
            $crate::Condition<<$platform as $crate::Platform>::Scalar>,
          >,
        >,
      );
      /// Derives the edges the child is pinned to from the signals `edges` reads: a
      /// value, a token or an expression.
      fn derive_edges[R: Into<$crate::StaticLeaf<$crate::Inset<$crate::Dimension<<$platform as $crate::Platform>::Scalar>>>>](
        edges: impl Fn() -> R + 'static,
      );
    }
  };
}

/// Pins the child it wraps against the edges of a [`Stack`](crate::Stack), out of
/// flow.
///
/// Creates no node: the edges become the `pinned` property of the child's own style,
/// so they compile, condition and derive like any property. An edge left `Auto` puts
/// the child where the stack's alignment does on that axis; every edge is `Auto` until
/// [`set_edges`](Self::set_edges). Accepted only where a stack's children are; holds
/// [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct Pinned<P: Platform, C = NoChild> {
  edges: Value<Inset<Dimension<P::Scalar>>, Condition<P::Scalar>>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, C> std::fmt::Debug for Pinned<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Pinned").finish_non_exhaustive()
  }
}

impl<P: Platform> Default for Pinned<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Pinned<P> {
  /// A wrapper with every edge `Auto` and no child yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      edges: Value::from(Inset::all(Dimension::Auto)),
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> Pinned<P> {
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
  pub fn set_child(self, child: impl View<P>) -> Pinned<P, AnyView<P>> {
    Pinned {
      edges: self.edges,
      child: child.into_any(),
      platform: PhantomData,
    }
  }
}

impl<P: Platform> IntoChildOf<P, StackChild> for Pinned<P, AnyView<P>> {
  fn into_child(self) -> AnyView<P> {
    AnyView::new(move || {
      let annotation = Annotation {
        pinned: Some(self.edges),
        ..Annotation::default()
      };

      Scene::<P>::annotate(annotation, || self.child.build())
    })
  }
}
