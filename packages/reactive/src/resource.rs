//! Asynchronously loaded data, readable as a signal.

use std::{fmt, future::Future, pin::Pin, rc::Rc};

use crate::{
  access::{Get, Set, Update, With},
  effect::untrack,
  owner::Owner,
  signal::{Signal, update_silently},
  spawn::{TaskHandle, spawn},
};

/// A resource's loader, re-run on [`reload`](Resource::reload).
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
  const fn value(&self) -> Option<&T> {
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
/// Every load, reloads included, runs on the executor the platform installed
/// ([`spawn`](crate::spawn)) under the owner current where the resource was created, so
/// it ends with the view that created the resource and with nothing else, and a reload
/// cancels the load it overtakes. Reading tracks, so a view that reads a resource
/// re-runs when the value arrives; a reload keeps the last value readable while the
/// next one is in flight. The loader is retained and read untracked, so a reload never
/// subscribes the reloading effect to what the loader reads.
///
/// Failure is the loaded type's business, not the resource's: load a `Result` and
/// match on it.
#[must_use]
pub struct Resource<T: 'static> {
  value: Signal<Load<T>>,
  loader: Signal<Loader<T>>,
  in_flight: Signal<Option<TaskHandle>>,
  owner: Owner,
}

impl<T: 'static> Clone for Resource<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: 'static> Copy for Resource<T> {}

impl<T: 'static> fmt::Debug for Resource<T> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Resource")
      .field("value", &self.value)
      .finish_non_exhaustive()
  }
}

impl<T: 'static> Resource<T> {
  fn with_loader<F>(load: impl Fn() -> F + 'static, value: Load<T>) -> Self
  where
    F: Future<Output = T> + 'static,
  {
    Self {
      value: Signal::transient(value),
      loader: Signal::transient(Rc::new(move || Box::pin(load()))),
      in_flight: Signal::transient(None),
      owner: Owner::current(),
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

  /// Spawns the loader into the value, cancelling the load in flight; does nothing
  /// once the resource's owner is gone.
  fn load(&self) {
    let Some(loader) = self.loader.try_get_untracked()
    else {
      return;
    };
    let overtaken = update_silently(self.in_flight.id(), Option::<TaskHandle>::take).flatten();

    if let Some(overtaken) = overtaken {
      overtaken.cancel();
    }

    let value = self.value;
    let task = self.owner.enter(|| {
      let future = untrack(|| loader());

      spawn(async move {
        value.set(Load::Ready(future.await));
      })
    });

    update_silently(self.in_flight.id(), |in_flight: &mut Option<TaskHandle>| {
      *in_flight = Some(task);
    });
  }

  /// Re-runs the loader, cancelling the load in flight; a value already landed stays
  /// readable, as [`Refetching`](LoadState::Refetching), until the new one lands. Does
  /// nothing once the resource's owner is gone.
  pub fn reload(&self) {
    let live = self.value.try_update(|load| {
      *load = match std::mem::replace(load, Load::Loading) {
        Load::Loading => Load::Loading,
        Load::Ready(value) | Load::Refetching(value) => Load::Refetching(value),
      };
    });

    if live.is_some() {
      self.load();
    }
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
