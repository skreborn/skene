//! Fallback content while a subtree's async work is pending.

use std::{cell::Cell, marker::PhantomData, rc::Rc};

use skene_core::{
  AnyKind,
  AnyView,
  Boundary,
  IntoView,
  NoChild,
  NodeId,
  Platform,
  Scene,
  Show,
  View,
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

impl<P: Platform, C> Suspense<P, C> {
  /// Sets the fallback shown while the content's work is pending; nothing by default.
  #[inline(always)]
  pub fn set_fallback(mut self, fallback: impl IntoView<P>) -> Self {
    self.fallback = Some(fallback.into_view());

    self
  }

  /// Sets the content, built by `child` under the boundary.
  #[inline(always)]
  pub fn set_child_builder<V: IntoView<P>>(
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
    let (start, annotation) =
      Scene::<P>::with(|scene| (scene.create_segment().0, scene.take_pending_annotation()));
    let boundary = Boundary::new();
    let parent = Boundary::current();
    let scope = Owner::current().child();
    let child = self.child;
    let content = AnyView::<P>::new(move || {
      scope.enter(|| {
        provide_context(boundary);

        child.build()
      })
    });
    let content = Scene::<P>::annotate(annotation.clone(), || {
      Show::<P>::new()
        .derive_shown(move || boundary.is_settled())
        .set_child(content)
        .build()
    });

    Scene::<P>::with(|scene| scene.insert_in_segment(start, content, None));

    if let Some(fallback) = self.fallback {
      let fallback = Scene::<P>::annotate(annotation, || {
        Show::<P>::new()
          .derive_shown(move || !boundary.is_settled())
          .set_child(fallback)
          .build()
      });

      Scene::<P>::with(|scene| scene.insert_in_segment(start, fallback, None));
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
