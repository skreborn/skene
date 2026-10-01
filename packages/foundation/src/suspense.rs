//! Fallback content while a subtree's async work is pending.

use std::{cell::Cell, marker::PhantomData, rc::Rc};

use skene_reactive::{Effect, Owner, on_cleanup, provide_context};

use crate::{
  AnyKind,
  AnyView,
  Boundary,
  NoChild,
  NodeId,
  Platform,
  Segment,
  View,
  realize::ChildBuilder,
};

/// Shows its content once that subtree's async work settles, and the fallback until
/// then.
///
/// The content builds at once under a fresh [`Boundary`], so its tasks start right away
/// and report there; content and fallback are each built once and attached or detached
/// between anchors as the boundary empties and refills, so state inside survives the
/// swaps, and both are destroyed when the owner the view was built under is disposed.
/// Toward an enclosing boundary the whole subtree counts as one pending task. Holds
/// [`NoChild`] until [`set_child_builder`](Self::set_child_builder) supplies the
/// content; it is a builder closure because it is built under the owner the boundary
/// is provided on.
#[derive(Debug)]
#[must_use]
pub struct Suspense<P: Platform, C = NoChild> {
  fallback: Option<AnyView<P>>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> Default for Suspense<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Suspense<P> {
  /// A suspense with no fallback and no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      fallback: None,
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> Suspense<P> {
  /// Sets the fallback shown while the content's work is pending; nothing by default.
  pub fn set_fallback(mut self, fallback: impl View<P>) -> Self {
    self.fallback = Some(fallback.into_any());

    self
  }

  /// Sets the content, built by `child` under the boundary.
  pub fn set_child_builder<V: View<P>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Suspense<P, ChildBuilder<P>> {
    Suspense {
      fallback: self.fallback,
      child: ChildBuilder::new(child),
      platform: PhantomData,
    }
  }
}

impl<P: Platform, C> AnyKind for Suspense<P, C> {}

impl<P: Platform> View<P> for Suspense<P, ChildBuilder<P>> {
  fn build(self) -> NodeId {
    let segment = Segment::<P>::new();
    let start = segment.start();
    let boundary = Boundary::new();
    let parent = Boundary::current();
    let owner = Owner::current();
    let child = self.child;
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
