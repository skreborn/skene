//! The borrow-tracked cell holding a local signal's value.

#[cfg(debug_assertions)]
use std::cell::Cell;
#[cfg(debug_assertions)]
use std::panic::Location;
use std::{
  cell::{Ref, RefCell, RefMut},
  ops::{Deref, DerefMut},
};

/// A `RefCell` that, in debug builds, reports where a conflicting borrow was taken.
#[derive(Debug)]
pub(crate) struct TrackedRefCell<T> {
  inner: RefCell<T>,
  #[cfg(debug_assertions)]
  shared_borrows: Cell<usize>,
  #[cfg(debug_assertions)]
  has_mut_borrow: Cell<bool>,
  #[cfg(debug_assertions)]
  holder: Cell<Option<&'static Location<'static>>>,
}

impl<T> TrackedRefCell<T> {
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) const fn new(value: T) -> Self {
    Self {
      inner: RefCell::new(value),
      #[cfg(debug_assertions)]
      shared_borrows: Cell::new(0),
      #[cfg(debug_assertions)]
      has_mut_borrow: Cell::new(false),
      #[cfg(debug_assertions)]
      holder: Cell::new(None),
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn borrow(&self) -> TrackedRef<'_, T> {
    #[cfg(debug_assertions)]
    return self.borrow_at(Location::caller());
    #[cfg(not(debug_assertions))]
    return TrackedRef {
      inner: self.inner.borrow(),
    };
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn borrow_mut(&self) -> TrackedRefMut<'_, T> {
    #[cfg(debug_assertions)]
    return self.borrow_mut_at(Location::caller());
    #[cfg(not(debug_assertions))]
    return TrackedRefMut {
      inner: self.inner.borrow_mut(),
    };
  }

  #[cfg(debug_assertions)]
  pub(crate) fn borrow_at(&self, caller: &'static Location<'static>) -> TrackedRef<'_, T> {
    let inner = self
      .inner
      .try_borrow()
      .unwrap_or_else(|_| self.panic_conflict(caller));
    let shared = self.shared_borrows.get();

    if shared == 0 && !self.has_mut_borrow.get() {
      self.holder.set(Some(caller));
    }

    self.shared_borrows.set(shared + 1);

    TrackedRef { inner, cell: self }
  }

  #[cfg(debug_assertions)]
  pub(crate) fn borrow_mut_at(&self, caller: &'static Location<'static>) -> TrackedRefMut<'_, T> {
    let inner = self
      .inner
      .try_borrow_mut()
      .unwrap_or_else(|_| self.panic_conflict(caller));

    if self.shared_borrows.get() == 0 && !self.has_mut_borrow.get() {
      self.holder.set(Some(caller));
    }

    self.has_mut_borrow.set(true);

    TrackedRefMut { inner, cell: self }
  }

  #[cfg(debug_assertions)]
  fn release_shared(&self) {
    let shared = self.shared_borrows.get().saturating_sub(1);

    self.shared_borrows.set(shared);

    if shared == 0 && !self.has_mut_borrow.get() {
      self.holder.set(None);
    }
  }

  #[cfg(debug_assertions)]
  fn release_mut(&self) {
    self.has_mut_borrow.set(false);

    if self.shared_borrows.get() == 0 {
      self.holder.set(None);
    }
  }

  #[cfg(debug_assertions)]
  fn panic_conflict(&self, caller: &'static Location<'static>) -> ! {
    match self.holder.get() {
      Some(loc) => panic!(
        "signal value already borrowed at {}:{} (attempted at {}:{})",
        loc.file(),
        loc.line(),
        caller.file(),
        caller.line()
      ),
      None => panic!(
        "signal value already borrowed (attempted at {}:{})",
        caller.file(),
        caller.line()
      ),
    }
  }
}

/// A shared borrow guard over a local signal's value.
#[must_use]
pub(crate) struct TrackedRef<'a, T> {
  inner: Ref<'a, T>,
  #[cfg(debug_assertions)]
  cell: &'a TrackedRefCell<T>,
}

impl<T> Drop for TrackedRef<'_, T> {
  fn drop(&mut self) {
    #[cfg(debug_assertions)]
    self.cell.release_shared();
  }
}

impl<T> Deref for TrackedRef<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

/// An exclusive borrow guard over a local signal's value.
pub(crate) struct TrackedRefMut<'a, T> {
  inner: RefMut<'a, T>,
  #[cfg(debug_assertions)]
  cell: &'a TrackedRefCell<T>,
}

impl<T> Drop for TrackedRefMut<'_, T> {
  fn drop(&mut self) {
    #[cfg(debug_assertions)]
    self.cell.release_mut();
  }
}

impl<T> Deref for TrackedRefMut<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<T> DerefMut for TrackedRefMut<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}
