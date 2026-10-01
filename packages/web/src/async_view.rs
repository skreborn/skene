//! A lazily-loaded subtree: content from an async source.

use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use skene_core::{AnyKind, NodeId, Scene, View};
use skene_reactive::Owner;

use crate::ActivePlatform;

/// A lazy segment's place among the hydrating resources, on the leaves that hydrate.
#[cfg(not(skene_web_platform = "headless"))]
type Region = crate::platform::LazyRegion;

/// The host has no hydration, so a lazy segment has no place among resources.
#[cfg(skene_web_platform = "headless")]
struct Region;

/// A subtree whose content arrives asynchronously—typically a [`macro@lazy`](crate::lazy)
/// function whose body loads in a separately-fetched wasm module.
///
/// The content builds and mounts between the view's anchors once the future resolves.
/// Until then the view counts as pending toward the enclosing boundary, so a wrapping
/// [`Suspense`](crate::Suspense) shows its fallback, a [`Deferred`](crate::Deferred)
/// settles apart from it, and a server render settles through it to the content.
///
/// On a hydrated page whose server render included the content, the server markup is
/// claimed instead: it stays visible, already satisfying the enclosing boundary, until
/// the module arrives and the real content replaces it in place.
#[must_use]
pub struct AsyncView {
  load: Box<dyn FnOnce(NodeId)>,
}

impl std::fmt::Debug for AsyncView {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("AsyncView").finish_non_exhaustive()
  }
}

impl AsyncView {
  /// Creates the view over `load`, run to completion as a task.
  pub fn new<V, F>(load: impl FnOnce() -> F + 'static) -> Self
  where
    V: View<ActivePlatform>,
    F: Future<Output = V> + 'static,
  {
    Self {
      load: Box::new(move |start| {
        let claimed = claim(start);
        let region = enter_region();
        let owner = Owner::current().child();
        let work = InRegion::new(region, async move {
          let content = load().await;
          let content = owner.enter(|| content.build());

          if claimed {
            discard(start);
          }

          Scene::<ActivePlatform>::with(|scene| scene.insert_in_segment(start, content, None));
        });

        if claimed {
          // The claimed markup already satisfies the enclosing boundary; the swap
          // reports to a private one nothing watches.
          let scope = Owner::current().child();

          scope.enter(|| {
            skene_reactive::provide_context(skene_core::Boundary::new());
            crate::spawn(work);
          });
        }
        else {
          crate::spawn(work);
        }
      }),
    }
  }
}

/// The lazy segment's place among the hydrating resources, taken as it is built.
#[inline(always)]
#[cfg_attr(
  skene_web_platform = "headless",
  expect(
    clippy::missing_const_for_fn,
    reason = "the web leaves' body is not const"
  )
)]
fn enter_region() -> Region {
  #[cfg(not(skene_web_platform = "headless"))]
  {
    crate::platform::enter_lazy()
  }

  #[cfg(skene_web_platform = "headless")]
  Region
}

/// Runs `work` as the content of `region`, so the resources it creates register or
/// adopt under the segment.
fn within_region<R>(region: &Region, work: impl FnOnce() -> R) -> R {
  #[cfg(not(skene_web_platform = "headless"))]
  {
    crate::platform::within_lazy(region, work)
  }

  #[cfg(skene_web_platform = "headless")]
  {
    let _ = region;

    work()
  }
}

/// The segment's work, polled as the content of its region: the load, the build and the
/// swap alike, so a resource created anywhere along it belongs to the segment.
struct InRegion<F> {
  region: Region,
  work: Pin<Box<F>>,
}

impl<F: Future> InRegion<F> {
  #[inline(always)]
  fn new(region: Region, work: F) -> Self {
    Self {
      region,
      work: Box::pin(work),
    }
  }
}

impl<F: Future> Future for InRegion<F> {
  type Output = F::Output;

  #[inline(always)]
  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = &mut *self;

    within_region(&this.region, || this.work.as_mut().poll(cx))
  }
}

/// Claims the server's markup in the segment, when hydrating over some.
#[inline(always)]
#[cfg_attr(
  not(skene_web_platform = "browser"),
  expect(
    clippy::missing_const_for_fn,
    reason = "the browser leaf's body is not const"
  )
)]
fn claim(start: NodeId) -> bool {
  #[cfg(skene_web_platform = "browser")]
  {
    crate::platform::claim_lazy(start)
  }

  #[cfg(not(skene_web_platform = "browser"))]
  {
    let _ = start;

    false
  }
}

/// Removes the claimed markup, as the real content arrives.
#[inline(always)]
#[cfg_attr(
  not(skene_web_platform = "browser"),
  expect(
    clippy::missing_const_for_fn,
    reason = "the browser leaf's body is not const"
  )
)]
fn discard(start: NodeId) {
  #[cfg(skene_web_platform = "browser")]
  crate::platform::discard_lazy(start);

  #[cfg(not(skene_web_platform = "browser"))]
  let _ = start;
}

impl AnyKind for AsyncView {}

impl View<ActivePlatform> for AsyncView {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<ActivePlatform>::create_segment();

    (self.load)(start);

    start
  }
}
