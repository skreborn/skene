//! A view, and what is accepted where one is.

use std::{hash::Hash, marker::PhantomData};

use skene_reactive::Key;

use super::{Driver, Keyed, NodeId};

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
pub trait View<D: Driver>: AnyKind + 'static {
  /// Realizes this view and returns its root node.
  fn build(self) -> NodeId;

  /// Gives this view a stable identity, so the state created under it keeps its
  /// place in a snapshot wherever the view sits and however its siblings change.
  ///
  /// The key is matched across runs by its hash. Goes after the properties and before
  /// the children: the keyed view still takes them.
  #[inline(always)]
  fn set_key(self, key: impl Hash) -> Keyed<D, Self>
  where
    Self: Sized,
  {
    Keyed::new(Key::of(&key), self)
  }

  /// Erases this view's concrete type.
  #[inline(always)]
  fn into_any(self) -> AnyView<D>
  where
    Self: Sized,
  {
    AnyView::new(move || self.build())
  }
}

/// A type-erased view: a boxed build, or a node already built.
#[must_use]
pub struct AnyView<D: Driver> {
  inner: Inner,
  driver: PhantomData<fn() -> D>,
}

enum Inner {
  Build(Box<dyn FnOnce() -> NodeId>),
  Node(NodeId),
}

impl<D: Driver> AnyView<D> {
  /// A view built by `build`.
  #[inline(always)]
  pub fn new(build: impl FnOnce() -> NodeId + 'static) -> Self {
    Self {
      inner: Inner::Build(Box::new(build)),
      driver: PhantomData,
    }
  }

  /// A view standing for a node already built.
  #[inline(always)]
  pub const fn node(node: NodeId) -> Self {
    Self {
      inner: Inner::Node(node),
      driver: PhantomData,
    }
  }
}

impl<D: Driver> AnyKind for AnyView<D> {}

impl<D: Driver> View<D> for AnyView<D> {
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

impl<D: Driver> View<D> for NodeId {
  #[inline(always)]
  fn build(self) -> NodeId {
    self
  }

  #[inline(always)]
  fn into_any(self) -> AnyView<D> {
    AnyView::node(self)
  }
}

/// A value that can be turned into a view.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a complete view",
  label = "not a complete view",
  note = "check that it is indeed a view type",
  note = "see its documentation to determine what completes it"
)]
pub trait IntoView<D: Driver> {
  /// Converts this value into a view.
  fn into_view(self) -> AnyView<D>;
}

#[diagnostic::do_not_recommend]
impl<D: Driver, V: View<D>> IntoView<D> for V {
  #[inline(always)]
  fn into_view(self) -> AnyView<D> {
    self.into_any()
  }
}

/// The absence of a child, in a view that requires one.
///
/// A wrapper starts out holding this and cannot build; `set_child` exchanges it for
/// the child, and only then is the view a view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoChild;

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
/// Every view is a child of every kind; a wrapper view implements only the kind of the
/// container that understands its properties, and is not [`AnyKind`].
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a compatible and complete view",
  label = "not a compatible and complete view",
  note = "check that it is indeed a compatible view type",
  note = "a wrapper (`Flexible`, `Pinned`, `GridCell`) is accepted only by the container that understands it",
  note = "see its documentation to determine what completes it"
)]
pub trait IntoChildOf<D: Driver, Kind> {
  /// Converts this value into the child's view.
  fn into_child(self) -> AnyView<D>;
}

#[diagnostic::do_not_recommend]
impl<D: Driver, Kind, V: IntoView<D> + AnyKind> IntoChildOf<D, Kind> for V {
  #[inline(always)]
  fn into_child(self) -> AnyView<D> {
    self.into_view()
  }
}
