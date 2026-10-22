//! A lazily loaded subtree: content from an async source.

use std::future::Future;

use skene_foundation::{
  NoChild,
  View,
  Wraps,
  web::{AsyncViewBase, ChildLoader},
};

use crate::Active;

/// A subtree whose content arrives asynchronously—typically a [`macro@lazy`](crate::lazy)
/// function whose body loads in a separately fetched wasm module.
///
/// The content builds and mounts between the view's anchors once the future resolves.
/// Until then the view counts as pending toward the enclosing boundary, so a wrapping
/// [`Suspense`](crate::Suspense) shows its fallback, a [`Deferred`](crate::Deferred)
/// settles apart from it, and a server render settles through it to the content.
///
/// On a hydrated page whose server render included the content, the server markup is
/// claimed instead: it stays visible, already satisfying the enclosing boundary, until
/// the module arrives and the real content replaces it in place. Holds [`NoChild`]
/// until [`set_child_loader`](Self::set_child_loader) supplies the content.
///
/// ```
/// use skene_web::{AsyncView, prelude::*};
///
/// async fn settings() -> Text {
///   Text::new("Settings")
/// }
///
/// let page = AsyncView::new().set_child_loader(settings);
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct AsyncView<C = NoChild>(skene_foundation::web::AsyncView<Active, C>);

impl AsyncViewBase<Active> for AsyncView {}

impl AsyncView {
  /// Sets the content, loaded by `load`, run to completion as a task.
  #[inline(always)]
  pub fn set_child_loader<V, F>(
    self,
    load: impl FnOnce() -> F + 'static,
  ) -> AsyncView<ChildLoader<Active>>
  where
    V: View<Active>,
    F: Future<Output = V> + 'static,
  {
    AsyncView(self.0.set_child_loader(load))
  }
}
