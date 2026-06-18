use crate::{NodeId, dynamic};

/// A view that builds into a backend node.
pub trait View: 'static {
  /// Realizes this view and returns its root node.
  fn build(self) -> NodeId;

  /// Erases this view's concrete type.
  #[inline(always)]
  fn into_any(self) -> AnyView
  where
    Self: Sized,
  {
    AnyView(Box::new(move || self.build()))
  }
}

/// A type-erased view.
#[must_use]
pub struct AnyView(Box<dyn FnOnce() -> NodeId>);

impl AnyView {
  /// Creates a type-erased view from `build`.
  #[inline(always)]
  pub fn new(build: impl FnOnce() -> NodeId + 'static) -> Self {
    AnyView(Box::new(build))
  }
}

impl View for AnyView {
  #[inline(always)]
  fn build(self) -> NodeId {
    (self.0)()
  }

  #[inline(always)]
  fn into_any(self) -> AnyView {
    self
  }
}

/// A value that can be turned into a view.
pub trait IntoView {
  /// Converts this value into a view.
  fn into_view(self) -> AnyView;
}

impl<V: View> IntoView for V {
  #[inline(always)]
  fn into_view(self) -> AnyView {
    self.into_any()
  }
}

/// A reactive subtree.
#[must_use]
pub struct DynamicView {
  build: Box<dyn Fn() -> NodeId>,
}

impl DynamicView {
  /// Creates a reactive subtree.
  #[inline(always)]
  pub fn new<V: View>(build: impl Fn() -> V + 'static) -> Self {
    Self {
      build: Box::new(move || build().build()),
    }
  }
}

impl View for DynamicView {
  fn build(self) -> NodeId {
    dynamic(self.build)
  }
}
