//! Asynchronously loaded data that crosses the server-to-client boundary.

use std::{future::Future, marker::PhantomData, ops::Deref};

use serde::{Serialize, de::DeserializeOwned};

use super::{HydrationSource, WebHydration};

/// A [`skene_reactive::Resource`] that hydrates: a value loaded on the server crosses
/// the boundary to the client and is adopted there rather than re-loaded.
///
/// Every read—`get`, `with`, `state`, `reload`—is the engine resource's, reached
/// through `Deref`. The constructors differ only in how the two sides pair:
/// [`new`](Self::new) by creation order, [`tagged`](Self::tagged) by tag, and
/// [`client`](Self::client) not at all.
#[must_use]
pub struct Resource<P: WebHydration, T: 'static> {
  inner: skene_reactive::Resource<T>,
  platform: PhantomData<fn() -> P>,
}

impl<P: WebHydration, T: 'static> std::fmt::Debug for Resource<P, T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Resource").finish_non_exhaustive()
  }
}

impl<P: WebHydration, T: 'static> Clone for Resource<P, T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<P: WebHydration, T: 'static> Copy for Resource<P, T> {}

impl<P: WebHydration, T: 'static> Deref for Resource<P, T> {
  type Target = skene_reactive::Resource<T>;

  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<P: WebHydration, T: 'static> Resource<P, T> {
  /// Holds `inner`.
  #[inline(always)]
  const fn hold(inner: skene_reactive::Resource<T>) -> Self {
    Self {
      inner,
      platform: PhantomData,
    }
  }

  /// Starts loading, returning a resource that is loading until `load` completes.
  ///
  /// The value crosses the server-to-client boundary: on the server its settled value
  /// is serialized into the render, and on the client that value is adopted instead of
  /// loading again. Resources are matched across the boundary by creation order within
  /// the lazy segment they are built in, so the client must create its resources in
  /// the same order the server did—which it does when it builds the same view, and an
  /// [`AsyncView`](super::AsyncView) content's resources are matched among themselves
  /// however late its module arrives. When no server value is present (a plain client
  /// render, or one the server did not settle), it loads normally.
  /// [`tagged`](Self::tagged) pairs the two sides explicitly instead.
  pub fn new<F>(load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
    T: Serialize + DeserializeOwned,
  {
    P::install_executor();

    if let Some(state) = P::adopt_next()
      && let Ok(adopted) = serde_json::from_str::<T>(&state)
    {
      return Self::hold(skene_reactive::Resource::ready(adopted, load));
    }

    let resource = Self::hold(skene_reactive::Resource::new(load));

    P::register_source(resource.source());

    resource
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
    P::install_executor();

    if let Some(state) = P::adopt_tagged(tag)
      && let Ok(adopted) = serde_json::from_str::<T>(&state)
    {
      return Self::hold(skene_reactive::Resource::ready(adopted, load));
    }

    let resource = Self::hold(skene_reactive::Resource::new(load));

    P::register_tagged_source(tag, resource.source());

    resource
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
    P::install_executor();

    if P::is_server() {
      return Self::hold(skene_reactive::Resource::unloaded(load));
    }

    Self::hold(skene_reactive::Resource::new(load))
  }

  /// The serializer a server render drains once this resource settles.
  fn source(&self) -> HydrationSource
  where
    T: Serialize,
  {
    let resource = self.inner;

    Box::new(move || {
      resource.with(|value| value.and_then(|value| serde_json::to_string(value).ok()))
    })
  }
}
