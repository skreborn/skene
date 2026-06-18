use std::marker::PhantomData;

use crate::{
  ReadSignal,
  RwSignal,
  SignalGet,
  SignalUpdate,
  SignalWith,
  WriteSignal,
  id::Id,
  signal::SignalState,
  storage::SyncStorage,
};

/// Controls the lifetime of a signal directly, apart from any `Scope`. This
/// unsync variant stores its value on the UI thread.
///
/// The signal is placed in no scope; it lives until the `BaseSignal` is dropped,
/// which disposes the underlying signal.
#[must_use]
pub struct BaseSignal<T: 'static> {
  id: Id,
  ty: PhantomData<T>,
}

impl<T: 'static> Eq for BaseSignal<T> {}

impl<T: 'static> PartialEq for BaseSignal<T> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T: 'static> Drop for BaseSignal<T> {
  fn drop(&mut self) {
    self.id.dispose();
  }
}

impl<T: 'static> BaseSignal<T> {
  /// Creates a base signal holding `value`, disposed when it is dropped.
  pub fn new(value: T) -> Self {
    let id = SignalState::new(value);

    BaseSignal {
      id,
      ty: PhantomData,
    }
  }

  /// Creates a `RwSignal` of this signal.
  #[inline(always)]
  pub fn rw(&self) -> RwSignal<T> {
    RwSignal {
      id: self.id,
      ty: PhantomData,
      st: PhantomData,
    }
  }

  /// Creates a read-only `ReadSignal` of this signal.
  #[inline(always)]
  pub fn read_only(&self) -> ReadSignal<T> {
    ReadSignal {
      id: self.id,
      ty: PhantomData,
      st: PhantomData,
    }
  }

  /// Creates a write-only `WriteSignal` of this signal.
  #[inline(always)]
  pub fn write_only(&self) -> WriteSignal<T> {
    WriteSignal {
      id: self.id,
      ty: PhantomData,
      st: PhantomData,
    }
  }
}

impl<T: Clone + 'static> SignalGet<T> for BaseSignal<T> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: Clone + 'static> SignalWith<T> for BaseSignal<T> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: 'static> SignalUpdate<T> for BaseSignal<T> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn set(&self, new_value: T)
  where
    T: 'static,
  {
    self.write_only().set(new_value)
  }

  fn update(&self, f: impl FnOnce(&mut T))
  where
    T: 'static,
  {
    self.write_only().update(f)
  }

  fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
  where
    T: 'static,
  {
    self.write_only().try_update(f)
  }
}

/// Thread-safe variant that stores the signal in the sync runtime.
#[must_use]
pub struct SyncBaseSignal<T: Send + Sync + 'static> {
  id: Id,
  ty: PhantomData<T>,
}

impl<T: Send + Sync + 'static> Eq for SyncBaseSignal<T> {}

impl<T: Send + Sync + 'static> PartialEq for SyncBaseSignal<T> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T: Send + Sync + 'static> Drop for SyncBaseSignal<T> {
  fn drop(&mut self) {
    self.id.dispose();
  }
}

impl<T: Send + Sync + 'static> SyncBaseSignal<T> {
  /// Creates a thread-safe base signal holding `value`, disposed when it is dropped.
  pub fn new(value: T) -> Self {
    let id = SignalState::new_sync(value);

    SyncBaseSignal {
      id,
      ty: PhantomData,
    }
  }

  /// Creates a `RwSignal` of this signal.
  #[inline(always)]
  pub fn rw(&self) -> RwSignal<T, SyncStorage> {
    RwSignal {
      id: self.id,
      ty: PhantomData,
      st: PhantomData,
    }
  }

  /// Creates a read-only `ReadSignal` of this signal.
  #[inline(always)]
  pub fn read_only(&self) -> ReadSignal<T, SyncStorage> {
    ReadSignal {
      id: self.id,
      ty: PhantomData,
      st: PhantomData,
    }
  }

  /// Creates a write-only `WriteSignal` of this signal.
  #[inline(always)]
  pub fn write_only(&self) -> WriteSignal<T, SyncStorage> {
    WriteSignal {
      id: self.id,
      ty: PhantomData,
      st: PhantomData,
    }
  }
}

impl<T: Clone + Send + Sync + 'static> SignalGet<T> for SyncBaseSignal<T> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: Clone + Send + Sync + 'static> SignalWith<T> for SyncBaseSignal<T> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: Send + Sync + 'static> SignalUpdate<T> for SyncBaseSignal<T> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn set(&self, new_value: T)
  where
    T: 'static,
  {
    self.write_only().set(new_value)
  }

  fn update(&self, f: impl FnOnce(&mut T))
  where
    T: 'static,
  {
    self.write_only().update(f)
  }

  fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
  where
    T: 'static,
  {
    self.write_only().try_update(f)
  }
}
