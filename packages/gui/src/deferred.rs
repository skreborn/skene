//! A streaming boundary: a subtree the server can flush around and stream in place.
//!
//! Wrap slow content in a [`Deferred`]. On the server the render emits everything up to the
//! boundary, blocks until the boundary's async work settles, then streams the content in document
//! order — so a client with no JavaScript still receives a correct, progressively-rendered page.
//! On the client the content builds directly; its own reactive subtrees show their placeholders
//! until they load, and hydration adopts the server's already-settled markup.

use skene_core::{IntoView, NodeId, View};

use crate::platform::tree;

/// A subtree whose async work the server streams around, flushing the shell before it settles.
#[must_use]
pub struct Deferred {
  content: Box<dyn FnOnce() -> NodeId>,
}

impl Deferred {
  /// Defers `content` to a streamed chunk: the server flushes the surrounding page, then streams
  /// this subtree once its async work settles.
  #[inline(always)]
  pub fn new<V: IntoView>(content: impl FnOnce() -> V + 'static) -> Self {
    Self {
      content: Box::new(move || content().into_view().build()),
    }
  }
}

impl View for Deferred {
  #[cfg(target_os = "unknown")]
  fn build(self) -> NodeId {
    let node = tree::create_group();
    let content = (self.content)();

    tree::attach(node, content, None);

    node
  }

  #[cfg(target_os = "wasi")]
  fn build(self) -> NodeId {
    let node = tree::create_group();
    let boundary = crate::boundary::Boundary::new();
    // A child scope so the boundary is in `Context` for the content's tasks only, not its
    // siblings; it holds no state of its own and disposes with the render.
    let scope = skene_reactive::Scope::current().create_child();

    let content = scope.enter(|| {
      skene_reactive::Context::provide(boundary);

      (self.content)()
    });

    tree::attach(node, content, None);
    crate::platform::web::register_boundary(node, Box::new(move || boundary.is_settled()));

    node
  }
}
