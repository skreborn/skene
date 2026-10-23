//! Fallback content while a subtree holds reported errors.

use std::{marker::PhantomData, rc::Rc};

use skene_reactive::{Effect, Owner, provide_context};

use crate::{
  AnyKind,
  AnyView,
  Errors,
  NodeId,
  Realization,
  Segment,
  View,
  Wraps,
  derived,
  empty,
  realize::ChildBuilder,
};

/// Shows its content while no errors are held, and the fallback once one is reported.
///
/// The boundary provides an [`Errors`] handle to its content through the reactive
/// context; the fallback receives the same handle, to present the errors and dismiss
/// them. The content is built once and attached or detached between anchors as errors
/// arrive and clear, so clearing restores it as it was; the fallback rebuilds whenever
/// the held errors change; both are destroyed when the owner the view was built under
/// is disposed. The content is a builder closure
/// [`set_child_builder`](Self::set_child_builder) supplies, built under the owner the
/// handle is provided on; without it the view builds nothing, its fallback included.
#[must_use]
pub struct ErrorBoundary<P: Realization> {
  fallback: Option<Rc<dyn Fn(Errors) -> AnyView<P>>>,
  child: Option<ChildBuilder<P>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization> std::fmt::Debug for ErrorBoundary<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ErrorBoundary")
      .finish_non_exhaustive()
  }
}

impl<P: Realization> Default for ErrorBoundary<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> ErrorBoundary<P> {
  /// A boundary with no fallback and no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      fallback: None,
      child: None,
      platform: PhantomData,
    }
  }
}

impl<P: Realization> ErrorBoundary<P> {
  /// Sets the fallback shown while errors are held, built by `fallback` from the
  /// boundary's handle whenever the held errors change; nothing by default.
  pub fn set_fallback_builder<V: View<P>>(
    mut self,
    fallback: impl Fn(Errors) -> V + 'static,
  ) -> Self {
    self.fallback = Some(Rc::new(move |errors| fallback(errors).into_any()));

    self
  }

  /// Sets the content, built by `child` under the boundary.
  pub fn set_child_builder<V: View<P>>(mut self, child: impl FnOnce() -> V + 'static) -> Self {
    self.child = Some(ChildBuilder::new(child));

    self
  }
}

impl<P: Realization> AnyKind for ErrorBoundary<P> {}

impl<P: Realization> View<P> for ErrorBoundary<P> {
  fn build(self) -> NodeId {
    let Some(child) = self.child
    else {
      return empty::<P>().build();
    };
    let segment = Segment::<P>::new();
    let start = segment.start();
    let errors = Errors::new();
    let owner = Owner::current();
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

/// What a facade's [`ErrorBoundary`] carries: its constructor, its fallback and its
/// content.
pub trait ErrorBoundaryBase<P: Realization>: Wraps<Inner = ErrorBoundary<P>> {
  /// A boundary with no fallback and no content yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(ErrorBoundary::new())
  }

  /// Sets the fallback shown while errors are held, built by `fallback` from the
  /// boundary's handle whenever the held errors change; nothing by default.
  #[must_use]
  fn set_fallback_builder<V: View<P>>(self, fallback: impl Fn(Errors) -> V + 'static) -> Self {
    Self::wrap(self.into_inner().set_fallback_builder(fallback))
  }

  /// Sets the content, built by `child` under the boundary.
  #[must_use]
  fn set_child_builder<V: View<P>>(self, child: impl FnOnce() -> V + 'static) -> Self {
    Self::wrap(self.into_inner().set_child_builder(child))
  }
}
