//! What every view's build shares: reaching a node's sink, binding the render effect
//! behind each derived slot, writing a value that is fixed or derived, building a
//! child under a provided context, and registering a driver-made sink as a node.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{
  AccessProp,
  AnyView,
  BoxSlot,
  Driver,
  IntoView,
  NodeId,
  NodeKind,
  RunSlot,
  Scene,
  View,
};
use skene_reactive::Effect;

/// Runs `operation` on a copy of the sink of `node`, if the node is live.
///
/// The scene is released before `operation` runs, so a driver call inside it may
/// reach the scene again.
#[inline(always)]
pub fn with_sink<D: Driver, R>(node: NodeId, operation: impl FnOnce(&D::Sink) -> R) -> Option<R> {
  let sink = Scene::<D>::with(|scene| scene.sink(node))?;

  Some(operation(&sink))
}

/// Binds a render effect that computes `slot`'s leaf and writes it into the box
/// `node`, now and whenever a signal the leaf reads changes.
pub fn bind_box_slot<D: Driver>(node: NodeId, slot: BoxSlot<D::Scalar, D::StyleExt>) {
  let BoxSlot { slot, compute, .. } = slot;

  Effect::render(move |_: Option<()>| {
    let value = compute();

    with_sink::<D, _>(node, |sink| D::default().update_box(sink, slot, &value));
  });
}

/// Binds a render effect that computes `slot`'s leaf and writes it into the run
/// `node`, now and whenever a signal the leaf reads changes.
pub fn bind_run_slot<D: Driver>(node: NodeId, slot: RunSlot<D::StyleExt>) {
  let RunSlot { slot, compute, .. } = slot;

  Effect::render(move |_: Option<()>| {
    let value = compute();

    with_sink::<D, _>(node, |sink| D::default().update_run(sink, slot, &value));
  });
}

/// Reports `access` as what the view that made `node` says to assistive technology,
/// beside whatever describes the node.
pub fn report_access<D: Driver>(node: NodeId, access: Vec<AccessProp>) {
  let access = Scene::<D>::with(|scene| scene.report_access(node, access));

  with_sink::<D, _>(node, |sink| D::default().update_access(sink, &access));
}

/// Reports `description` as what a view describing `node` says to assistive
/// technology, beside the node's own report.
pub fn describe<D: Driver>(node: NodeId, description: Vec<AccessProp>) {
  let access = Scene::<D>::with(|scene| scene.describe(node, description));

  with_sink::<D, _>(node, |sink| D::default().update_access(sink, &access));
}

/// A setting outside the style model: fixed, or recomputed from the signals it reads.
#[must_use]
pub enum Source<T> {
  /// A value known when the view is built.
  Fixed(T),
  /// A value recomputed whenever a signal it reads changes.
  Derived(Rc<dyn Fn() -> T>),
}

impl<T: 'static> Source<T> {
  /// A source recomputed by `compute`.
  #[inline(always)]
  pub fn derive(compute: impl Fn() -> T + 'static) -> Self {
    Self::Derived(Rc::new(compute))
  }

  /// Writes the value into `node` through `write`: once for a fixed value, and under a
  /// render effect for a derived one.
  pub fn bind<D: Driver>(self, node: NodeId, write: impl Fn(&D::Sink, &T) + 'static) {
    match self {
      Self::Fixed(value) => {
        with_sink::<D, _>(node, |sink| write(sink, &value));
      }
      Self::Derived(compute) => {
        Effect::render(move |_: Option<()>| {
          let value = compute();

          with_sink::<D, _>(node, |sink| write(sink, &value));
        });
      }
    }
  }
}

/// The closure building a view's child under the owner the view provides context on.
#[must_use]
pub struct ChildBuilder<D: Driver>(Box<dyn FnOnce() -> AnyView<D>>);

impl<D: Driver> ChildBuilder<D> {
  /// A builder running `child` when the view builds.
  #[inline(always)]
  pub fn new<V: IntoView<D>>(child: impl FnOnce() -> V + 'static) -> Self {
    Self(Box::new(move || child().into_view()))
  }

  /// Builds the child.
  #[inline(always)]
  pub fn build(self) -> NodeId {
    (self.0)().build()
  }
}

/// The entry a low-level view realizes through: a sink the view made against a
/// driver's escape trait, registered in the scene as an ordinary node.
///
/// The node it returns is a [`View`], so it composes as a built-in does: as the child
/// of a [`Container`](crate::Container) that sizes and decorates it, under a theme,
/// inside a wrapper, with handlers registered on it through the [`Scene`]. A box the
/// view creates through [`Driver::create_box`] instead needs no registration beyond
/// the [`LayoutBox`](crate::LayoutBox) that made it.
#[must_use]
pub struct LowLevel<D: Driver>(PhantomData<fn() -> D>);

impl<D: Driver> LowLevel<D> {
  /// Registers `sink` as a detached node of `kind` and returns its id.
  #[inline(always)]
  pub fn realize(sink: D::Sink, kind: NodeKind) -> NodeId {
    Scene::<D>::with(|scene| scene.create(kind, sink))
  }
}
