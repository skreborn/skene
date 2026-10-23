//! A lazily loaded subtree: content from an async source.

use skene_foundation::{View, Wraps, web::AsyncViewBase};

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
/// the module arrives and the real content replaces it in place. Without the content
/// [`set_child_loader`](Self::set_child_loader) supplies, it renders nothing.
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
pub struct AsyncView(skene_foundation::web::AsyncView<Active>);

impl AsyncViewBase<Active> for AsyncView {}
