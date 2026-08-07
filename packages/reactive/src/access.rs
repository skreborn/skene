//! The access traits every readable or writable reactive handle shares.

use crate::{
  guards::{ReadGuard, WriteGuard},
  id::Id,
};

/// Subscribes the running effect to a signal without reading its value.
pub trait Track {
  /// The signal's id.
  fn id(&self) -> Id;

  /// Subscribes the running effect to the signal.
  ///
  /// # Panics
  ///
  /// Panics when the signal is disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn track(&self) {
    self.id().state().expect("signal disposed").subscribe();
  }

  /// Subscribes the running effect to the signal, if it is live.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_track(&self) {
    if let Some(state) = self.id().state() {
      state.subscribe();
    }
  }
}

/// Reads a signal's value by cloning it.
pub trait Get<T: Clone> {
  /// The signal's id.
  fn id(&self) -> Id;

  /// Clones the current value, subscribing the running effect.
  ///
  /// # Panics
  ///
  /// Panics when the signal is disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn get(&self) -> T
  where
    T: 'static,
  {
    self.try_get().expect("signal disposed")
  }

  /// Clones the current value without subscribing.
  ///
  /// # Panics
  ///
  /// Panics when the signal is disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn get_untracked(&self) -> T
  where
    T: 'static,
  {
    self.try_get_untracked().expect("signal disposed")
  }

  /// Clones the current value if the signal is live, subscribing the running effect.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_get(&self) -> Option<T>
  where
    T: 'static,
  {
    self.id().state().map(|state| state.get())
  }

  /// Clones the current value if the signal is live, without subscribing.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_get_untracked(&self) -> Option<T>
  where
    T: 'static,
  {
    self.id().state().map(|state| state.get_untracked())
  }
}

/// Reads a signal's value by reference.
pub trait With<T> {
  /// The signal's id.
  fn id(&self) -> Id;

  /// Passes the current value to `f`, subscribing the running effect.
  ///
  /// # Panics
  ///
  /// Panics when the signal is disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn with<O>(&self, f: impl FnOnce(&T) -> O) -> O
  where
    T: 'static,
  {
    self.try_with(f).expect("signal disposed")
  }

  /// Passes the current value to `f` without subscribing.
  ///
  /// # Panics
  ///
  /// Panics when the signal is disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn with_untracked<O>(&self, f: impl FnOnce(&T) -> O) -> O
  where
    T: 'static,
  {
    self.try_with_untracked(f).expect("signal disposed")
  }

  /// Passes the current value to `f` if the signal is live, subscribing the running
  /// effect.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_with<O>(&self, f: impl FnOnce(&T) -> O) -> Option<O>
  where
    T: 'static,
  {
    self.id().state().map(|state| state.with(f))
  }

  /// Passes the current value to `f` if the signal is live, without subscribing.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_with_untracked<O>(&self, f: impl FnOnce(&T) -> O) -> Option<O>
  where
    T: 'static,
  {
    self.id().state().map(|state| state.with_untracked(f))
  }
}

/// Reads a signal's value through a guard, without cloning.
pub trait Read<T> {
  /// The signal's id.
  fn id(&self) -> Id;

  /// Borrows the value, subscribing the running effect.
  ///
  /// # Panics
  ///
  /// Panics when the signal is disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn read(&self) -> ReadGuard<'_, T>
  where
    T: 'static,
  {
    self.try_read().expect("signal disposed")
  }

  /// Borrows the value without subscribing.
  ///
  /// # Panics
  ///
  /// Panics when the signal is disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn read_untracked(&self) -> ReadGuard<'_, T>
  where
    T: 'static,
  {
    self.try_read_untracked().expect("signal disposed")
  }

  /// Borrows the value if the signal is live, subscribing the running effect.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_read(&self) -> Option<ReadGuard<'_, T>>
  where
    T: 'static,
  {
    self.id().state().map(|state| {
      state.subscribe();

      state.read_guard()
    })
  }

  /// Borrows the value if the signal is live, without subscribing.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_read_untracked(&self) -> Option<ReadGuard<'_, T>>
  where
    T: 'static,
  {
    self.id().state().map(|state| state.read_guard())
  }
}

/// Replaces a signal's value, re-running its subscribers.
pub trait Set<T> {
  /// The signal's id.
  fn id(&self) -> Id;

  /// Replaces the value and re-runs subscribers; does nothing when the signal is
  /// disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn set(&self, value: T)
  where
    T: 'static,
  {
    if let Some(state) = self.id().state() {
      state.update(|current| *current = value);
    }
  }
}

/// Updates a signal's value in place, re-running its subscribers.
pub trait Update<T> {
  /// The signal's id.
  fn id(&self) -> Id;

  /// Mutates the value through `f` and re-runs subscribers; does nothing when the
  /// signal is disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn update(&self, f: impl FnOnce(&mut T))
  where
    T: 'static,
  {
    let _ = self.try_update(f);
  }

  /// Mutates the value through `f` if the signal is live, re-runs subscribers, and
  /// returns `f`'s result.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
  where
    T: 'static,
  {
    self.id().state().map(|state| state.update(f))
  }
}

/// Mutably borrows a signal's value through a guard.
pub trait Write<T> {
  /// The signal's id.
  fn id(&self) -> Id;

  /// Mutably borrows the value, re-running subscribers when the guard is dropped.
  ///
  /// # Panics
  ///
  /// Panics when the signal is disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  fn write(&self) -> WriteGuard<'_, T>
  where
    T: 'static,
  {
    self.try_write().expect("signal disposed")
  }

  /// Mutably borrows the value if the signal is live.
  #[cfg_attr(debug_assertions, track_caller)]
  fn try_write(&self) -> Option<WriteGuard<'_, T>>
  where
    T: 'static,
  {
    self.id().state().map(|state| state.write_guard())
  }
}

mod impls {
  use super::{Get, Read, Set, Track, Update, With, Write};
  use crate::{
    id::Id,
    signal::{ReadSignal, Signal, WriteSignal},
  };

  macro_rules! impl_id {
    ($trait:ident $(<$param:ident>)?, $handle:ident $(, $bound:path)?) => {
      impl<T $(: $bound)?, S> $trait$(<$param>)? for $handle<T, S> {
        #[inline(always)]
        fn id(&self) -> Id {
          self.id
        }
      }
    };
  }

  impl_id!(Track, Signal);
  impl_id!(Get<T>, Signal, Clone);
  impl_id!(With<T>, Signal);
  impl_id!(Read<T>, Signal);
  impl_id!(Set<T>, Signal);
  impl_id!(Update<T>, Signal);
  impl_id!(Write<T>, Signal);
  impl_id!(Track, ReadSignal);
  impl_id!(Get<T>, ReadSignal, Clone);
  impl_id!(With<T>, ReadSignal);
  impl_id!(Read<T>, ReadSignal);
  impl_id!(Set<T>, WriteSignal);
  impl_id!(Update<T>, WriteSignal);
  impl_id!(Write<T>, WriteSignal);
}
