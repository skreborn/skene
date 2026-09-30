//! The wrapper giving a flex child its flexibility.

use std::marker::PhantomData;

use skene_core::{
  Annotation,
  AnyView,
  Condition,
  Dimension,
  FlexChild,
  Flexibility,
  IntoChildOf,
  IntoView,
  ItemAlignment,
  Leaf,
  NoChild,
  Platform,
  Scene,
  StaticLeaf,
  Value,
  View,
};

use crate::realize::Source;

/// Gives the child it wraps its flexibility in a [`Row`](crate::Row) or a
/// [`Column`](crate::Column).
///
/// Creates no node: the flexibility becomes the `flex` property of the child's own
/// style, and the alignment its `self_alignment`, so they compile, condition and
/// derive like any property. Accepted only where a flex container's children are;
/// holds [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct Flexible<P: Platform, C = NoChild> {
  initial: Dimension<P::Scalar>,
  grow: Source<f32>,
  shrink: f32,
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
    let Flexibility {
      initial,
      grow_factor,
      shrink_factor,
    } = Flexibility::default();

    Self {
      initial,
      grow: Source::Fixed(grow_factor),
      shrink: shrink_factor,
      alignment: None,
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> Flexible<P> {
  /// Sets the weight the child grows by when the container has room to spare.
  #[inline(always)]
  pub fn set_grow_factor(mut self, factor: f32) -> Self {
    self.grow = Source::Fixed(factor);

    self
  }

  /// Derives the grow weight from the signals `factor` reads.
  #[inline(always)]
  pub fn derive_grow_factor(mut self, factor: impl Fn() -> f32 + 'static) -> Self {
    self.grow = Source::derive(factor);

    self
  }

  /// Sets the weight the child shrinks by when the container runs short.
  #[inline(always)]
  pub const fn set_shrink_factor(mut self, factor: f32) -> Self {
    self.shrink = factor;

    self
  }

  /// Sets the child's size before growing and shrinking.
  #[inline(always)]
  pub fn set_initial(mut self, initial: impl Into<Dimension<P::Scalar>>) -> Self {
    self.initial = initial.into();

    self
  }

  /// Sets the child's own cross-axis alignment, overriding the container's.
  #[inline(always)]
  pub fn set_item_alignment(
    mut self,
    alignment: impl Into<Value<ItemAlignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.alignment = Some(alignment.into());

    self
  }

  /// Derives the child's own cross-axis alignment from the signals `alignment` reads:
  /// a value, a token or an expression.
  #[inline(always)]
  pub fn derive_item_alignment<R: Into<StaticLeaf<ItemAlignment>>>(
    mut self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.alignment = Some(Value::new(Leaf::derive(alignment)));

    self
  }

  /// Sets the child.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<P>) -> Flexible<P, AnyView<P>> {
    Flexible {
      initial: self.initial,
      grow: self.grow,
      shrink: self.shrink,
      alignment: self.alignment,
      child: child.into_view(),
      platform: PhantomData,
    }
  }
}

impl<P: Platform> IntoChildOf<P, FlexChild> for Flexible<P, AnyView<P>> {
  fn into_child(self) -> AnyView<P> {
    AnyView::new(move || {
      let initial = self.initial;
      let shrink = self.shrink;
      let flex = match self.grow {
        Source::Fixed(grow) => Value::from(Flexibility::new(initial, grow, shrink)),
        Source::Derived(grow) => Value::new(Leaf::derive(move || {
          Flexibility::new(initial, grow(), shrink)
        })),
      };

      let annotation = Annotation {
        flex: Some(flex),
        self_alignment: self.alignment,
        ..Annotation::default()
      };

      Scene::<P>::annotate(annotation, || self.child.build())
    })
  }
}
