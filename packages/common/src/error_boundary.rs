//! Fallback content while a subtree holds reported errors.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{
  AnyKind,
  AnyView,
  Errors,
  IntoView,
  NoChild,
  NodeId,
  Platform,
  Segment,
  View,
  derived,
  empty,
};
use skene_reactive::{Effect, Owner, provide_context};

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
pub struct ErrorBoundary<P: Platform, C = NoChild> {
  fallback: Option<Rc<dyn Fn(Errors) -> AnyView<P>>>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, C> std::fmt::Debug for ErrorBoundary<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ErrorBoundary")
      .finish_non_exhaustive()
  }
}

impl<P: Platform> Default for ErrorBoundary<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> ErrorBoundary<P> {
  /// A boundary with no fallback and no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      fallback: None,
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> ErrorBoundary<P> {
  /// Sets the fallback shown while errors are held, built by `fallback` from the
  /// boundary's handle whenever the held errors change; nothing by default.
  #[inline(always)]
  pub fn set_fallback_builder<V: IntoView<P>>(
    mut self,
    fallback: impl Fn(Errors) -> V + 'static,
  ) -> Self {
    self.fallback = Some(Rc::new(move |errors| fallback(errors).into_view()));

    self
  }

  /// Sets the content, built by `child` under the boundary.
  #[inline(always)]
  pub fn set_child_builder<V: IntoView<P>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> ErrorBoundary<P, ChildBuilder<P>> {
    ErrorBoundary {
      fallback: self.fallback,
      child: ChildBuilder::new(child),
      platform: PhantomData,
    }
  }
}

impl<P: Platform, C> AnyKind for ErrorBoundary<P, C> {}

impl<P: Platform> View<P> for ErrorBoundary<P, ChildBuilder<P>> {
  fn build(self) -> NodeId {
    let segment = Segment::<P>::new();
    let start = segment.start();
    let errors = Errors::new();
    let owner = Owner::current();
    let child = self.child;
    let content = segment.build(owner.child(), || {
      provide_context(errors);

      child.build()
    });
    let fallback = self.fallback.map(|fallback| {
      segment.build(owner.child(), || {
        derived::<P>(move || {
          if errors.is_empty() {
            empty::<P>().build()
          }
          else {
            fallback(errors).build()
          }
        })
      })
    });

    if let Some(fallback) = fallback {
      segment.mount(fallback);
    }

    Effect::render(move |previous: Option<bool>| {
      let clear = errors.is_empty();

      if previous != Some(clear) {
        match (clear, fallback) {
          (true, Some(fallback)) => segment.mount_before(content, fallback),
          (true, None) => segment.mount(content),
          (false, _) => segment.unmount(content),
        }
      }

      clear
    });

    start
  }
}
