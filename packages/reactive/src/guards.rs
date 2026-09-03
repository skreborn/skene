use std::{
  any::Any,
  mem::transmute,
  ops::{Deref, DerefMut},
  rc::Rc,
  sync::Arc,
};

use parking_lot::{Mutex, MutexGuard};

use crate::{
  id::Id,
  value::{TrackedRef, TrackedRefCell, TrackedRefMut},
};

/// A shared borrow guard over a signal's value.
#[must_use]
pub struct ReadGuard<'a, T>(ReadGuardInner<'a, T>);

enum ReadGuardInner<'a, T> {
  Local {
    _handle: Rc<TrackedRefCell<T>>,
    guard: TrackedRef<'a, T>,
  },
  Sync {
    _handle: Arc<dyn Any + Send + Sync>,
    guard: MutexGuard<'a, T>,
  },
}

impl<'a, T> ReadGuard<'a, T> {
  pub(crate) fn local(handle: Rc<TrackedRefCell<T>>) -> Self {
    let guard = handle.borrow();

    // SAFETY: the retained `Rc` keeps the data alive for the guard's lifetime.
    let guard = unsafe { transmute::<TrackedRef<'_, T>, TrackedRef<'a, T>>(guard) };

    Self(ReadGuardInner::Local {
      _handle: handle,
      guard,
    })
  }

  pub(crate) fn sync(handle: Arc<dyn Any + Send + Sync>) -> Self
  where
    T: 'static,
  {
    let guard = handle
      .downcast_ref::<Mutex<T>>()
      .expect("to downcast signal type")
      .lock();

    // SAFETY: the retained `Arc` keeps the data alive for the guard's lifetime.
    let guard = unsafe { transmute::<MutexGuard<'_, T>, MutexGuard<'a, T>>(guard) };

    Self(ReadGuardInner::Sync {
      _handle: handle,
      guard,
    })
  }
}

impl<T> Deref for ReadGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    match &self.0 {
      ReadGuardInner::Local { guard, .. } => guard,
      ReadGuardInner::Sync { guard, .. } => guard,
    }
  }
}

/// An exclusive borrow guard over a signal's value; dropping it notifies subscribers.
#[must_use]
pub struct WriteGuard<'a, T> {
  id: Id,
  inner: WriteGuardInner<'a, T>,
}

enum WriteGuardInner<'a, T> {
  Local {
    _handle: Rc<TrackedRefCell<T>>,
    guard: Option<TrackedRefMut<'a, T>>,
  },
  Sync {
    _handle: Arc<dyn Any + Send + Sync>,
    guard: Option<MutexGuard<'a, T>>,
  },
}

impl<'a, T> WriteGuard<'a, T> {
  pub(crate) fn local(id: Id, handle: Rc<TrackedRefCell<T>>) -> Self {
    let guard = handle.borrow_mut();

    // SAFETY: the retained `Rc` keeps the data alive for the guard's lifetime.
    let guard = unsafe { transmute::<TrackedRefMut<'_, T>, TrackedRefMut<'a, T>>(guard) };

    Self {
      id,
      inner: WriteGuardInner::Local {
        _handle: handle,
        guard: Some(guard),
      },
    }
  }

  pub(crate) fn sync(id: Id, handle: Arc<dyn Any + Send + Sync>) -> Self
  where
    T: 'static,
  {
    let guard = handle
      .downcast_ref::<Mutex<T>>()
      .expect("to downcast signal type")
      .lock();

    // SAFETY: the retained `Arc` keeps the data alive for the guard's lifetime.
    let guard = unsafe { transmute::<MutexGuard<'_, T>, MutexGuard<'a, T>>(guard) };

    Self {
      id,
      inner: WriteGuardInner::Sync {
        _handle: handle,
        guard: Some(guard),
      },
    }
  }
}

impl<T> Drop for WriteGuard<'_, T> {
  fn drop(&mut self) {
    match &mut self.inner {
      WriteGuardInner::Local { guard, .. } => drop(guard.take()),
      WriteGuardInner::Sync { guard, .. } => drop(guard.take()),
    }

    if let Some(state) = self.id.state() {
      state.notify();
    }
  }
}

impl<T> Deref for WriteGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    match &self.inner {
      WriteGuardInner::Local { guard, .. } => guard.as_ref().expect("guard present"),
      WriteGuardInner::Sync { guard, .. } => guard.as_ref().expect("guard present"),
    }
  }
}

impl<T> DerefMut for WriteGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    match &mut self.inner {
      WriteGuardInner::Local { guard, .. } => guard.as_mut().expect("guard present"),
      WriteGuardInner::Sync { guard, .. } => guard.as_mut().expect("guard present"),
    }
  }
}
