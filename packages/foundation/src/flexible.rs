//! The wrapper giving a flex child its flexibility.

use std::marker::PhantomData;

use crate::{
  Annotation,
  AnyView,
  Condition,
  FlexChild,
  Flexibility,
  IntoChildOf,
  ItemAlignment,
  Leaf,
  NoChild,
  Platform,
  Scene,
  StaticLeaf,
  Value,
  View,
  Wraps,
};

/// Gives the child it wraps its flexibility in a [`Row`](crate::Row) or a
/// [`Column`](crate::Column).
///
/// Creates no node: the flexibility becomes the `flex` property of the child's own
/// style, and the alignment its `self_alignment`, so they compile, condition and
/// derive like any property. Accepted only where a flex container's children are;
/// holds [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct Flexible<P: Platform, C = NoChild> {
  flexibility: Value<Flexibility<P::Scalar>, Condition<P::Scalar>>,
  alignment: Option<Value<ItemAlignment, Condition<P::Scalar>>>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, C> std::fmt::Debug for Flexible<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Flexible").finish_non_exhaustive()
  }
}

impl<P: Platform> Default for Flexible<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Flexible<P> {
  /// A wrapper with the default flexibility and no child yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      flexibility: Value::from(Flexibility::default()),
      alignment: None,
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> Flexible<P> {
  /// Sets the child's flexibility: the size it starts from and its weights for taking
  /// free space and giving up overflow.
  pub fn set_flexibility(
    mut self,
    flexibility: impl Into<Value<Flexibility<P::Scalar>, Condition<P::Scalar>>>,
  ) -> Self {
    self.flexibility = flexibility.into();

    self
  }

  /// Derives the child's flexibility from the signals `flexibility` reads.
  pub fn derive_flexibility<R: Into<StaticLeaf<Flexibility<P::Scalar>>>>(
    mut self,
    flexibility: impl Fn() -> R + 'static,
  ) -> Self {
    self.flexibility = Value::new(Leaf::derive(flexibility));

    self
  }

  /// Sets the child's own cross-axis alignment, overriding the container's.
  pub fn set_alignment(
    mut self,
    alignment: impl Into<Value<ItemAlignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.alignment = Some(alignment.into());

    self
  }

  /// Derives the child's own cross-axis alignment from the signals `alignment` reads:
  /// a value, a token or an expression.
  pub fn derive_alignment<R: Into<StaticLeaf<ItemAlignment>>>(
    mut self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.alignment = Some(Value::new(Leaf::derive(alignment)));

    self
  }

  /// Sets the child.
  pub fn set_child(self, child: impl View<P>) -> Flexible<P, AnyView<P>> {
    Flexible {
      flexibility: self.flexibility,
      alignment: self.alignment,
      child: child.into_any(),
      platform: PhantomData,
    }
  }
}

impl<P: Platform> IntoChildOf<P, FlexChild> for Flexible<P, AnyView<P>> {
  fn into_child(self) -> AnyView<P> {
    AnyView::new(move || {
      let annotation = Annotation {
        flex: Some(self.flexibility),
        self_alignment: self.alignment,
        ..Annotation::default()
      };

      Scene::<P>::annotate(annotation, || self.child.build())
    })
  }
}

/// What a facade's [`Flexible`] carries: its constructor and its setters.
pub trait FlexibleBase<P: Platform>: Wraps<Inner = Flexible<P>> {
  /// A wrapper with the default flexibility and no child yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(Flexible::new())
  }

  /// Sets the child's flexibility: the size it starts from and its weights for taking
  /// free space and giving up overflow.
  #[must_use]
  fn set_flexibility(
    self,
    flexibility: impl Into<Value<Flexibility<P::Scalar>, Condition<P::Scalar>>>,
  ) -> Self {
    Self::wrap(self.into_inner().set_flexibility(flexibility))
  }

  /// Derives the child's flexibility from the signals `flexibility` reads.
  #[must_use]
  fn derive_flexibility<R: Into<StaticLeaf<Flexibility<P::Scalar>>>>(
    self,
    flexibility: impl Fn() -> R + 'static,
  ) -> Self {
    Self::wrap(self.into_inner().derive_flexibility(flexibility))
  }

  /// Sets the child's own cross-axis alignment, overriding the container's.
  #[must_use]
  fn set_alignment(self, alignment: impl Into<Value<ItemAlignment, Condition<P::Scalar>>>) -> Self {
    Self::wrap(self.into_inner().set_alignment(alignment))
  }

  /// Derives the child's own cross-axis alignment from the signals `alignment` reads:
  /// a value, a token or an expression.
  #[must_use]
  fn derive_alignment<R: Into<StaticLeaf<ItemAlignment>>>(
    self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    Self::wrap(self.into_inner().derive_alignment(alignment))
  }
}
