//! Asynchronously loaded data that crosses the server-to-client boundary.

use std::{future::Future, ops::Deref};

use serde::{Serialize, de::DeserializeOwned};
pub use skene_reactive::LoadState;
#[cfg(not(skene_leaf = "host"))]
use skene_web as platform;

use crate::task::install_executor;

/// A [`skene_reactive::Resource`] that hydrates: a value loaded on the server crosses
/// the boundary to the client and is adopted there rather than re-loaded.
///
/// Every read—`get`, `with`, `state`, `reload`—is the engine resource's, reached
/// through `Deref`. The constructors differ only in how the two sides pair:
/// [`new`](Self::new) by creation order, [`tagged`](Self::tagged) by tag, and
/// [`client`](Self::client) not at all.
///
/// ```ignore
/// let user = Resource::new(|| async { load_user().await });
///
/// derived(move || match user.get() {
///   None => Text::new("Loading…").build(),
///   Some(user) => profile(user).build(),
/// })
/// ```
#[derive(Debug)]
#[must_use]
pub struct Resource<T: 'static>(skene_reactive::Resource<T>);

impl<T: 'static> Clone for Resource<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: 'static> Copy for Resource<T> {}

impl<T: 'static> Deref for Resource<T> {
  type Target = skene_reactive::Resource<T>;

  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: 'static> Resource<T> {
  /// Starts loading, returning a resource that is loading until `load` completes.
  ///
  /// The value crosses the server-to-client boundary: on the server its settled value
  /// is serialized into the render, and on the client that value is adopted instead of
  /// loading again. Resources are matched across the boundary by creation order within
  /// the lazy segment they are built in, so the client must create its resources in
  /// the same order the server did—which it does when it builds the same view, and a
  /// [`AsyncView`](crate::AsyncView) content's resources are matched among themselves however
  /// late its module arrives. When no server value is present (a plain client render,
  /// or one the server did not settle), it loads normally.
  /// [`tagged`](Self::tagged) pairs the two sides explicitly instead.
  pub fn new<F>(load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
    T: Serialize + DeserializeOwned,
  {
    install_executor();

    #[cfg(not(skene_leaf = "host"))]
    {
      if let Some(state) = platform::adopt_next()
        && let Ok(adopted) = serde_json::from_str::<T>(&state)
      {
        return Self(skene_reactive::Resource::ready(adopted, load));
      }

      let resource = Self(skene_reactive::Resource::new(load));

      platform::register_source(resource.source());

      resource
    }

    #[cfg(skene_leaf = "host")]
    Self(skene_reactive::Resource::new(load))
  }

  /// Starts loading, adopting and serializing by `tag` rather than creation order.
  ///
  /// Order-keyed matching (see [`new`](Self::new)) requires the client to create its
  /// resources in exactly the server's order; a tag makes the pairing explicit, so a
  /// build that diverges between server and client never adopts another resource's
  /// value. Tags must be unique among a render's tagged resources.
  ///
  /// # Panics
  ///
  /// A server render panics when two live resources register the same tag.
  pub fn tagged<F>(tag: &'static str, load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
    T: Serialize + DeserializeOwned,
  {
    install_executor();

    #[cfg(not(skene_leaf = "host"))]
    {
      if let Some(state) = platform::adopt_tagged(tag)
        && let Ok(adopted) = serde_json::from_str::<T>(&state)
      {
        return Self(skene_reactive::Resource::ready(adopted, load));
      }

      let resource = Self(skene_reactive::Resource::new(load));

      platform::register_tagged_source(tag, resource.source());

      resource
    }

    #[cfg(skene_leaf = "host")]
    {
      let _ = tag;

      Self(skene_reactive::Resource::new(load))
    }
  }

  /// Starts loading a client-only resource: it neither serializes on the server nor
  /// adopts a server value, and runs only on the client.
  ///
  /// For data a server cannot produce (a browser API) or a value that does not
  /// serialize. It takes no part in hydration, so it does not shift the creation-order
  /// matching of the serializable resources around it, and it does not run during the
  /// server render.
  pub fn client<F>(load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
  {
    install_executor();

    #[cfg(not(skene_leaf = "host"))]
    if platform::is_server() {
      return Self(skene_reactive::Resource::unloaded(load));
    }

    Self(skene_reactive::Resource::new(load))
  }

  /// The serializer a server render drains once this resource settles.
  #[cfg(not(skene_leaf = "host"))]
  fn source(&self) -> platform::HydrationSource
  where
    T: Serialize,
  {
    let resource = self.0;

    Box::new(move || {
      resource.with(|value| value.and_then(|value| serde_json::to_string(value).ok()))
    })
  }
}
