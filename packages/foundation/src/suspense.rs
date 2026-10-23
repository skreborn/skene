//! Fallback content while a subtree's async work is pending.

use std::{cell::Cell, marker::PhantomData, rc::Rc};

use skene_reactive::{Effect, Owner, on_cleanup, provide_context};

use crate::{
  AnyKind,
  Boundary,
  NodeId,
  Realization,
  Segment,
  View,
  Wraps,
  empty,
  realize::ChildBuilder,
};

/// Shows its content once that subtree's async work settles, and the fallback until
/// then.
///
/// The content builds at once under a fresh [`Boundary`], so its tasks start right away
/// and report there; content and fallback are each built once and attached or detached
/// between anchors as the boundary empties and refills, so state inside survives the
/// swaps, and both are destroyed when the owner the view was built under is disposed.
/// Toward an enclosing boundary the whole subtree counts as one pending task. The
/// content is a builder closure [`set_child_builder`](Self::set_child_builder) supplies,
/// built under the owner the boundary is provided on; without it the view builds
/// nothing, its fallback included.
#[derive(Debug)]
#[must_use]
pub struct Suspense<P: Realization> {
  fallback: Option<ChildBuilder<P>>,
  child: Option<ChildBuilder<P>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization> Default for Suspense<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Suspense<P> {
  /// A suspense with no fallback and no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      fallback: None,
      child: None,
      platform: PhantomData,
    }
  }
}

impl<P: Realization> Suspense<P> {
  /// Sets the fallback shown while the content's work is pending, built by `fallback`;
  /// nothing by default.
  pub fn set_fallback_builder<V: View<P>>(
    mut self,
    fallback: impl FnOnce() -> V + 'static,
  ) -> Self {
    self.fallback = Some(ChildBuilder::new(fallback));

    self
  }

  /// Sets the content, built by `child` under the boundary.
  pub fn set_child_builder<V: View<P>>(mut self, child: impl FnOnce() -> V + 'static) -> Self {
    self.child = Some(ChildBuilder::new(child));

    self
  }
}

impl<P: Realization> AnyKind for Suspense<P> {}

impl<P: Realization> View<P> for Suspense<P> {
  fn build(self) -> NodeId {
    let Some(child) = self.child
    else {
      return empty::<P>().build();
    };
    let segment = Segment::<P>::new();
    let start = segment.start();
    let boundary = Boundary::new();
    let parent = Boundary::current();
    let owner = Owner::current();
    let content = segment.build(owner.child(), || {
      provide_context(boundary);

      child.build()
    });
    let fallback = self
      .fallback
      .map(|fallback| segment.build(owner.child(), || fallback.build()));

    Effect::render(move |previous: Option<bool>| {
      let settled = boundary.is_settled();

      if previous != Some(settled) {
        let (shown, hidden) = if settled {
          (Some(content), fallback)
        }
        else {
          (fallback, Some(content))
        };

        if let Some(hidden) = hidden {
          segment.unmount(hidden);
        }

        if let Some(shown) = shown {
          segment.mount(shown);
        }
      }

      settled
    });

    if let Some(parent) = parent {
      let entered = Rc::new(Cell::new(false));
      let pending = Rc::clone(&entered);

      on_cleanup(move || {
        if pending.get() {
          parent.leave();
        }
      });

      Effect::new(move |_: Option<()>| {
        if boundary.is_settled() {
          if entered.replace(false) {
            parent.leave();
          }
        }
        else if !entered.replace(true) {
          parent.enter();
        }
      });
    }

    start
  }
}

/// What a facade's [`Suspense`] carries: its constructor, its fallback and its content.
pub trait SuspenseBase<P: Realization>: Wraps<Inner = Suspense<P>> {
  /// A suspense with no fallback and no content yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(Suspense::new())
  }

  /// Sets the fallback shown while the content's work is pending, built by `fallback`;
  /// nothing by default.
  #[must_use]
  fn set_fallback_builder<V: View<P>>(self, fallback: impl FnOnce() -> V + 'static) -> Self {
    Self::wrap(self.into_inner().set_fallback_builder(fallback))
  }

  /// Sets the content, built by `child` under the boundary.
  #[must_use]
  fn set_child_builder<V: View<P>>(self, child: impl FnOnce() -> V + 'static) -> Self {
    Self::wrap(self.into_inner().set_child_builder(child))
  }
}
