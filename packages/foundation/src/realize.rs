//! What every view's build shares.
//!
//! Reaching a node's sink, binding the render effect behind each derived slot, writing
//! a value that is fixed or derived, building a child under a provided context, and
//! registering a platform-made sink, or a platform's own view, as a node.

use std::{marker::PhantomData, rc::Rc};

use skene_reactive::Effect;

use crate::{
  AccessProp,
  Annotation,
  AnyView,
  CompiledStyle,
  ControlSink,
  DerivedSlot,
  NodeId,
  NodeKind,
  NodeSpec,
  Realization,
  Record,
  Scene,
  SlotLeaf,
  StaticLeaf,
  View,
};

/// Binds a render effect that computes `slot`'s leaf and writes it, erased over its
/// property, into the slot on `node`, now and whenever a signal the leaf reads changes.
///
/// Each write first lays over the scene's root the default of every palette a token of
/// the leaf links to, where nothing laid it before.
pub fn bind_slot<P: Realization, K, V: 'static>(node: NodeId, slot: DerivedSlot<K, V>)
where
  StaticLeaf<V>: Into<SlotLeaf<P::Scalar, P::StyleExt>>,
{
  let DerivedSlot { slot, compute, .. } = slot;

  Effect::render(move |_: Option<()>| {
    let leaf = compute().into();

    Scene::<P>::define_referenced(&leaf);
    P::apply_slot(node, slot, &leaf);
  });
}

/// Gives `node` its style through the platform ([`Realization::apply_style`]).
///
/// It first lays over the scene's root the default of every palette a token of the style
/// links to, where nothing laid it before.
pub fn apply_style<P: Realization>(node: NodeId, style: CompiledStyle<P::Scalar, P::StyleExt>) {
  Scene::<P>::define_referenced(&style);
  P::apply_style(node, style);
}

/// Reports `access` as what the view that made `node` says to assistive technology,
/// beside whatever describes the node.
pub fn report_access<P: Realization>(node: NodeId, access: Vec<AccessProp>) {
  let access = Scene::<P>::with(|scene| scene.report_access(node, access));

  P::update_access(node, &access);
}

/// Reports `description` as what a view describing `node` says to assistive
/// technology, beside the node's own report.
pub fn describe<P: Realization>(node: NodeId, description: Vec<AccessProp>) {
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
pub struct ChildBuilder<P: Realization>(Box<dyn FnOnce() -> AnyView<P>>);

impl<P: Realization> std::fmt::Debug for ChildBuilder<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ChildBuilder")
      .finish_non_exhaustive()
  }
}

impl<P: Realization> ChildBuilder<P> {
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

/// The entry a low-level view realizes through: a node the scene mints, an ordinary node
/// from then on.
///
/// The view realizes the node against a platform's escape trait, or has the platform
/// adopt its own view as the node through the native door.
///
/// The node it returns is a [`View`], so it composes as a built-in does: as the child
/// of a [`Container`](crate::Container) that sizes and decorates it, under a theme,
/// inside a wrapper, with handlers registered on it through the [`Scene`]. A box the
/// view builds through a [`LayoutBox`](crate::LayoutBox) instead needs no registration
/// beyond it.
#[derive(Debug)]
#[must_use]
pub struct LowLevel<P: Realization>(PhantomData<fn() -> P>);

impl<P: Realization> LowLevel<P> {
  /// Mints a detached node of `kind`, has `make` realize it against a platform's
  /// escape trait, and returns its id, applying the annotation a wrapper around the
  /// view gave it to the node as a style holding that layout alone; see
  /// [`Realization::apply_style`].
  pub fn realize(kind: NodeKind, make: impl FnOnce(NodeId)) -> NodeId {
    let (node, annotation) = Scene::<P>::with(|scene| {
      let annotation = scene.take_pending_annotation();

      (scene.create(kind), annotation)
    });

    make(node);
    Self::apply_annotation(node, annotation);

    node
  }

  /// Mints a detached node as `spec` describes it, has the platform adopt `native`, its
  /// own view, as the node through the native door, and returns its id, applying the
  /// annotation a wrapper around the view gave it as [`realize`](Self::realize) does;
  /// see [`Realization::adopt`].
  pub fn adopt(spec: &NodeSpec, native: P::Native) -> NodeId {
    let annotation = Scene::<P>::with(Scene::take_pending_annotation);
    let node = Scene::<P>::adopt(spec, native);

    Self::apply_annotation(node, annotation);

    node
  }

  /// Registers `sink` on `node`, a view adopted through the native door, as what reads
  /// and writes its door's contents ([`ContentKey::door`](crate::ContentKey::door)), so a
  /// [`Controller`](crate::Controller) of one writes and reads through it; it is
  /// registered before the controller is, which reads what the view shows through it.
  pub fn register_sink(node: NodeId, sink: impl ControlSink + 'static) {
    let sink: Rc<dyn ControlSink> = Rc::new(sink);

    Scene::<P>::with(|scene| scene.set_sink(node, sink));
  }

  /// Applies the layout `annotation` holds to `node` as a style holding it alone, where
  /// a wrapper gave one.
  fn apply_annotation(node: NodeId, annotation: Option<Annotation<P>>) {
    let Some(annotation) = annotation
    else {
      return;
    };
    let (layout, slots) = annotation.into_style().split();
    let mut record = Record::new();

    record.add_box(&layout);
    apply_style::<P>(node, record.into_kept());

    for slot in slots {
      bind_slot::<P, _, _>(node, slot);
    }
  }
}
