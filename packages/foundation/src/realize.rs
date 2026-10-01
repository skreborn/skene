//! What every view's build shares.
//!
//! Reaching a node's sink, binding the render effect behind each derived slot, writing
//! a value that is fixed or derived, building a child under a provided context, and
//! registering a platform-made sink as a node.

use std::{marker::PhantomData, rc::Rc};

use skene_reactive::Effect;

use crate::{AccessProp, AnyView, BoxSlot, NodeId, NodeKind, Platform, RunSlot, Scene, View};

/// Binds a render effect that computes `slot`'s leaf and writes it into the box
/// `node`, now and whenever a signal the leaf reads changes.
pub fn bind_box_slot<P: Platform>(node: NodeId, slot: BoxSlot<P::Scalar, P::StyleExt>) {
  let BoxSlot { slot, compute, .. } = slot;

  Effect::render(move |_: Option<()>| {
    let value = compute();

    P::update_box(node, slot, &value);
  });
}

/// Binds a render effect that computes `slot`'s leaf and writes it into the run
/// `node`, now and whenever a signal the leaf reads changes.
pub fn bind_run_slot<P: Platform>(node: NodeId, slot: RunSlot<P::StyleExt>) {
  let RunSlot { slot, compute, .. } = slot;

  Effect::render(move |_: Option<()>| {
    let value = compute();

    P::update_run(node, slot, &value);
  });
}

/// Reports `access` as what the view that made `node` says to assistive technology,
/// beside whatever describes the node.
pub fn report_access<P: Platform>(node: NodeId, access: Vec<AccessProp>) {
  let access = Scene::<P>::with(|scene| scene.report_access(node, access));

  P::update_access(node, &access);
}

/// Reports `description` as what a view describing `node` says to assistive
/// technology, beside the node's own report.
pub fn describe<P: Platform>(node: NodeId, description: Vec<AccessProp>) {
  let access = Scene::<P>::with(|scene| scene.describe(node, description));

  P::update_access(node, &access);
}

/// A setting outside the style model: fixed, or recomputed from the signals it reads.
#[must_use]
pub enum Source<T> {
  /// A value known when the view is built.
  Fixed(T),
  /// A value recomputed whenever a signal it reads changes.
  Derived(Rc<dyn Fn() -> T>),
}

impl<T> std::fmt::Debug for Source<T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Source").finish_non_exhaustive()
  }
}

impl<T: Clone> Clone for Source<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    match self {
      Self::Fixed(value) => Self::Fixed(value.clone()),
      Self::Derived(compute) => Self::Derived(Rc::clone(compute)),
    }
  }
}

impl<T: 'static> Source<T> {
  /// A source recomputed by `compute`.
  pub fn derive(compute: impl Fn() -> T + 'static) -> Self {
    Self::Derived(Rc::new(compute))
  }

  /// The value as of now: the fixed one, cloned, or the one the derived closure
  /// computes, tracked.
  #[inline(always)]
  pub fn get(&self) -> T
  where
    T: Clone,
  {
    match self {
      Self::Fixed(value) => value.clone(),
      Self::Derived(compute) => compute(),
    }
  }

  /// The source as a read: the fixed value, cloned, or the derived closure.
  #[inline(always)]
  pub fn into_read(self) -> Rc<dyn Fn() -> T>
  where
    T: Clone,
  {
    match self {
      Self::Fixed(value) => Rc::new(move || value.clone()),
      Self::Derived(compute) => compute,
    }
  }

  /// Writes the value into `node` through `write`: once for a fixed value, and under a
  /// render effect for a derived one.
  pub fn bind(self, node: NodeId, write: impl Fn(NodeId, &T) + 'static) {
    match self {
      Self::Fixed(value) => {
        write(node, &value);
      }
      Self::Derived(compute) => {
        Effect::render(move |_: Option<()>| {
          let value = compute();

          write(node, &value);
        });
      }
    }
  }
}

/// The closure building a view's child under the owner the view provides context on.
#[must_use]
pub struct ChildBuilder<P: Platform>(Box<dyn FnOnce() -> AnyView<P>>);

impl<P: Platform> std::fmt::Debug for ChildBuilder<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ChildBuilder")
      .finish_non_exhaustive()
  }
}

impl<P: Platform> ChildBuilder<P> {
  /// A builder running `child` when the view builds.
  pub fn new<V: View<P>>(child: impl FnOnce() -> V + 'static) -> Self {
    Self(Box::new(move || child().into_any()))
  }

  /// Builds the child.
  #[inline(always)]
  pub fn build(self) -> NodeId {
    (self.0)().build()
  }
}

/// The entry a low-level view realizes through: a node the scene mints, realized by the
/// view against a platform's escape trait, an ordinary node from then on.
///
/// The node it returns is a [`View`], so it composes as a built-in does: as the child
/// of a [`Container`](crate::Container) that sizes and decorates it, under a theme,
/// inside a wrapper, with handlers registered on it through the [`Scene`]. A box the
/// view creates through [`Platform::create_box`] instead needs no registration beyond
/// the [`LayoutBox`](crate::LayoutBox) that made it.
#[derive(Debug)]
#[must_use]
pub struct LowLevel<P: Platform>(PhantomData<fn() -> P>);

impl<P: Platform> LowLevel<P> {
  /// Mints a detached node of `kind`, has `make` realize it against a platform's
  /// escape trait, and returns its id, recording the annotation a wrapper around the
  /// view gave it and applying it to the node as its layout; see
  /// [`Platform::apply_layout`].
  pub fn realize(kind: NodeKind, make: impl FnOnce(NodeId)) -> NodeId {
    let (node, annotation) = Scene::<P>::with(|scene| {
      let annotation = scene.take_pending_annotation();

      (scene.create(kind, annotation.clone()), annotation)
    });

    make(node);

    if let Some(annotation) = annotation {
      let (layout, slots) = annotation.into_style().split();

      P::apply_layout(node, &layout);

      for slot in slots {
        bind_box_slot::<P>(node, slot);
      }
    }

    node
  }
}
