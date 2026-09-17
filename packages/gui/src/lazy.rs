//! A lazily-loaded subtree: content from an async source.

use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use skene_core::{AnyKind, IntoView, NodeId, Scene, View};
use skene_reactive::Owner;

use crate::ActivePlatform;

/// A lazy segment's place among the hydrating resources, on the leaves that hydrate.
#[cfg(target_arch = "wasm32")]
type Region = skene_web::LazyRegion;

/// The host has no hydration, so a lazy segment has no place among resources.
#[cfg(not(target_arch = "wasm32"))]
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
pub struct Lazy {
  load: Box<dyn FnOnce(NodeId)>,
}

impl Lazy {
  /// Creates the view over `load`, run to completion as a task.
  #[inline(always)]
  pub fn new<V, F>(load: impl FnOnce() -> F + 'static) -> Self
  where
    V: IntoView<ActivePlatform>,
    F: Future<Output = V> + 'static,
  {
    Self {
      load: Box::new(move |start| {
        let claimed = claim(start);
        let region = enter_region();
        let owner = Owner::current().child();
        let work = InRegion::new(region, async move {
          let content = load().await;
          let content = owner.enter(|| content.into_view().build());

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
fn enter_region() -> Region {
  #[cfg(target_arch = "wasm32")]
  {
    skene_web::enter_lazy()
  }

  #[cfg(not(target_arch = "wasm32"))]
  Region
}

/// Runs `work` as the content of `region`, so the resources it creates register or
/// adopt under the segment.
#[inline(always)]
fn within_region<R>(region: &Region, work: impl FnOnce() -> R) -> R {
  #[cfg(target_arch = "wasm32")]
  {
    skene_web::within_lazy(region, work)
  }

  #[cfg(not(target_arch = "wasm32"))]
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
fn claim(start: NodeId) -> bool {
  #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
  {
    skene_web::claim_lazy(start)
  }

  #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
  {
    let _ = start;

    false
  }
}

/// Removes the claimed markup, as the real content arrives.
#[inline(always)]
fn discard(start: NodeId) {
  #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
  skene_web::discard_lazy(start);

  #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
  let _ = start;
}

impl AnyKind for Lazy {}

impl View<ActivePlatform> for Lazy {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<ActivePlatform>::with(Scene::create_segment);

    (self.load)(start);

    start
  }
}
