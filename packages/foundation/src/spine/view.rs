//! A view, and what is accepted where one is.

use std::{hash::Hash, marker::PhantomData};

use skene_reactive::Key;

use super::{Keyed, NodeId, Realization};

/// A view accepted as a child of every kind.
///
/// Every view has it; a wrapper carrying per-child properties (`Flexible`, `Pinned`,
/// `GridCell`) does not, and implements [`IntoChildOf`] for the one kind that
/// understands them instead.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a view of every kind",
  label = "a view implements `AnyKind`; a wrapper implements `IntoChildOf` for its one kind instead"
)]
pub trait AnyKind {}

/// A view: a one-shot build that mints scene nodes, binds effects, attaches children
/// and returns its root.
///
/// There is no update path; updates are the effects a build left behind.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a view",
  label = "not a view",
  note = "a view is a built-in such as `Text` or `Container`, or a type implementing `View`"
)]
pub trait View<P: Realization>: AnyKind + 'static {
  /// Realizes this view and returns its root node.
  fn build(self) -> NodeId;

  /// Gives this view a stable identity, so the state created under it keeps its
  /// place in a snapshot wherever the view sits and however its siblings change.
  ///
  /// The key is matched across runs by its hash. Goes after the properties and before
  /// the children: the keyed view still takes them.
  fn set_key(self, key: impl Hash) -> Keyed<P, Self>
  where
    Self: Sized,
  {
    Keyed::new(Key::of(&key), self)
  }

  /// Erases this view's concrete type.
  #[inline(always)]
  fn into_any(self) -> AnyView<P>
  where
    Self: Sized,
  {
    AnyView::new(move || self.build())
  }
}

/// A type-erased view: a boxed build, or a node already built.
#[must_use]
pub struct AnyView<P: Realization> {
  inner: Inner,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization> std::fmt::Debug for AnyView<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("AnyView").finish_non_exhaustive()
  }
}

enum Inner {
  Build(Box<dyn FnOnce() -> NodeId>),
  Node(NodeId),
}

impl<P: Realization> AnyView<P> {
  /// A view built by `build`.
  pub fn new(build: impl FnOnce() -> NodeId + 'static) -> Self {
    Self {
      inner: Inner::Build(Box::new(build)),
      platform: PhantomData,
    }
  }

  /// A view standing for a node already built.
  #[inline(always)]
  pub const fn node(node: NodeId) -> Self {
    Self {
      inner: Inner::Node(node),
      platform: PhantomData,
    }
  }
}

impl<P: Realization> AnyKind for AnyView<P> {}

impl<P: Realization> View<P> for AnyView<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    match self.inner {
      Inner::Build(build) => build(),
      Inner::Node(node) => node,
    }
  }

  #[inline(always)]
  fn into_any(self) -> Self {
    self
  }
}

impl AnyKind for NodeId {}

impl<P: Realization> View<P> for NodeId {
  #[inline(always)]
  fn build(self) -> NodeId {
    self
  }

  #[inline(always)]
  fn into_any(self) -> AnyView<P> {
    AnyView::node(self)
  }
}

/// The child kind a plain container accepts: any view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlainChild;

/// The child kind a flex container accepts: any view, or a `Flexible` wrapper.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlexChild;

/// The child kind a stack accepts: any view, or a `Pinned` wrapper.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StackChild;

/// The child kind a grid accepts: any view, or a `GridCell` wrapper.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GridChild;

/// A value accepted as one child of a container of kind `Kind`.
///
/// Every view is a child of every kind; a wrapper implements it only for the kind of
/// the container that understands its properties, and is not [`AnyKind`].
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a child this container accepts",
  label = "not a child this container accepts",
  note = "a container accepts any view, and a wrapper of its own kind",
  note = "a wrapper (`Flexible`, `Pinned`, `GridCell`) is accepted only by the container that understands it"
)]
pub trait IntoChildOf<P: Realization, Kind> {
  /// Converts this value into the child's view.
  fn into_child(self) -> AnyView<P>;
}

#[diagnostic::do_not_recommend]
impl<P: Realization, Kind, V: View<P>> IntoChildOf<P, Kind> for V {
  #[inline(always)]
  fn into_child(self) -> AnyView<P> {
    self.into_any()
  }
}
