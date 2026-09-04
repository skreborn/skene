//! A streaming boundary: a subtree the server can flush around.
//!
//! Wrap slow content in a [`Deferred`]. Its content builds in place, between anchors
//! of its own; on the server its async work reports to a boundary of its own rather
//! than the shell's, so the page around it settles without waiting on it. Streaming the
//! content as a later chunk is the server leaf's to add; until then a render settles
//! through the boundary and serializes it in place.

#[cfg(target_os = "wasi")]
use skene_core::Boundary;
use skene_core::next::{AnyKind, IntoView, NodeId, Scene, View};

use crate::ActiveDriver;

/// A subtree whose async work the server settles apart from the shell's.
#[must_use]
pub struct Deferred {
  content: Box<dyn FnOnce() -> NodeId>,
}

impl Deferred {
  /// Defers `content`: its async work reports to a boundary of its own.
  #[inline(always)]
  pub fn new<V: IntoView<ActiveDriver>>(content: impl FnOnce() -> V + 'static) -> Self {
    Self {
      content: Box::new(move || content().into_view().build()),
    }
  }

  /// Builds the content under a boundary of its own.
  #[cfg(target_os = "wasi")]
  fn build_content(self) -> NodeId {
    let boundary = Boundary::new();
    // A child scope so the boundary is in `Context` for the content's tasks only, not
    // its siblings; it holds no state of its own and disposes with the render.
    let scope = skene_reactive::Owner::current().child();

    scope.enter(|| {
      skene_reactive::provide_context(boundary);

      (self.content)()
    })
  }

  /// Builds the content directly; the client streams nothing.
  #[cfg(target_os = "unknown")]
  fn build_content(self) -> NodeId {
    (self.content)()
  }
}

impl AnyKind for Deferred {}

impl View<ActiveDriver> for Deferred {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<ActiveDriver>::with(Scene::create_segment);
    let content = self.build_content();

    Scene::<ActiveDriver>::with(|scene| scene.insert_in_segment(start, content, None));

    start
  }
}
