//! The wrapper giving a flex child its flexibility.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{
  Annotation,
  AnyView,
  Condition,
  Dimension,
  Driver,
  FlexChild,
  Flexibility,
  IntoChildOf,
  IntoView,
  ItemAlignment,
  Leaf,
  NoChild,
  Scene,
  Value,
  View,
};

/// A grow weight: fixed, or recomputed from the signals it reads.
enum Factor {
  Fixed(f32),
  Derived(Rc<dyn Fn() -> f32>),
}

/// Gives the child it wraps its flexibility in a [`Row`](crate::Row) or a
/// [`Column`](crate::Column).
///
/// Creates no node: the flexibility becomes the `flex` property of the child's own
/// style, and the alignment its `self_alignment`, so they compile, condition and
/// derive like any property. Accepted only where a flex container's children are;
/// holds [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct Flexible<D: Driver, C = NoChild> {
  initial: Dimension<D::Scalar>,
  grow: Factor,
  shrink: f32,
  alignment: Option<Value<ItemAlignment, Condition<D::Scalar>>>,
  child: C,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> Default for Flexible<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> Flexible<D> {
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
      grow: Factor::Fixed(grow_factor),
      shrink: shrink_factor,
      alignment: None,
      child: NoChild,
      driver: PhantomData,
    }
  }
}

impl<D: Driver, C> Flexible<D, C> {
  /// Sets the weight the child grows by when the container has room to spare.
  #[inline(always)]
  pub fn set_grow_factor(mut self, factor: f32) -> Self {
    self.grow = Factor::Fixed(factor);

    self
  }

  /// Derives the grow weight from the signals `factor` reads.
  #[inline(always)]
  pub fn derive_grow_factor(mut self, factor: impl Fn() -> f32 + 'static) -> Self {
    self.grow = Factor::Derived(Rc::new(factor));

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
  pub fn set_initial(mut self, initial: impl Into<Dimension<D::Scalar>>) -> Self {
    self.initial = initial.into();

    self
  }

  /// Sets the child's own cross-axis alignment, overriding the container's.
  #[inline(always)]
  pub fn set_item_alignment(
    mut self,
    alignment: impl Into<Value<ItemAlignment, Condition<D::Scalar>>>,
  ) -> Self {
    self.alignment = Some(alignment.into());

    self
  }

  /// Derives the child's own cross-axis alignment from the signals `alignment` reads.
  #[inline(always)]
  pub fn derive_item_alignment(mut self, alignment: impl Fn() -> ItemAlignment + 'static) -> Self {
    self.alignment = Some(Value::new(Leaf::Derived(Rc::new(alignment))));

    self
  }

  /// Sets the child.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<D>) -> Flexible<D, AnyView<D>> {
    Flexible {
      initial: self.initial,
      grow: self.grow,
      shrink: self.shrink,
      alignment: self.alignment,
      child: child.into_view(),
      driver: PhantomData,
    }
  }
}

impl<D: Driver> IntoChildOf<D, FlexChild> for Flexible<D, AnyView<D>> {
  fn into_child(self) -> AnyView<D> {
    AnyView::new(move || {
      let initial = self.initial;
      let shrink = self.shrink;
      let flex = match self.grow {
        Factor::Fixed(grow) => Value::from(Flexibility::new(initial, grow, shrink)),
        Factor::Derived(grow) => Value::new(Leaf::Derived(Rc::new(move || {
          Flexibility::new(initial, grow(), shrink)
        }))),
      };

      Scene::<D>::with(|scene| {
        scene.set_pending_annotation(Annotation {
          flex: Some(flex),
          self_alignment: self.alignment,
          ..Annotation::default()
        });
      });

      self.child.build()
    })
  }
}
