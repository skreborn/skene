#[cfg(debug_assertions)]
use std::panic::Location;
use std::{any::Any, fmt, marker::PhantomData, rc::Rc};

use crate::{
  computation,
  encode::Snapshot,
  guards::{ReadGuard, WriteGuard},
  id::{Id, Kind},
  runtime::{RUNTIME, Runtime, with_graph},
  value::TrackedRefCell,
};

/// A reactive value: reads subscribe the running effect, writes re-run subscribers.
///
/// The handle is `Copy` and stays on the thread that made it; the value lives in that
/// thread's runtime until the owning [`Owner`](crate::Owner) is disposed.
#[must_use]
pub struct Signal<T> {
  pub(crate) id: Id,
  pub(crate) ty: PhantomData<T>,
  local: PhantomData<*const ()>,
}

/// The read half of a signal.
#[must_use]
pub struct ReadSignal<T> {
  pub(crate) id: Id,
  pub(crate) ty: PhantomData<T>,
  local: PhantomData<*const ()>,
}

/// The write half of a signal.
#[must_use]
pub struct WriteSignal<T> {
  pub(crate) id: Id,
  pub(crate) ty: PhantomData<T>,
  local: PhantomData<*const ()>,
}

macro_rules! handle_common {
  ($handle:ident) => {
    impl<T> $handle<T> {
      /// The signal's id.
      #[inline(always)]
      pub fn id(&self) -> Id {
        self.id
      }

      pub(crate) const fn from_id(id: Id) -> Self {
        Self {
          id,
          ty: PhantomData,
          local: PhantomData,
        }
      }
    }

    impl<T> Copy for $handle<T> {}

    impl<T> Clone for $handle<T> {
      fn clone(&self) -> Self {
        *self
      }
    }

    impl<T> Eq for $handle<T> {}

    impl<T> PartialEq for $handle<T> {
      fn eq(&self, other: &Self) -> bool {
        self.id == other.id
      }
    }

    impl<T> fmt::Debug for $handle<T> {
      fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut r#struct = formatter.debug_struct(stringify!($handle));

        r#struct.field("id", &self.id);

        r#struct.finish()
      }
    }
  };
}

handle_common!(Signal);
handle_common!(ReadSignal);
handle_common!(WriteSignal);

impl<T> Signal<T> {
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

  /// Both halves of this signal.
  #[inline(always)]
  pub const fn split(&self) -> (ReadSignal<T>, WriteSignal<T>) {
    (self.read_only(), self.write_only())
  }
}

impl<T: Default + Snapshot> Default for Signal<T> {
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<T: Snapshot> Signal<T> {
  /// Creates a signal holding `value`, owned by the current [`Owner`](crate::Owner).
  ///
  /// The signal takes part in state snapshots: its identity is its position—the
  /// owner's path and the signal's index among the owner's snapshot signals, plus its
  /// type—so a [`restore`](crate::restore)d run adopts the value a prior run held at
  /// that position, when one was staged and decodes as `T`, and `value` otherwise.
  /// [`tagged`](Self::tagged) names the position instead; [`opaque`](Self::opaque)
  /// stays out of snapshots and needs no [`Snapshot`].
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(value: T) -> Self {
    Self::snapshotted(None, value)
  }
}

impl<T: 'static> Signal<T> {
  /// Creates a signal holding `value`, owned by the current [`Owner`](crate::Owner),
  /// that takes no part in state snapshots.
  ///
  /// For a value that cannot or should not round-trip: a closure, a handle, platform
  /// state the platform owns. It occupies no snapshot position, so adding or removing
  /// one shifts nothing around it.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn opaque(value: T) -> Self {
    let id = SignalState::create(value);

    Runtime::adopt_current(id);

    Self::from_id(id)
  }
}

/// Creates a signal holding `value` and returns its two halves; see [`Signal::new`].
#[cfg_attr(debug_assertions, track_caller)]
pub fn signal<T: Snapshot>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
  let signal = Signal::new(value);

  signal.split()
}

