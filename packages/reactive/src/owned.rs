//! Signals whose lifetime is the handle itself, apart from the owner tree.

use std::marker::PhantomData;

use crate::{
  id::Id,
  signal::{ReadSignal, Signal, SignalState, WriteSignal},
};

/// A signal owned by this handle: unowned by any [`Owner`](crate::Owner), disposed
/// when the handle drops.
#[must_use]
pub struct OwnedSignal<T: 'static> {
  id: Id,
  ty: PhantomData<T>,
  local: PhantomData<*const ()>,
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
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(value: T) -> Self {
    Self {
      id: SignalState::create(value),
      ty: PhantomData,
      local: PhantomData,
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
