//! A lazily-loaded subtree: content from an async source.

use std::future::Future;

use skene_core::{IntoView, NodeId, View};

use crate::driver as tree;

/// A subtree whose content arrives asynchronously—typically a [`macro@lazy`](crate::lazy)
/// function whose body loads in a separately-fetched wasm module.
///
/// The content builds and attaches once the future resolves. Until then the view
/// counts as pending toward the enclosing boundary, so a wrapping
/// [`Suspense`](crate::Suspense) shows its fallback, a [`Deferred`](crate::Deferred)
/// streams around it, and a server render settles through it to the content.
#[must_use]
pub struct Lazy {
  load: Box<dyn FnOnce(NodeId)>,
}

impl Lazy {
  /// Creates the view over `load`, run to completion as a task.
  #[inline(always)]
  pub fn new<V, F>(load: impl FnOnce() -> F + 'static) -> Self
  where
    V: IntoView,
    F: Future<Output = V> + 'static,
  {
    Self {
      load: Box::new(move |node| {
        crate::spawn(async move {
          let content = load().await.into_view().build();

          tree::attach(node, content, None);
        });
      }),
    }
  }
}

impl View for Lazy {
  fn build(self) -> NodeId {
    let node = tree::create_group();

    (self.load)(node);

    node
  }
}
