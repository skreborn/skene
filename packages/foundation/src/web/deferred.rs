//! A streaming boundary: a subtree the server can flush around and stream in place.

use std::marker::PhantomData;

use skene_reactive::{Owner, provide_context};

use super::WebHydration;
use crate::{AnyKind, Boundary, NoChild, NodeId, Scene, View, Wraps, realize::ChildBuilder};

/// A subtree whose async work the server settles apart from the shell's and streams
/// in place once it does.
///
/// Its content builds in place, between anchors of its own. On the server its async
/// work reports to a boundary of its own rather than the shell's, so a streaming render
/// emits everything up to the boundary, blocks until that work settles, then streams
/// the content in document order; a whole render settles through it and serializes it
/// in place. On the client the content builds directly, and hydration adopts the
/// server's already-settled markup. Holds [`NoChild`] until
/// [`set_child_builder`](Self::set_child_builder) supplies the content.
#[must_use]
pub struct Deferred<P: WebHydration, C = NoChild> {
  content: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: WebHydration, C> std::fmt::Debug for Deferred<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Deferred").finish_non_exhaustive()
  }
}

impl<P: WebHydration> Default for Deferred<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: WebHydration> Deferred<P> {
  /// A boundary with no content yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      content: NoChild,
      platform: PhantomData,
    }
  }

  /// Sets the content, built by `content`: its async work reports to a boundary of its
  /// own, and a streaming render flushes the surrounding page before it settles.
  pub fn set_child_builder<V: View<P>>(
    self,
    content: impl FnOnce() -> V + 'static,
  ) -> Deferred<P, ChildBuilder<P>> {
    Deferred {
      content: ChildBuilder::new(content),
      platform: PhantomData,
    }
  }
}

impl<P: WebHydration> Deferred<P, ChildBuilder<P>> {
  /// Builds the content, under a boundary of its own registered for the segment `start`
  /// begins where the platform renders on a server.
  fn build_content(self, start: NodeId) -> NodeId {
    if !P::is_server() {
      return self.content.build();
    }

    let boundary = Boundary::new();
    // A child scope so the boundary is in context for the content's tasks only, not
    // its siblings'; it holds no state of its own and disposes with the render.
    let scope = Owner::current().child();
    let content = scope.enter(|| {
      provide_context(boundary);

      self.content.build()
    });

    P::register_boundary(start, Box::new(move || boundary.is_settled()));

    content
  }
}

impl<P: WebHydration> AnyKind for Deferred<P, ChildBuilder<P>> {}

impl<P: WebHydration> View<P> for Deferred<P, ChildBuilder<P>> {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<P>::create_segment();
    let content = self.build_content(start);

    Scene::<P>::with(|scene| scene.insert_in_segment(start, content, None));

    start
  }
}

/// What a facade's [`Deferred`] carries: its constructor.
pub trait DeferredBase<P: WebHydration>: Wraps<Inner = Deferred<P>> {
  /// A boundary with no content yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(Deferred::new())
  }
}
