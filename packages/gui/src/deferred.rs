//! A streaming boundary: a subtree the server can flush around and stream in place.
//!
//! Wrap slow content in a [`Deferred`]. Its content builds in place, between anchors
//! of its own. On the server its async work reports to a boundary of its own rather
//! than the shell's, so a streaming render emits everything up to the boundary,
//! blocks until that work settles, then streams the content in document order; a
//! whole render settles through it and serializes it in place. On the client the
//! content builds directly; hydration adopts the server's already-settled markup.

#[cfg(skene_gui_platform = "server")]
use skene_core::Boundary;
use skene_core::{AnyKind, NodeId, Scene, View};

use crate::ActivePlatform;

/// A subtree whose async work the server settles apart from the shell's and streams
/// in place once it does.
///
/// A streamed page's head flushes with the shell, so a title or head element
/// registered inside the subtree reaches that page only once the client hydrates.
#[must_use]
pub struct Deferred {
  content: Box<dyn FnOnce() -> NodeId>,
}

impl std::fmt::Debug for Deferred {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Deferred").finish_non_exhaustive()
  }
}

impl Deferred {
  /// Defers `content`: its async work reports to a boundary of its own, and a
  /// streaming render flushes the surrounding page before it settles.
  pub fn new<V: View<ActivePlatform>>(content: impl FnOnce() -> V + 'static) -> Self {
    Self {
      content: Box::new(move || content().build()),
    }
  }

  /// Builds the content under a boundary of its own and registers the segment
  /// `start` begins as a streaming boundary.
  #[cfg(skene_gui_platform = "server")]
  fn build_content(self, start: NodeId) -> NodeId {
    let boundary = Boundary::new();
    // A child scope so the boundary is in `Context` for the content's tasks only, not
    // its siblings; it holds no state of its own and disposes with the render.
    let scope = skene_reactive::Owner::current().child();
    let content = scope.enter(|| {
      skene_reactive::provide_context(boundary);

      (self.content)()
    });

    skene_web::register_boundary(start, Box::new(move || boundary.is_settled()));

    content
  }

  /// Builds the content directly; only the server leaf streams.
  #[cfg(not(skene_gui_platform = "server"))]
  fn build_content(self, _start: NodeId) -> NodeId {
    (self.content)()
  }
}

impl AnyKind for Deferred {}

impl View<ActivePlatform> for Deferred {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<ActivePlatform>::create_segment();
    let content = self.build_content(start);

    Scene::<ActivePlatform>::with(|scene| scene.insert_in_segment(start, content, None));

    start
  }
}
