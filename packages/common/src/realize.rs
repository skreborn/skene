//! What every view's build shares: reaching a node's sink, binding the render effect
//! behind each derived slot, writing a value that is fixed or derived, building a
//! child under a provided context, and registering a platform-made sink as a node.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{
  AccessProp,
  AnyView,
  BoxSlot,
  IntoView,
  NodeId,
  NodeKind,
  Platform,
  RunSlot,
  Scene,
  View,
};
use skene_reactive::Effect;

/// Runs `operation` on a copy of the sink of `node`, if the node is live.
///
/// The scene is released before `operation` runs, so a platform call inside it may
/// reach the scene again.
#[inline(always)]
pub fn with_sink<P: Platform, R>(node: NodeId, operation: impl FnOnce(&P::Sink) -> R) -> Option<R> {
  let sink = Scene::<P>::with(|scene| scene.sink(node))?;

  Some(operation(&sink))
}

/// Binds a render effect that computes `slot`'s leaf and writes it into the box
/// `node`, now and whenever a signal the leaf reads changes.
pub fn bind_box_slot<P: Platform>(node: NodeId, slot: BoxSlot<P::Scalar, P::StyleExt>) {
  let BoxSlot { slot, compute, .. } = slot;

  Effect::render(move |_: Option<()>| {
    let value = compute();

    with_sink::<P, _>(node, |sink| P::default().update_box(sink, slot, &value));
  });
}

/// Binds a render effect that computes `slot`'s leaf and writes it into the run
/// `node`, now and whenever a signal the leaf reads changes.
pub fn bind_run_slot<P: Platform>(node: NodeId, slot: RunSlot<P::StyleExt>) {
  let RunSlot { slot, compute, .. } = slot;

  Effect::render(move |_: Option<()>| {
    let value = compute();

    with_sink::<P, _>(node, |sink| P::default().update_run(sink, slot, &value));
  });
}

/// Reports `access` as what the view that made `node` says to assistive technology,
/// beside whatever describes the node.
pub fn report_access<P: Platform>(node: NodeId, access: Vec<AccessProp>) {
  let access = Scene::<P>::with(|scene| scene.report_access(node, access));

  with_sink::<P, _>(node, |sink| P::default().update_access(sink, &access));
}

/// Reports `description` as what a view describing `node` says to assistive
/// technology, beside the node's own report.
pub fn describe<P: Platform>(node: NodeId, description: Vec<AccessProp>) {
  let access = Scene::<P>::with(|scene| scene.describe(node, description));

  with_sink::<P, _>(node, |sink| P::default().update_access(sink, &access));
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
  pub fn bind<P: Platform>(self, node: NodeId, write: impl Fn(&P::Sink, &T) + 'static) {
    match self {
      Self::Fixed(value) => {
        with_sink::<P, _>(node, |sink| write(sink, &value));
      }
      Self::Derived(compute) => {
        Effect::render(move |_: Option<()>| {
          let value = compute();

          with_sink::<P, _>(node, |sink| write(sink, &value));
        });
      }
    }
  }
}

/// The closure building a view's child under the owner the view provides context on.
#[must_use]
pub struct ChildBuilder<P: Platform>(Box<dyn FnOnce() -> AnyView<P>>);

impl<P: Platform> ChildBuilder<P> {
  /// A builder running `child` when the view builds.
  #[inline(always)]
  pub fn new<V: IntoView<P>>(child: impl FnOnce() -> V + 'static) -> Self {
    Self(Box::new(move || child().into_view()))
  }

  /// Builds the child.
  #[inline(always)]
  pub fn build(self) -> NodeId {
    (self.0)().build()
  }
}

/// The entry a low-level view realizes through: a sink the view made against a
/// platform's escape trait, registered in the scene as an ordinary node.
///
/// The node it returns is a [`View`], so it composes as a built-in does: as the child
/// of a [`Container`](crate::Container) that sizes and decorates it, under a theme,
/// inside a wrapper, with handlers registered on it through the [`Scene`]. A box the
/// view creates through [`Platform::create_box`] instead needs no registration beyond
/// the [`LayoutBox`](crate::LayoutBox) that made it.
#[must_use]
pub struct LowLevel<P: Platform>(PhantomData<fn() -> P>);

impl<P: Platform> LowLevel<P> {
  /// Registers `sink` as a detached node of `kind` and returns its id, recording the
  /// annotation a wrapper around the view gave it.
  #[inline(always)]
  pub fn realize(sink: P::Sink, kind: NodeKind) -> NodeId {
    Scene::<P>::with(|scene| {
      let annotation = scene.take_pending_annotation();
      let node = scene.create(kind, sink);

      if let Some(annotation) = annotation {
        scene.set_annotation(node, annotation);
      }

      node
    })
  }
}
