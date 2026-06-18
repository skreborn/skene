use std::{mem::transmute, ops::Deref, rc::Rc, sync::Arc};

use parking_lot::{Mutex, MutexGuard};

use crate::{
  id::Id,
  signal::{TrackedRef, TrackedRefCell},
};

/// A read guard over a thread-safe signal's value.
#[must_use]
pub struct SyncReadRef<'a, T> {
  _handle: Arc<Mutex<T>>,
  pub(crate) guard: MutexGuard<'a, T>,
}

/// A read guard over a thread-local signal's value.
#[must_use]
pub struct LocalReadRef<'a, T> {
  _handle: Rc<TrackedRefCell<T>>,
  pub(crate) guard: TrackedRef<'a, T>,
}

/// A read guard over a signal's value, thread-safe or thread-local.
#[must_use]
pub enum ReadRef<'a, T> {
  /// A guard over a thread-safe signal.
  Sync(SyncReadRef<'a, T>),
  /// A guard over a thread-local signal.
  Local(LocalReadRef<'a, T>),
}

impl<'a, T> Deref for SyncReadRef<'a, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<'a, T> Deref for LocalReadRef<'a, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<'a, T> Deref for ReadRef<'a, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    match self {
      ReadRef::Sync(v) => &v.guard,
      ReadRef::Local(v) => &v.guard,
    }
  }
}

impl<'a, T> SyncReadRef<'a, T> {
  pub(crate) fn new(handle: Arc<Mutex<T>>) -> Self {
    let guard = handle.lock();

    // SAFETY: the retained `Arc` keeps the data alive for the guard's lifetime.
    let guard = unsafe { transmute::<MutexGuard<'_, T>, MutexGuard<'a, T>>(guard) };

    Self {
      _handle: handle,
      guard,
    }
  }
}

impl<'a, T> LocalReadRef<'a, T> {
  pub(crate) fn new(handle: Rc<TrackedRefCell<T>>) -> Self {
    let guard = handle.borrow();

    // SAFETY: the retained `Rc` keeps the data alive for the guard's lifetime.
    let guard = unsafe { transmute::<TrackedRef<'_, T>, TrackedRef<'a, T>>(guard) };

    Self {
      _handle: handle,
      guard,
    }
  }
}

/// Reads a signal's value by cloning it.
pub trait SignalGet<T: Clone> {
  /// The signal's `Id`.
  fn id(&self) -> Id;

  /// Clones the current value without subscribing the running effect.
  fn get_untracked(&self) -> T
  where
    T: 'static,
  {
    self.try_get_untracked().unwrap()
  }

  /// Clones the current value, subscribing the running effect.
  fn get(&self) -> T
  where
    T: 'static,
  {
    self.try_get().unwrap()
  }

  /// Clones the current value if the signal is live, subscribing the running effect.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_get(&self) -> Option<T>
  where
    T: 'static,
  {
    self.id().signal().map(|signal| signal.get())
  }

  /// Clones the current value if the signal is live, without subscribing.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_get_untracked(&self) -> Option<T>
  where
    T: 'static,
  {
    self.id().signal().map(|signal| signal.get_untracked())
  }
}

/// Subscribes the running effect to a signal without reading its value.
pub trait SignalTrack<T> {
  /// The signal's `Id`.
  fn id(&self) -> Id;

  /// Subscribes the running effect to the signal.
  #[cfg_attr(debug_assertions, track_caller)]
  fn track(&self) {
    let signal = self.id().signal().unwrap();

    signal.subscribe();
  }

  /// Subscribes the running effect to the signal if it is live.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_track(&self) {
    if let Some(signal) = self.id().signal() {
      signal.subscribe();
    }
  }
}

/// Reads a signal's value by reference.
pub trait SignalWith<T> {
  /// The signal's `Id`.
  fn id(&self) -> Id;

  /// Passes the current value to `f`, subscribing the running effect.
  #[cfg_attr(debug_assertions, track_caller)]
  fn with<O>(&self, f: impl FnOnce(&T) -> O) -> O
  where
    T: 'static,
  {
    let signal = self.id().signal().unwrap();

    signal.with(f)
  }

  /// Passes the current value to `f` without subscribing.
  #[cfg_attr(debug_assertions, track_caller)]
  fn with_untracked<O>(&self, f: impl FnOnce(&T) -> O) -> O
  where
    T: 'static,
  {
    let signal = self.id().signal().unwrap();

    signal.with_untracked(f)
  }

  /// Passes the current value to `f` (`None` if the signal is disposed),
  /// subscribing the running effect.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_with<O>(&self, f: impl FnOnce(Option<&T>) -> O) -> O
  where
    T: 'static,
  {
    if let Some(signal) = self.id().signal() {
      signal.with(|v| f(Some(v)))
    }
    else {
      f(None)
    }
  }

  /// Passes the current value to `f` (`None` if the signal is disposed), without
  /// subscribing.
  fn try_with_untracked<O>(&self, f: impl FnOnce(Option<&T>) -> O) -> O
  where
    T: 'static,
  {
    if let Some(signal) = self.id().signal() {
      signal.with_untracked(|v| f(Some(v)))
    }
    else {
      f(None)
    }
  }
}

/// Reads a signal's value through a guard, without cloning.
pub trait SignalRead<T> {
  /// The signal's `Id`.
  fn id(&self) -> Id;

  /// Reads the data stored in the signal, subscribing the current running effect.
  fn read(&self) -> ReadRef<'_, T>
  where
    T: 'static,
  {
    self.try_read().unwrap()
  }

  /// Reads the data stored in the signal without subscribing.
  fn read_untracked(&self) -> ReadRef<'_, T>
  where
    T: 'static,
  {
    self.try_read_untracked().unwrap()
  }

  /// Reads the data stored in the signal if it is live, subscribing the current
  /// running effect.
  fn try_read(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static;

  /// Reads the data stored in the signal if it is live, without subscribing.
  fn try_read_untracked(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static;
}
