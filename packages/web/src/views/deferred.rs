//! A streaming boundary: a subtree the server can flush around and stream in place.

use skene_foundation::{ChildBuilder, NoChild, View, Wraps, web::DeferredBase};

use crate::ActivePlatform;

/// A subtree whose async work the server settles apart from the shell's and streams
/// in place once it does.
///
/// Its content builds in place, between anchors of its own. On the server its async
/// work reports to a boundary of its own rather than the shell's, so a streaming render
/// emits everything up to the boundary, blocks until that work settles, then streams
/// the content in document order; a whole render settles through it and serializes it
/// in place. On the client the content builds directly, and hydration adopts the
/// server's already-settled markup. A streamed page's head flushes with the shell, so a
/// head element registered inside the subtree reaches that page only once the client
/// hydrates. Holds [`NoChild`] until [`set_child_builder`](Self::set_child_builder)
/// supplies the content.
///
/// ```
/// use skene_web::{Deferred, prelude::*};
///
/// let comments = Deferred::new().set_child_builder(|| Text::new("The comments"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Deferred<C = NoChild>(skene_foundation::web::Deferred<ActivePlatform, C>);

impl DeferredBase<ActivePlatform> for Deferred {}

impl Deferred {
  /// Sets the content, built by `content`: its async work reports to a boundary of its
  /// own, and a streaming render flushes the surrounding page before it settles.
  #[inline(always)]
  pub fn set_child_builder<V: View<ActivePlatform>>(
    self,
    content: impl FnOnce() -> V + 'static,
  ) -> Deferred<ChildBuilder<ActivePlatform>> {
    Deferred(self.0.set_child_builder(content))
  }
}
