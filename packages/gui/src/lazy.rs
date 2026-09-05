//! A lazily-loaded subtree: content from an async source.

use std::future::Future;

use skene_core::{AnyKind, IntoView, NodeId, Scene, View};
use skene_reactive::Owner;

use crate::ActiveDriver;

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
    V: IntoView<ActiveDriver>,
    F: Future<Output = V> + 'static,
  {
    Self {
      load: Box::new(move |start| {
        let claimed = claim(start);
        let owner = Owner::current().child();
        let work = async move {
          let content = load().await;
          let content = owner.enter(|| content.into_view().build());

          if claimed {
            discard(start);
          }

          Scene::<ActiveDriver>::with(|scene| scene.insert_in_segment(start, content, None));
        };

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

/// Claims the server's markup in the segment, when hydrating over some.
#[inline(always)]
fn claim(start: NodeId) -> bool {
  #[cfg(target_os = "unknown")]
  {
    skene_web::claim_lazy(start)
  }

  #[cfg(target_os = "wasi")]
  {
    let _ = start;

    false
  }
}

/// Removes the claimed markup, as the real content arrives.
#[inline(always)]
fn discard(start: NodeId) {
  #[cfg(target_os = "unknown")]
  skene_web::discard_lazy(start);

  #[cfg(target_os = "wasi")]
  let _ = start;
}

impl AnyKind for Lazy {}

impl View<ActiveDriver> for Lazy {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<ActiveDriver>::with(Scene::create_segment);

    (self.load)(start);

    start
  }
}
