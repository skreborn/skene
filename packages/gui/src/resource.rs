//! Asynchronously loaded data, readable as a signal.

use std::{future::Future, pin::Pin, rc::Rc};

use serde::{Serialize, de::DeserializeOwned};
use skene_reactive::{Get, Set, Signal, With};
use skene_web::next as driver;

use crate::spawn;

/// A resource's loader, boxed so it can be stored and re-run on [`reload`](Resource::reload).
type Loader<T> = Rc<dyn Fn() -> Pin<Box<dyn Future<Output = T>>>>;

/// Data loaded by a task, `None` until it lands.
///
/// The load starts on creation and is bound to the current scope, so it ends with the view
/// that asked for it. Reading tracks, so a view that reads a resource re-runs when the value
/// arrives—pair it with a [`derived`](crate::derived) subtree to swap a placeholder for
/// the loaded tree:
///
/// ```ignore
/// let user = Resource::new(|| async { load_user().await });
///
/// derived(move || match user.get() {
///   None => Text::new("Loading…").build(),
///   Some(user) => profile(user).build(),
/// })
/// ```
///
/// A resource loaded on the server crosses the boundary to the client and is adopted on
/// hydration rather than re-loaded—so [`new`](Self::new) requires the value to be
/// serializable. Data only a client can produce (a browser API, an unserializable value) uses
/// [`client`](Self::client) instead, which skips the server and loads on the client.
///
/// The loader is retained, so [`reload`](Self::reload) can re-run it. Failure is the loaded
/// type's business, not the resource's: load a `Result` and match on it.
#[must_use]
pub struct Resource<T: 'static> {
  value: Signal<Option<T>>,
  loader: Signal<Loader<T>>,
}

impl<T: 'static> Clone for Resource<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: 'static> Copy for Resource<T> {}

impl<T: 'static> Resource<T> {
  /// A resource with its value unloaded and `loader` retained for [`load`](Self::load).
  fn with_loader(loader: Loader<T>) -> Self {
    Self {
      value: Signal::new(None),
      loader: Signal::new(loader),
    }
  }

  /// Spawns the loader into the value signal, scope-bound like any task.
  fn load(&self) {
    let value = self.value;
    let future = (self.loader.get())();

    spawn(async move { value.set(Some(future.await)) });
  }

  /// Starts loading, returning a resource that is `None` until `load` completes.
  ///
  /// The value crosses the server-to-client boundary: on the server its settled value is
  /// serialized into the render, and on the client that value is adopted instead of loading
  /// again. Resources are matched across the boundary by creation order, so the client must
  /// create its resources in the same order the server did—which it does when it builds the
  /// same view. When no server value is present (a plain client render, or one the server did
  /// not settle), it loads normally. [`tagged`](Self::tagged) pairs the two sides
  /// explicitly instead.
  pub fn new<F>(load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
    T: Serialize + DeserializeOwned,
  {
    let resource = Self::with_loader(Rc::new(move || Box::pin(load())));

    if let Some(state) = driver::adopt_next()
      && let Ok(adopted) = serde_json::from_str::<T>(&state)
    {
      resource.value.set(Some(adopted));

      return resource;
    }

    resource.load();

    // On the server this registers the value for serialization once the render settles; on the
    // client it is a no-op.
    let value = resource.value;
    driver::register_source(Box::new(move || {
      value.with(|value| {
        value
          .as_ref()
          .and_then(|value| serde_json::to_string(value).ok())
      })
    }));

    resource
  }

  /// Starts loading, adopting and serializing by `tag` rather than creation order.
  ///
  /// Order-keyed matching (see [`new`](Self::new)) requires the client to create its
  /// resources in exactly the server's order; a tag makes the pairing explicit, so a build
  /// that diverges between server and client never adopts another resource's value. Tags
  /// must be unique among a render's tagged resources.
  ///
  /// # Panics
  ///
  /// A server render panics when two live resources register the same tag.
  pub fn tagged<F>(tag: &'static str, load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
    T: Serialize + DeserializeOwned,
  {
    let resource = Self::with_loader(Rc::new(move || Box::pin(load())));

    if let Some(state) = driver::adopt_tagged(tag)
      && let Ok(adopted) = serde_json::from_str::<T>(&state)
    {
      resource.value.set(Some(adopted));

      return resource;
    }

    resource.load();

    // On the server this registers the value for serialization once the render settles; on the
    // client it is a no-op.
    let value = resource.value;
    driver::register_tagged_source(
      tag,
      Box::new(move || {
        value.with(|value| {
          value
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok())
        })
      }),
    );

    resource
  }

  /// Starts loading a client-only resource: it neither serializes on the server nor adopts a
  /// server value, and runs only on the client.
  ///
  /// For data a server cannot produce (a browser API) or a value that does not serialize. It
  /// takes no part in hydration, so it does not shift the creation-order matching of the
  /// serializable resources around it, and it does not run during the server render.
  pub fn client<F>(load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
  {
    let resource = Self::with_loader(Rc::new(move || Box::pin(load())));

    if !driver::is_server() {
      resource.load();
    }

    resource
  }

  /// Re-runs the loader, clearing the value to `None` while it reloads.
  pub fn reload(&self) {
    self.value.set(None);

    self.load();
  }

  /// The loaded value, or `None` while it is still loading. Tracks.
  #[must_use]
  pub fn get(&self) -> Option<T>
  where
    T: Clone,
  {
    self.value.get()
  }

  /// Runs `apply` on the loaded value, or on `None` while it is still loading. Tracks, and
  /// needs no [`Clone`].
  pub fn with<R>(&self, apply: impl FnOnce(Option<&T>) -> R) -> R {
    self.value.with(|value| apply(value.as_ref()))
  }

  /// Whether the value has not landed yet. Tracks.
  #[must_use]
  pub fn is_loading(&self) -> bool {
    self.value.with(Option::is_none)
  }
}
