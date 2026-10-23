//! A lazily loaded subtree: content from an async source.

use std::{
  future::Future,
  marker::PhantomData,
  pin::Pin,
  task::{Context, Poll},
};

use skene_reactive::{Owner, provide_context, spawn};

use super::WebHydration;
use crate::{AnyKind, Boundary, NodeId, Scene, View, Wraps, empty};

/// A subtree whose content arrives asynchronously—typically a `#[lazy]` function whose
/// body loads in a separately fetched wasm module.
///
/// The content builds and mounts between the view's anchors once the future resolves.
/// Until then the view counts as pending toward the enclosing boundary, so a wrapping
/// [`Suspense`](crate::Suspense) shows its fallback, a [`Deferred`](super::Deferred)
/// settles apart from it, and a server render settles through it to the content.
///
/// On a hydrated page whose server render included the content, the server markup is
/// claimed instead: it stays visible, already satisfying the enclosing boundary, until
/// the module arrives and the real content replaces it in place. Without the content
/// [`set_child_loader`](Self::set_child_loader) supplies, it renders nothing.
#[must_use]
pub struct AsyncView<P: WebHydration> {
  load: Option<ChildLoader>,
  platform: PhantomData<fn() -> P>,
}

impl<P: WebHydration> std::fmt::Debug for AsyncView<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("AsyncView").finish_non_exhaustive()
  }
}

/// The loader of an [`AsyncView`]'s content, run to completion as a task once the view
/// builds, given the start of the view's segment.
type ChildLoader = Box<dyn FnOnce(NodeId)>;

impl<P: WebHydration> Default for AsyncView<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: WebHydration> AsyncView<P> {
  /// A view with no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      load: None,
      platform: PhantomData,
    }
  }

  /// Sets the content, loaded by `load`, run to completion as a task.
  pub fn set_child_loader<V, F>(mut self, load: impl FnOnce() -> F + 'static) -> Self
  where
    V: View<P>,
    F: Future<Output = V> + 'static,
  {
    self.load = Some(Box::new(move |start| {
      let claimed = P::claim_lazy(start);
      let region = P::enter_lazy();
      let owner = Owner::current().child();
      let work = InRegion::<P, _>::new(region, async move {
        let content = load().await;
        let content = owner.enter(|| content.build());

        if claimed {
          P::discard_lazy(start);
        }

        Scene::<P>::with(|scene| scene.insert_in_segment(start, content, None));
      });

      P::install_executor();

      if claimed {
        // The claimed markup already satisfies the enclosing boundary; the swap
        // reports to a private one nothing watches.
        let scope = Owner::current().child();

        scope.enter(|| {
          provide_context(Boundary::new());
          spawn(work);
        });
      }
      else {
        spawn(work);
      }
    }));

    self
  }
}

/// The segment's work, polled as the content of its region: the load, the build and the
/// swap alike, so a resource created anywhere along it belongs to the segment.
struct InRegion<P: WebHydration, F> {
  region: P::LazyRegion,
  work: Pin<Box<F>>,
}

impl<P: WebHydration, F: Future> InRegion<P, F> {
  fn new(region: P::LazyRegion, work: F) -> Self {
    Self {
      region,
      work: Box::pin(work),
    }
  }
}

impl<P: WebHydration, F: Future> Future for InRegion<P, F> {
  type Output = F::Output;

  #[inline(always)]
  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = &mut *self;

    P::within_lazy(&this.region, || this.work.as_mut().poll(cx))
  }
}

impl<P: WebHydration> AnyKind for AsyncView<P> {}

impl<P: WebHydration> View<P> for AsyncView<P> {
  fn build(self) -> NodeId {
    let Some(load) = self.load
    else {
      return empty::<P>().build();
    };
    let (start, _) = Scene::<P>::create_segment();

    load(start);

    start
  }
}

/// What a facade's [`AsyncView`] carries: its constructor and its content's loader.
pub trait AsyncViewBase<P: WebHydration>: Wraps<Inner = AsyncView<P>> {
  /// A view with no content yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(AsyncView::new())
  }

  /// Sets the content, loaded by `load`, run to completion as a task.
  #[must_use]
  fn set_child_loader<V, F>(self, load: impl FnOnce() -> F + 'static) -> Self
  where
    V: View<P>,
    F: Future<Output = V> + 'static,
  {
    Self::wrap(self.into_inner().set_child_loader(load))
  }
}
