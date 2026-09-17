//! Asynchronously loaded data, readable as a signal.

use std::{future::Future, pin::Pin, rc::Rc};

use crate::{
  access::{Get, Set, Update, With},
  effect::untrack,
  signal::{Signal, update_silently},
  spawn::spawn,
};

/// A resource's loader, boxed so it can be stored and re-run on
/// [`reload`](Resource::reload).
type Loader<T> = Rc<dyn Fn() -> Pin<Box<dyn Future<Output = T>>>>;

/// Where a resource's load stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum LoadState {
  /// No value has landed yet.
  Loading,
  /// The last load landed and nothing is in flight.
  Ready,
  /// A reload is in flight; the last value stays readable meanwhile.
  Refetching,
}

/// The value and its state, as the resource's signal holds them.
enum Load<T> {
  Loading,
  Ready(T),
  Refetching(T),
}

impl<T> Load<T> {
  fn value(&self) -> Option<&T> {
    match self {
      Self::Loading => None,
      Self::Ready(value) | Self::Refetching(value) => Some(value),
    }
  }

  const fn state(&self) -> LoadState {
    match self {
      Self::Loading => LoadState::Loading,
      Self::Ready(_) => LoadState::Ready,
      Self::Refetching(_) => LoadState::Refetching,
    }
  }
}

/// Data loaded by a task, readable as a signal.
///
/// The load runs on the executor the platform installed ([`spawn`](crate::spawn)), so it
/// is bound to the current owner and ends with the view that asked for it. Reading
/// tracks, so a view that reads a resource re-runs when the value arrives; a reload
/// keeps the last value readable while the next one is in flight. The loader is
/// retained and read untracked, so a reload never subscribes the reloading effect to
/// what the loader reads.
///
/// Failure is the loaded type's business, not the resource's: load a `Result` and
/// match on it.
#[must_use]
pub struct Resource<T: 'static> {
  value: Signal<Load<T>>,
  loader: Signal<Loader<T>>,
  generation: Signal<u64>,
}

impl<T: 'static> Clone for Resource<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: 'static> Copy for Resource<T> {}

impl<T: 'static> Resource<T> {
  fn with_loader<F>(load: impl Fn() -> F + 'static, value: Load<T>) -> Self
  where
    F: Future<Output = T> + 'static,
  {
    Self {
      value: Signal::opaque(value),
      loader: Signal::opaque(Rc::new(move || Box::pin(load()))),
      generation: Signal::opaque(0),
    }
  }

  /// Starts loading, returning a resource that is [`Loading`](LoadState::Loading)
  /// until `load` completes.
  pub fn new<F>(load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
  {
    let resource = Self::with_loader(load, Load::Loading);

    resource.load();

    resource
  }

  /// A resource already [`Ready`](LoadState::Ready) with `value`; `load` runs only on
  /// a later [`reload`](Self::reload).
  ///
  /// For a value produced elsewhere—a server's render adopted by the client.
  pub fn ready<F>(value: T, load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
  {
    Self::with_loader(load, Load::Ready(value))
  }

  /// A resource that stays [`Loading`](LoadState::Loading) until a
  /// [`reload`](Self::reload) runs `load` for the first time.
  ///
  /// For a load that must not run where the resource is created—a client-only load
  /// during a server render.
  pub fn unloaded<F>(load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
  {
    Self::with_loader(load, Load::Loading)
  }

  /// Spawns the loader into the value; a load overtaken by a later one lands nowhere.
  fn load(&self) {
    let value = self.value;
    let generation = self.generation;
    let current = update_silently(generation.id(), |generation: &mut u64| {
      *generation += 1;

      *generation
    });
    let Some(current) = current
    else {
      return;
    };
    let future = untrack(|| (self.loader.get_untracked())());

    spawn(async move {
      let loaded = future.await;

      if generation.try_get_untracked() == Some(current) {
        value.set(Load::Ready(loaded));
      }
    });
  }

  /// Re-runs the loader; a value already landed stays readable, as
  /// [`Refetching`](LoadState::Refetching), until the new one lands.
  pub fn reload(&self) {
    self.value.update(|load| {
      *load = match std::mem::replace(load, Load::Loading) {
        Load::Loading => Load::Loading,
        Load::Ready(value) | Load::Refetching(value) => Load::Refetching(value),
      };
    });

    self.load();
  }

  /// The loaded value, or `None` while nothing has landed. Tracks.
  #[must_use]
  pub fn get(&self) -> Option<T>
  where
    T: Clone,
  {
    self.value.with(|load| load.value().cloned())
  }

  /// Runs `apply` on the loaded value, or on `None` while nothing has landed. Tracks,
  /// and needs no [`Clone`].
  pub fn with<R>(&self, apply: impl FnOnce(Option<&T>) -> R) -> R {
    self.value.with(|load| apply(load.value()))
  }

  /// Where the load stands. Tracks.
  pub fn state(&self) -> LoadState {
    self.value.with(Load::state)
  }

  /// Whether nothing has landed yet. Tracks.
  #[must_use]
  pub fn is_loading(&self) -> bool {
    self.state() == LoadState::Loading
  }

  /// Whether a reload is in flight behind a readable value. Tracks.
  #[must_use]
  pub fn is_refetching(&self) -> bool {
    self.state() == LoadState::Refetching
  }
}
