//! Signals whose lifetime is the handle itself, apart from the owner tree.

use crate::{
  id::Id,
  signal::{ReadSignal, Signal, SignalState, WriteSignal},
  storage::SyncStorage,
};

/// A signal owned by this handle: unowned by any [`Owner`](crate::Owner), disposed
/// when the handle drops.
#[must_use]
pub struct OwnedSignal<T: 'static> {
  id: Id,
  ty: std::marker::PhantomData<T>,
}

impl<T: 'static> Eq for OwnedSignal<T> {}

impl<T: 'static> PartialEq for OwnedSignal<T> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T: 'static> Drop for OwnedSignal<T> {
  fn drop(&mut self) {
    self.id.dispose();
  }
}

impl<T: 'static> OwnedSignal<T> {
  /// Creates a signal holding `value`, disposed when this handle drops.
  pub fn new(value: T) -> Self {
    Self {
      id: SignalState::create(value),
      ty: std::marker::PhantomData,
    }
  }

  /// A read-write handle to this signal.
  #[inline(always)]
  pub const fn signal(&self) -> Signal<T> {
    Signal::from_id(self.id)
  }

  /// The read half of this signal.
  #[inline(always)]
  pub const fn read_only(&self) -> ReadSignal<T> {
    ReadSignal::from_id(self.id)
  }

  /// The write half of this signal.
  #[inline(always)]
  pub const fn write_only(&self) -> WriteSignal<T> {
    WriteSignal::from_id(self.id)
  }
}

/// The thread-safe [`OwnedSignal`]: the signal lives in the sync registry.
#[must_use]
pub struct SyncOwnedSignal<T: Send + Sync + 'static> {
  id: Id,
  ty: std::marker::PhantomData<T>,
}

impl<T: Send + Sync + 'static> Eq for SyncOwnedSignal<T> {}

impl<T: Send + Sync + 'static> PartialEq for SyncOwnedSignal<T> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T: Send + Sync + 'static> Drop for SyncOwnedSignal<T> {
  fn drop(&mut self) {
    self.id.dispose();
  }
}

impl<T: Send + Sync + 'static> SyncOwnedSignal<T> {
  /// Creates a thread-safe signal holding `value`, disposed when this handle drops.
  pub fn new(value: T) -> Self {
    Self {
      id: SignalState::create_sync(value),
      ty: std::marker::PhantomData,
    }
  }

  /// A read-write handle to this signal.
  #[inline(always)]
  pub const fn signal(&self) -> Signal<T, SyncStorage> {
    Signal::from_id(self.id)
  }

  /// The read half of this signal.
  #[inline(always)]
  pub const fn read_only(&self) -> ReadSignal<T, SyncStorage> {
    ReadSignal::from_id(self.id)
  }

  /// The write half of this signal.
  #[inline(always)]
  pub const fn write_only(&self) -> WriteSignal<T, SyncStorage> {
    WriteSignal::from_id(self.id)
  }
}
