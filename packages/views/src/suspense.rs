//! Fallback content while a subtree's async work is pending.

use std::{cell::Cell, marker::PhantomData, rc::Rc};

use skene_core::{
  Boundary,
  next::{AnyKind, AnyView, Driver, IntoView, NoChild, NodeId, Scene, Show, View},
};
use skene_reactive::{Effect, Owner, on_cleanup, provide_context};

use crate::realize::ChildBuilder;

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
#[must_use]
pub struct Suspense<D: Driver, C = NoChild> {
  fallback: Option<AnyView<D>>,
  child: C,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> Default for Suspense<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> Suspense<D> {
  /// A suspense with no fallback and no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      fallback: None,
      child: NoChild,
      driver: PhantomData,
    }
  }
}

impl<D: Driver, C> Suspense<D, C> {
  /// Sets the fallback shown while the content's work is pending; nothing by default.
  #[inline(always)]
  pub fn set_fallback(mut self, fallback: impl IntoView<D>) -> Self {
    self.fallback = Some(fallback.into_view());

    self
  }

  /// Sets the content, built by `child` under the boundary.
  #[inline(always)]
  pub fn set_child_builder<V: IntoView<D>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Suspense<D, ChildBuilder<D>> {
    Suspense {
      fallback: self.fallback,
      child: ChildBuilder::new(child),
      driver: PhantomData,
    }
  }
}

impl<D: Driver, C> AnyKind for Suspense<D, C> {}

impl<D: Driver> View<D> for Suspense<D, ChildBuilder<D>> {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<D>::with(Scene::create_segment);
    let boundary = Boundary::new();
    let parent = Boundary::current();
    let scope = Owner::current().child();
    let child = self.child;
    let content = AnyView::<D>::new(move || {
      scope.enter(|| {
        provide_context(boundary);

        child.build()
      })
    });
    let content = Show::<D>::new()
      .derive_shown(move || boundary.is_settled())
      .set_child(content)
      .build();

    Scene::<D>::with(|scene| scene.insert_in_segment(start, content, None));

    if let Some(fallback) = self.fallback {
      let fallback = Show::<D>::new()
        .derive_shown(move || !boundary.is_settled())
        .set_child(fallback)
        .build();

      Scene::<D>::with(|scene| scene.insert_in_segment(start, fallback, None));
    }

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