/// One signal's value cell, cloned out of the graph so a caller can act on it after
/// releasing every runtime borrow.
#[derive(Clone)]
pub(crate) struct SignalState {
  pub(crate) id: Id,
  pub(crate) value: Rc<dyn Any>,
}

impl Id {
  /// The signal state this id names.
  pub(crate) fn state(self) -> Option<SignalState> {
    if self.kind() != Kind::Signal {
      return None;
    }

    with_graph(|graph| {
      graph.signal(self).map(|signal| SignalState {
        id: self,
        value: signal.value.clone(),
      })
    })
  }
}

impl SignalState {
  /// Creates a local signal holding `value` and returns its id, unowned.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn create<T: Any + 'static>(value: T) -> Id {
    #[cfg(debug_assertions)]
    let site = Location::caller();
    let value = Rc::new(TrackedRefCell::new(value));

    with_graph(|graph| {
      let id = graph.allocate_signal(value);

      #[cfg(debug_assertions)]
      graph.note_creation(id, site);

      id
    })
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn get_untracked<T: Clone + 'static>(&self) -> T {
    self.with_untracked(Clone::clone)
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn get<T: Clone + 'static>(&self) -> T {
    self.subscribe();

    self.get_untracked()
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn with_untracked<O, T: 'static>(&self, apply: impl FnOnce(&T) -> O) -> O {
    let value = self
      .value
      .downcast_ref::<TrackedRefCell<T>>()
      .expect("to downcast signal type");

    #[cfg(debug_assertions)]
    {
      apply(&value.borrow_at(Location::caller()))
    }
    #[cfg(not(debug_assertions))]
    {
      apply(&value.borrow())
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn with<O, T: 'static>(&self, apply: impl FnOnce(&T) -> O) -> O {
    self.subscribe();

    self.with_untracked(apply)
  }

  /// Mutates the value through `mutate` and notifies subscribers.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn update<U, T: 'static>(&self, mutate: impl FnOnce(&mut T) -> U) -> U {
    let result = self.update_untracked(mutate);

    self.notify();

    result
  }

  /// Mutates the value without notifying subscribers, for a caller owning its own
  /// notification (a store's trigger fan-out).
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn update_untracked<U, T: 'static>(&self, mutate: impl FnOnce(&mut T) -> U) -> U {
    let value = self
      .value
      .downcast_ref::<TrackedRefCell<T>>()
      .expect("to downcast signal type");

    #[cfg(debug_assertions)]
    {
      mutate(&mut value.borrow_mut_at(Location::caller()))
    }
    #[cfg(not(debug_assertions))]
    {
      mutate(&mut value.borrow_mut())
    }
  }

  /// A read guard over the value.
  pub(crate) fn read_guard<'a, T: 'static>(&self) -> ReadGuard<'a, T> {
    ReadGuard::new(self.cell::<T>())
  }

  /// A write guard over the value; dropping it notifies subscribers.
  pub(crate) fn write_guard<'a, T: 'static>(&self) -> WriteGuard<'a, T> {
    WriteGuard::new(self.id, self.cell::<T>())
  }

  /// Propagates a change of this signal and flushes unless a batch is open.
  pub(crate) fn notify(&self) {
    RUNTIME.with(|runtime| {
      runtime.graph().propagate(self.id);
      runtime.request_flush();
    });
  }

  /// Subscribes the running computation to this signal.
  pub(crate) fn subscribe(&self) {
    computation::subscribe(self.id);
  }

  /// The value's cell.
  pub(crate) fn cell<T: 'static>(&self) -> Rc<TrackedRefCell<T>> {
    self
      .value
      .clone()
      .downcast::<TrackedRefCell<T>>()
      .expect("to downcast signal type")
  }
}

/// Mutates a signal's value without notifying, for a caller owning its own
/// notification; `None` when the signal is disposed.
pub(crate) fn update_silently<T: 'static, R>(id: Id, write: impl FnOnce(&mut T) -> R) -> Option<R> {
  id.state().map(|state| state.update_untracked(write))
}
