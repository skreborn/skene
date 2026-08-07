//! Two-way projections over one signal.

use std::marker::PhantomData;

use crate::{
  access::{Get, Set, Track, Update, With},
  id::Id,
  signal::Signal,
  storage::{LocalStorage, SyncStorage},
};

/// A thread-safe [`Slice`].
pub type SyncSlice<T, O, GF, UF> = Slice<T, O, GF, UF, SyncStorage>;

/// A read-write projection over part of a signal's value.
///
/// Reads go through `read`, writes map the projected value back through `write` and
/// store it into the underlying signal—notifying its subscribers as any write would.
/// The projection closures are held in signals of their own, keeping the handle `Copy`.
#[must_use]
pub struct Slice<T: 'static, O, GF, UF, S = LocalStorage>
where
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  signal: Signal<T, S>,
  read: Signal<GF, S>,
  write: Signal<UF, S>,
  ty: PhantomData<O>,
}

impl<T, O, GF, UF, S> Clone for Slice<T, O, GF, UF, S>
where
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn clone(&self) -> Self {
    *self
  }
}

impl<T, O, GF, UF, S> Copy for Slice<T, O, GF, UF, S>
where
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
}

impl<T, O, GF, UF> Slice<T, O, GF, UF>
where
  T: 'static,
  O: 'static,
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  /// Creates a slice over `signal`, reading through `read` and writing back through
  /// `write`.
  pub fn new(signal: Signal<T>, read: GF, write: UF) -> Self {
    Self {
      signal,
      read: Signal::new(read),
      write: Signal::new(write),
      ty: PhantomData,
    }
  }
}

impl<T, O, GF, UF> SyncSlice<T, O, GF, UF>
where
  T: Send + Sync + 'static,
  O: 'static,
  GF: Fn(&T) -> O + Clone + Send + Sync + 'static,
  UF: Fn(&mut T, &O) + Clone + Send + Sync + 'static,
{
  /// Creates a thread-safe slice over `signal`; see [`Slice::new`].
  pub fn new_sync(signal: Signal<T, SyncStorage>, read: GF, write: UF) -> Self {
    Self {
      signal,
      read: Signal::new_sync(read),
      write: Signal::new_sync(write),
      ty: PhantomData,
    }
  }
}

impl<T, O, GF, UF, S> Track for Slice<T, O, GF, UF, S>
where
  T: 'static,
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn id(&self) -> Id {
    self.signal.id()
  }
}

impl<T, O, GF, UF, S> Get<O> for Slice<T, O, GF, UF, S>
where
  T: 'static,
  O: Clone + 'static,
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn id(&self) -> Id {
    self.signal.id()
  }

  fn try_get(&self) -> Option<O> {
    let read = self.read.try_get_untracked()?;

    self.signal.try_with(|value| read(value))
  }

  fn try_get_untracked(&self) -> Option<O> {
    let read = self.read.try_get_untracked()?;

    self.signal.try_with_untracked(|value| read(value))
  }
}

impl<T, O, GF, UF, S> With<O> for Slice<T, O, GF, UF, S>
where
  T: 'static,
  O: 'static,
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn id(&self) -> Id {
    self.signal.id()
  }

  fn try_with<O2>(&self, f: impl FnOnce(&O) -> O2) -> Option<O2> {
    let read = self.read.try_get_untracked()?;

    self.signal.try_with(|value| f(&read(value)))
  }

  fn try_with_untracked<O2>(&self, f: impl FnOnce(&O) -> O2) -> Option<O2> {
    let read = self.read.try_get_untracked()?;

    self.signal.try_with_untracked(|value| f(&read(value)))
  }
}

impl<T, O, GF, UF, S> Set<O> for Slice<T, O, GF, UF, S>
where
  T: 'static,
  O: 'static,
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn id(&self) -> Id {
    self.signal.id()
  }

  fn set(&self, value: O) {
    if let Some(write) = self.write.try_get_untracked() {
      self.signal.update(|current| write(current, &value));
    }
  }
}

impl<T, O, GF, UF, S> Update<O> for Slice<T, O, GF, UF, S>
where
  T: 'static,
  O: 'static,
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn id(&self) -> Id {
    self.signal.id()
  }

  fn try_update<O2>(&self, f: impl FnOnce(&mut O) -> O2) -> Option<O2> {
    let read = self.read.try_get_untracked()?;
    let write = self.write.try_get_untracked()?;

    self.signal.try_update(|current| {
      let mut projected = read(current);
      let result = f(&mut projected);

      write(current, &projected);

      result
    })
  }
}
