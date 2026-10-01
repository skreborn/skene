//! A streaming boundary: a subtree the server can flush around and stream in place.

use std::marker::PhantomData;

use skene_reactive::{Owner, provide_context};

use super::WebHydration;
use crate::{AnyKind, Boundary, NodeId, Scene, View, Wraps};

/// A subtree whose async work the server settles apart from the shell's and streams
/// in place once it does.
///
/// Its content builds in place, between anchors of its own. On the server its async
/// work reports to a boundary of its own rather than the shell's, so a streaming render
/// emits everything up to the boundary, blocks until that work settles, then streams
/// the content in document order; a whole render settles through it and serializes it
/// in place. On the client the content builds directly, and hydration adopts the
/// server's already-settled markup.
#[must_use]
pub struct Deferred<P: WebHydration> {
  content: Box<dyn FnOnce() -> NodeId>,
  platform: PhantomData<fn() -> P>,
}

impl<P: WebHydration> std::fmt::Debug for Deferred<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Deferred").finish_non_exhaustive()
  }
}

impl<P: WebHydration> Deferred<P> {
  /// Defers `content`: its async work reports to a boundary of its own, and a
  /// streaming render flushes the surrounding page before it settles.
  pub fn new<V: View<P>>(content: impl FnOnce() -> V + 'static) -> Self {
    Self {
      content: Box::new(move || content().build()),
      platform: PhantomData,
    }
  }

  /// Builds the content, under a boundary of its own registered for the segment `start`
  /// begins where the platform renders on a server.
  fn build_content(self, start: NodeId) -> NodeId {
    if !P::is_server() {
      return (self.content)();
    }

    let boundary = Boundary::new();
    // A child scope so the boundary is in context for the content's tasks only, not
    // its siblings'; it holds no state of its own and disposes with the render.
    let scope = Owner::current().child();
    let content = scope.enter(|| {
      provide_context(boundary);

      (self.content)()
    });

    P::register_boundary(start, Box::new(move || boundary.is_settled()));

    content
  }
}

impl<P: WebHydration> AnyKind for Deferred<P> {}

impl<P: WebHydration> View<P> for Deferred<P> {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<P>::create_segment();
    let content = self.build_content(start);

    Scene::<P>::with(|scene| scene.insert_in_segment(start, content, None));

    start
  }
}

/// What a facade's [`Deferred`] carries: its constructor.
pub trait DeferredBase<P: WebHydration>: Wraps<Inner = Deferred<P>> {
  /// Defers `content`: its async work reports to a boundary of its own, and a
  /// streaming render flushes the surrounding page before it settles.
  #[must_use]
  fn new<V: View<P>>(content: impl FnOnce() -> V + 'static) -> Self {
    Self::wrap(Deferred::new(content))
  }
}
