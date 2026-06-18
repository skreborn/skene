use std::{
  mem::transmute,
  ops::{Deref, DerefMut},
  rc::Rc,
  sync::Arc,
};

use parking_lot::{Mutex, MutexGuard};

use crate::{
  id::Id,
  signal::{TrackedRefCell, TrackedRefMut},
};

/// A mutable borrow guard over a thread-safe signal's value.
pub struct SyncWriteRef<'a, T> {
  id: Id,
  _handle: Arc<Mutex<T>>,
  pub(crate) guard: Option<MutexGuard<'a, T>>,
}

/// A mutable borrow guard over a thread-local signal's value.
pub struct LocalWriteRef<'a, T> {
  id: Id,
  _handle: Rc<TrackedRefCell<T>>,
  pub(crate) guard: Option<TrackedRefMut<'a, T>>,
}

/// A mutable borrow guard over a signal's value, thread-safe or thread-local.
pub enum WriteRef<'a, T> {
  /// A guard over a thread-safe signal.
  Sync(SyncWriteRef<'a, T>),
  /// A guard over a thread-local signal.
  Local(LocalWriteRef<'a, T>),
}

impl<'a, T> SyncWriteRef<'a, T> {
  pub(crate) fn new(id: Id, handle: Arc<Mutex<T>>) -> Self {
    let guard = handle.lock();

    // SAFETY: the retained `Arc` keeps the data alive for the guard's lifetime.
    let guard = unsafe { transmute::<MutexGuard<'_, T>, MutexGuard<'a, T>>(guard) };

    Self {
      id,
      _handle: handle,
      guard: Some(guard),
    }
  }
}

impl<'a, T> Drop for SyncWriteRef<'a, T> {
  fn drop(&mut self) {
    if let Some(guard) = self.guard.take() {
      drop(guard);
    }

    if let Some(signal) = self.id.signal() {
      signal.run_effects();
    }
  }
}

impl<'a, T> LocalWriteRef<'a, T> {
  pub(crate) fn new(id: Id, handle: Rc<TrackedRefCell<T>>) -> Self {
    let guard = handle.borrow_mut();

    // SAFETY: the retained `Rc` keeps the data alive for the guard's lifetime.
    let guard = unsafe { transmute::<TrackedRefMut<'_, T>, TrackedRefMut<'a, T>>(guard) };

    Self {
      id,
      _handle: handle,
      guard: Some(guard),
    }
  }
}

impl<'a, T> Drop for LocalWriteRef<'a, T> {
  fn drop(&mut self) {
    if let Some(guard) = self.guard.take() {
      drop(guard);
    }

    if let Some(signal) = self.id.signal() {
      signal.run_effects();
    }
  }
}

impl<'a, T> Deref for WriteRef<'a, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    match self {
      WriteRef::Sync(v) => v.guard.as_ref().expect("guard present"),
      WriteRef::Local(v) => v.guard.as_ref().expect("guard present"),
    }
  }
}

impl<'a, T> DerefMut for WriteRef<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    match self {
      WriteRef::Sync(v) => &mut *v.guard.as_mut().expect("guard present"),
      WriteRef::Local(v) => &mut *v.guard.as_mut().expect("guard present"),
    }
  }
}

/// Updates a signal's value in place, re-running its subscribers.
pub trait SignalUpdate<T> {
  /// The signal's `Id`.
  fn id(&self) -> Id;

  /// Replaces the value and re-runs subscribers.
  fn set(&self, new_value: T)
  where
    T: 'static,
  {
    let _ = self.try_update(|v| *v = new_value);
  }

  /// Mutates the value through `f` and re-runs subscribers.
  fn update(&self, f: impl FnOnce(&mut T))
  where
    T: 'static,
  {
    let _ = self.try_update(f);
  }

  /// Mutates the value through `f` if the signal is live, re-runs subscribers, and
  /// returns `f`'s result.
  fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
  where
    T: 'static;
}

/// Mutably borrows a signal's value through a guard.
pub trait SignalWrite<T> {
  /// The signal's `Id`.
  fn id(&self) -> Id;

  /// Mutably borrows the signal value, re-running subscribers when the guard is dropped.
  fn write(&self) -> WriteRef<'_, T>
  where
    T: 'static,
  {
    self.try_write().unwrap()
  }

  /// Mutably borrows the signal value if the signal is live.
  fn try_write(&self) -> Option<WriteRef<'_, T>>
  where
    T: 'static;
}
