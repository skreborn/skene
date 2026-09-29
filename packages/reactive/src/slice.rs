//! Two-way projections over one signal.

use std::marker::PhantomData;

use crate::{
  access::{Get, Set, Track, Update, With},
  id::Id,
  signal::Signal,
};

/// A read-write projection over part of a signal's value.
///
/// Reads go through `read`, writes map the projected value back through `write` and
/// store it into the underlying signal—notifying its subscribers as any write would.
/// The projection closures are held in signals of their own, keeping the handle `Copy`.
#[must_use]
pub struct Slice<T: 'static, O, GF, UF>
where
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  signal: Signal<T>,
  read: Signal<GF>,
  write: Signal<UF>,
  ty: PhantomData<O>,
}

impl<T: 'static, O, GF, UF> std::fmt::Debug for Slice<T, O, GF, UF>
where
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Slice").finish_non_exhaustive()
  }
}

impl<T, O, GF, UF> Clone for Slice<T, O, GF, UF>
where
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn clone(&self) -> Self {
    *self
  }
}

impl<T, O, GF, UF> Copy for Slice<T, O, GF, UF>
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
      read: Signal::opaque(read),
      write: Signal::opaque(write),
      ty: PhantomData,
    }
  }
}

impl<T, O, GF, UF> Track for Slice<T, O, GF, UF>
where
  T: 'static,
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn id(&self) -> Id {
    self.signal.id()
  }
}

impl<T, O, GF, UF> Get<O> for Slice<T, O, GF, UF>
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

impl<T, O, GF, UF> With<O> for Slice<T, O, GF, UF>
where
  T: 'static,
  O: 'static,
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn id(&self) -> Id {
    self.signal.id()
  }

  fn try_with<O2>(&self, apply: impl FnOnce(&O) -> O2) -> Option<O2> {
    let read = self.read.try_get_untracked()?;

    self.signal.try_with(|value| apply(&read(value)))
  }

  fn try_with_untracked<O2>(&self, apply: impl FnOnce(&O) -> O2) -> Option<O2> {
    let read = self.read.try_get_untracked()?;

    self.signal.try_with_untracked(|value| apply(&read(value)))
  }
}

impl<T, O, GF, UF> Set<O> for Slice<T, O, GF, UF>
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

impl<T, O, GF, UF> Update<O> for Slice<T, O, GF, UF>
where
  T: 'static,
  O: 'static,
  GF: Fn(&T) -> O + Clone + 'static,
  UF: Fn(&mut T, &O) + Clone + 'static,
{
  fn id(&self) -> Id {
    self.signal.id()
  }

  fn try_update<O2>(&self, mutate: impl FnOnce(&mut O) -> O2) -> Option<O2> {
    let read = self.read.try_get_untracked()?;
    let write = self.write.try_get_untracked()?;

    self.signal.try_update(|current| {
      let mut projected = read(current);
      let result = mutate(&mut projected);

      write(current, &projected);

      result
    })
  }
}
