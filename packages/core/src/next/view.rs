//! A view, and what is accepted where one is.

use std::marker::PhantomData;

use super::{Driver, NodeId};

/// A view: a one-shot build that mints scene nodes, binds effects, attaches children
/// and returns its root.
///
/// There is no update path; updates are the effects a build left behind.
pub trait View<D: Driver>: 'static {
  /// Realizes this view and returns its root node.
  fn build(self) -> NodeId;

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
/// container that understands its properties.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a compatible and complete view",
  label = "not a compatible and complete view",
  note = "check that it is indeed a compatible view type",
  note = "see its documentation to determine what completes it"
)]
pub trait IntoChildOf<D: Driver, Kind> {
  /// Converts this value into the child's view.
  fn into_child(self) -> AnyView<D>;
}

#[diagnostic::do_not_recommend]
impl<D: Driver, Kind, V: IntoView<D>> IntoChildOf<D, Kind> for V {
  #[inline(always)]
  fn into_child(self) -> AnyView<D> {
    self.into_view()
  }
}
