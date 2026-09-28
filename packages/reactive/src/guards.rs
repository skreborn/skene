use std::{
  mem::transmute,
  ops::{Deref, DerefMut},
  rc::Rc,
};

use crate::{
  id::Id,
  value::{TrackedRef, TrackedRefCell, TrackedRefMut},
};

/// A shared borrow guard over a signal's value.
#[must_use]
pub struct ReadGuard<'a, T> {
  guard: TrackedRef<'a, T>,
  _handle: Rc<TrackedRefCell<T>>,
}

impl<'a, T> ReadGuard<'a, T> {
  pub(crate) fn new(handle: Rc<TrackedRefCell<T>>) -> Self {
    let guard = handle.borrow();

    // SAFETY: `guard` is declared before `_handle`, so it drops while the retained
    // `Rc` still keeps the cell alive.
    let guard = unsafe { transmute::<TrackedRef<'_, T>, TrackedRef<'a, T>>(guard) };

    Self {
      guard,
      _handle: handle,
    }
  }
}

impl<T> Deref for ReadGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

/// An exclusive borrow guard over a signal's value; dropping it notifies subscribers.
#[must_use]
pub struct WriteGuard<'a, T> {
  id: Id,
  guard: Option<TrackedRefMut<'a, T>>,
  _handle: Rc<TrackedRefCell<T>>,
}

impl<'a, T> WriteGuard<'a, T> {
  pub(crate) fn new(id: Id, handle: Rc<TrackedRefCell<T>>) -> Self {
    let guard = handle.borrow_mut();

    // SAFETY: `Drop` releases `guard` first, while the retained `Rc` still keeps the
    // cell alive.
    let guard = unsafe { transmute::<TrackedRefMut<'_, T>, TrackedRefMut<'a, T>>(guard) };

    Self {
      id,
      guard: Some(guard),
      _handle: handle,
    }
  }
}

impl<T> Drop for WriteGuard<'_, T> {
  fn drop(&mut self) {
    drop(self.guard.take());

    if let Some(state) = self.id.state() {
      state.notify();
    }
  }
}

impl<T> Deref for WriteGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    self.guard.as_ref().expect("guard present")
  }
}

impl<T> DerefMut for WriteGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.guard.as_mut().expect("guard present")
  }
}
