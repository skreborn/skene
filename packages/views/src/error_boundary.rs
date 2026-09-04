//! Fallback content while a subtree holds reported errors.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{
  Errors,
  next::{AnyKind, AnyView, Driver, IntoView, NoChild, NodeId, Scene, Show, View, derived, empty},
};
use skene_reactive::{Owner, provide_context};

use crate::realize::ChildBuilder;

/// Shows its content while no errors are held, and the fallback once one is reported.
///
/// The boundary provides an [`Errors`] handle to its content through the reactive
/// context; the fallback receives the same handle, to present the errors and dismiss
/// them. The content is built once and attached or detached between anchors as errors
/// arrive and clear, so clearing restores it as it was; the fallback rebuilds whenever
/// the held errors change; both are destroyed when the owner the view was built under
/// is disposed. Holds [`NoChild`] until [`set_child_builder`](Self::set_child_builder)
/// supplies the content; it is a builder closure because it is built under the owner
/// the handle is provided on.
#[must_use]
pub struct ErrorBoundary<D: Driver, C = NoChild> {
  fallback: Option<Rc<dyn Fn(Errors) -> AnyView<D>>>,
  child: C,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> Default for ErrorBoundary<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> ErrorBoundary<D> {
  /// A boundary with no fallback and no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      fallback: None,
      child: NoChild,
      driver: PhantomData,
    }
  }
}

impl<D: Driver, C> ErrorBoundary<D, C> {
  /// Sets the fallback shown while errors are held, built by `fallback` from the
  /// boundary's handle whenever the held errors change; nothing by default.
  #[inline(always)]
  pub fn set_fallback_builder<V: IntoView<D>>(
    mut self,
    fallback: impl Fn(Errors) -> V + 'static,
  ) -> Self {
    self.fallback = Some(Rc::new(move |errors| fallback(errors).into_view()));

    self
  }

  /// Sets the content, built by `child` under the boundary.
  #[inline(always)]
  pub fn set_child_builder<V: IntoView<D>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> ErrorBoundary<D, ChildBuilder<D>> {
    ErrorBoundary {
      fallback: self.fallback,
      child: ChildBuilder::new(child),
      driver: PhantomData,
    }
  }
}

impl<D: Driver, C> AnyKind for ErrorBoundary<D, C> {}

impl<D: Driver> View<D> for ErrorBoundary<D, ChildBuilder<D>> {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<D>::with(Scene::create_segment);
    let errors = Errors::new();
    let scope = Owner::current().child();
    let child = self.child;
    let content = AnyView::<D>::new(move || {
      scope.enter(|| {
        provide_context(errors);

        child.build()
      })
    });
    let content = Show::<D>::new()
      .derive_shown(move || errors.is_empty())
      .set_child(content)
      .build();

    Scene::<D>::with(|scene| scene.insert_in_segment(start, content, None));

    if let Some(fallback) = self.fallback {
      let fallback = derived::<D>(move || {
        if errors.is_empty() {
          empty::<D>().build()
        }
        else {
          fallback(errors).build()
        }
      });

      Scene::<D>::with(|scene| scene.insert_in_segment(start, fallback, None));
    }

    start
  }
}
