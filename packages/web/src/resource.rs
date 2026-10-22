//! Asynchronously loaded data that crosses the server-to-client boundary.

pub use skene_reactive::LoadState;

use crate::Active;

/// A [`skene_reactive::Resource`] that hydrates: a value loaded on the server crosses
/// the boundary to the client and is adopted there rather than re-loaded.
///
/// ```ignore
/// let user = Resource::new(|| async { load_user().await });
///
/// derived(move || match user.get() {
///   None => Text::new("Loading…").build(),
///   Some(user) => profile(user).build(),
/// })
/// ```
pub type Resource<T> = skene_foundation::web::Resource<Active, T>;
