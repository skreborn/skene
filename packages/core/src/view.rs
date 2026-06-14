use skene_renderer::NodeId;

use crate::views::Label;

/// A node in the application's view tree.
///
/// `build` is the single responsibility of a view: realize itself against the
/// active backend (creating nodes, applying styles, wiring reactive effects and
/// event handlers) and return the id of its root node. The browser — or, in
/// future, a native/TUI backend — handles layout, painting and hit-testing.
pub trait View: 'static {
  /// Realize this view and return its root node.
  fn build(self) -> NodeId;

  /// Erase this view's concrete type into an [`AnyView`].
  fn into_any(self) -> AnyView
  where
    Self: Sized,
  {
    AnyView(Box::new(move || self.build()))
  }
}

/// A type-erased view: a deferred build closure.
pub struct AnyView(Box<dyn FnOnce() -> NodeId>);

impl AnyView {
  pub fn new(build: impl FnOnce() -> NodeId + 'static) -> Self {
    AnyView(Box::new(build))
  }
}

impl View for AnyView {
  fn build(self) -> NodeId {
    (self.0)()
  }

  fn into_any(self) -> AnyView {
    self
  }
}

/// Anything that can be turned into a view.
///
/// A blanket impl covers every [`View`]; additional impls provide ergonomic
/// conversions (e.g. string literals become text).
pub trait IntoView {
  fn into_view(self) -> AnyView;
}

impl<V: View> IntoView for V {
  fn into_view(self) -> AnyView {
    self.into_any()
  }
}

impl IntoView for &str {
  fn into_view(self) -> AnyView {
    Label::new(self).into_any()
  }
}

impl IntoView for String {
  fn into_view(self) -> AnyView {
    Label::new(self).into_any()
  }
}
