#[cfg(debug_assertions)]
use std::panic::Location;
use std::{any::Any, fmt, marker::PhantomData, rc::Rc, sync::Arc};

use parking_lot::Mutex;

use crate::{
  computation,
  encode::Snapshot,
  guards::{ReadGuard, WriteGuard},
  id::{Id, Kind},
  runtime::{RUNTIME, Runtime, with_graph},
  storage::{LocalStorage, SyncStorage},
  sync::SYNC,
  value::TrackedRefCell,
};

/// A thread-safe [`Signal`].
pub type SyncSignal<T> = Signal<T, SyncStorage>;
/// A thread-safe [`ReadSignal`].
pub type SyncReadSignal<T> = ReadSignal<T, SyncStorage>;
/// A thread-safe [`WriteSignal`].
pub type SyncWriteSignal<T> = WriteSignal<T, SyncStorage>;

/// A reactive value: reads subscribe the running effect, writes re-run subscribers.
///
/// The handle is `Copy`; the value lives in the runtime until the owning
/// [`Owner`](crate::Owner) is disposed.
#[must_use]
pub struct Signal<T, S = LocalStorage> {
  pub(crate) id: Id,
  pub(crate) ty: PhantomData<T>,
  pub(crate) storage: PhantomData<S>,
}

/// The read half of a signal.
#[must_use]
pub struct ReadSignal<T, S = LocalStorage> {
  pub(crate) id: Id,
  pub(crate) ty: PhantomData<T>,
  pub(crate) storage: PhantomData<S>,
}

/// The write half of a signal.
#[must_use]
pub struct WriteSignal<T, S = LocalStorage> {
  pub(crate) id: Id,
  pub(crate) ty: PhantomData<T>,
  pub(crate) storage: PhantomData<S>,
}

macro_rules! handle_common {
  ($handle:ident) => {
    impl<T, S> $handle<T, S> {
      /// The signal's id.
      #[inline(always)]
      pub fn id(&self) -> Id {
        self.id
      }

      pub(crate) const fn from_id(id: Id) -> Self {
        Self {
          id,
          ty: PhantomData,
          storage: PhantomData,
        }
      }
    }

    impl<T, S> Copy for $handle<T, S> {}

    impl<T, S> Clone for $handle<T, S> {
      fn clone(&self) -> Self {
        *self
      }
    }

    impl<T, S> Eq for $handle<T, S> {}

    impl<T, S> PartialEq for $handle<T, S> {
      fn eq(&self, other: &Self) -> bool {
        self.id == other.id
      }
    }

    impl<T, S> fmt::Debug for $handle<T, S> {
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

impl<T, S> Signal<T, S> {
  /// The read half of this signal.
  #[inline(always)]
  pub const fn read_only(&self) -> ReadSignal<T, S> {
    ReadSignal::from_id(self.id)
  }

  /// The write half of this signal.
  #[inline(always)]
  pub const fn write_only(&self) -> WriteSignal<T, S> {
    WriteSignal::from_id(self.id)
  }

  /// Both halves of this signal.
  #[inline(always)]
  pub const fn split(&self) -> (ReadSignal<T, S>, WriteSignal<T, S>) {
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

impl<T: Send + Sync + 'static> Signal<T, SyncStorage> {
  /// Creates a thread-safe signal holding `value`.
  ///
  /// On the UI thread the signal is owned by the current [`Owner`](crate::Owner); off
  /// it, the signal is unowned and the caller must dispose it.
  pub fn new_sync(value: T) -> Self {
    let id = SignalState::create_sync(value);

    if Runtime::is_ui_thread() {
      Runtime::adopt_current(id);
    }

    Self::from_id(id)
  }
}

/// Creates a signal holding `value` and returns its two halves; see [`Signal::new`].
#[cfg_attr(debug_assertions, track_caller)]
pub fn signal<T: Snapshot>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
  let signal = Signal::new(value);

  signal.split()
}

/// Creates a thread-safe signal holding `value` and returns its two halves; see
/// [`Signal::new_sync`] for the ownership contract.
pub fn sync_signal<T: Send + Sync + 'static>(value: T) -> (SyncReadSignal<T>, SyncWriteSignal<T>) {
  let signal = Signal::new_sync(value);

  signal.split()
}

/// One signal's value cell, cloned out of its realm so a caller can act on it after
/// releasing every runtime borrow.
#[derive(Clone)]
pub(crate) struct SignalState {
  pub(crate) id: Id,
  pub(crate) value: SignalValue,
}

/// A signal's value cell: local (single-thread) or sync (cross-thread).
#[derive(Clone)]
pub(crate) enum SignalValue {
  Sync(Arc<dyn Any + Send + Sync>),
  Local(Rc<dyn Any>),
}

impl Id {
  /// The signal state this id names, resolved in the id's own realm.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn state(self) -> Option<SignalState> {
    match self.kind() {
      Kind::Sync => SYNC.get(self),
      Kind::Signal => {
        Runtime::assert_ui_thread();

        with_graph(|graph| {
          graph.signal(self).map(|signal| SignalState {
            id: self,
            value: SignalValue::Local(signal.value.clone()),
          })
        })
      }
      Kind::Computation | Kind::Owner => None,
    }
  }
}

impl SignalState {
  /// Creates a local signal holding `value` and returns its id, unowned.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn create<T: Any + 'static>(value: T) -> Id {
    Runtime::assert_ui_thread();

    let value = Rc::new(TrackedRefCell::new(value));

    with_graph(|graph| graph.allocate_signal(value))
  }

  /// Creates a sync signal holding `value` and returns its id, unowned; the current
  /// owner is recorded for unlinking when the creating thread is the UI thread.
  pub(crate) fn create_sync<T: Any + Send + Sync + 'static>(value: T) -> Id {
    let parent = if Runtime::is_ui_thread() {
      RUNTIME.try_with(|runtime| runtime.current_owner.get()).ok()
    }
    else {
      None
    };

    SYNC
      .insert(Arc::new(Mutex::new(value)), parent)
      .expect("sync registry poisoned")
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
  pub(crate) fn with_untracked<O, T: 'static>(&self, f: impl FnOnce(&T) -> O) -> O {
    match &self.value {
      SignalValue::Sync(value) => {
        let value = value
          .as_ref()
          .downcast_ref::<Mutex<T>>()
          .expect("to downcast signal type");

        f(&value.lock())
      }
      SignalValue::Local(value) => {
        let value = value
          .as_ref()
          .downcast_ref::<TrackedRefCell<T>>()
          .expect("to downcast signal type");

        #[cfg(debug_assertions)]
        {
          f(&value.borrow_at(Location::caller()))
        }
        #[cfg(not(debug_assertions))]
        {
          f(&value.borrow())
        }
      }
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn with<O, T: 'static>(&self, f: impl FnOnce(&T) -> O) -> O {
    self.subscribe();

    self.with_untracked(f)
  }

  /// Mutates the value through `f` and notifies subscribers.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn update<U, T: 'static>(&self, f: impl FnOnce(&mut T) -> U) -> U {
    let result = self.update_untracked(f);

    self.notify();

    result
  }

  /// Mutates the value without notifying subscribers, for a caller owning its own
  /// notification (a store's trigger fan-out).
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn update_untracked<U, T: 'static>(&self, f: impl FnOnce(&mut T) -> U) -> U {
    match &self.value {
      SignalValue::Sync(value) => {
        let value = value
          .as_ref()
          .downcast_ref::<Mutex<T>>()
          .expect("to downcast signal type");

        f(&mut value.lock())
      }
      SignalValue::Local(value) => {
        let value = value
          .as_ref()
          .downcast_ref::<TrackedRefCell<T>>()
          .expect("to downcast signal type");

        #[cfg(debug_assertions)]
        {
          f(&mut value.borrow_mut_at(Location::caller()))
        }
        #[cfg(not(debug_assertions))]
        {
          f(&mut value.borrow_mut())
        }
      }
    }
  }

  /// A read guard over the value, in whichever realm it lives.
  pub(crate) fn read_guard<'a, T: 'static>(&self) -> ReadGuard<'a, T> {
    match &self.value {
      SignalValue::Local(_) => ReadGuard::local(self.as_local::<T>()),
      SignalValue::Sync(value) => ReadGuard::sync(value.clone()),
    }
  }

  /// A write guard over the value; dropping it notifies subscribers.
  pub(crate) fn write_guard<'a, T: 'static>(&self) -> WriteGuard<'a, T> {
    match &self.value {
      SignalValue::Local(_) => WriteGuard::local(self.id, self.as_local::<T>()),
      SignalValue::Sync(value) => WriteGuard::sync(self.id, value.clone()),
    }
  }

  /// Propagates a change of this signal and flushes unless a batch is open; an off-UI
  /// write enqueues to the sync registry instead.
  pub(crate) fn notify(&self) {
    if self.id.is_sync() {
      if SYNC.is_subscribed(self.id) {
        Self::warn_if_undesignated();
      }

      if !Runtime::is_ui_thread() {
        SYNC.enqueue_write(self.id);

        return;
      }
    }

    RUNTIME.with(|runtime| {
      runtime.graph().propagate(self.id);
      runtime.request_flush();
    });
  }

  /// Warns once when a sync signal with subscribers is written but no UI thread was
  /// ever designated. Without one, an off-thread write propagates in the writer's own
  /// runtime and never reaches the UI thread—a silent loss this at least makes loud.
  fn warn_if_undesignated() {
    if Runtime::ui_thread_designated() {
      return;
    }

    static WARNED: std::sync::Once = std::sync::Once::new();

    WARNED.call_once(|| {
      eprintln!(
        "skene-reactive: a sync signal has subscribed effects but \
         Runtime::init_on_ui_thread() was never called—an off-thread write's effects \
         are silently dropped. Call init_on_ui_thread() during setup."
      );
    });
  }

  /// Subscribes the running computation to this signal.
  ///
  /// Sync subscribers must be UI-thread effects; an off-thread read stays untracked.
  pub(crate) fn subscribe(&self) {
    if self.id.is_sync() {
      if !Runtime::is_ui_thread() {
        return;
      }

      SYNC.mark_subscribed(self.id);
    }

    computation::subscribe(self.id);
  }

  pub(crate) fn as_local<T: 'static>(&self) -> Rc<TrackedRefCell<T>> {
    match &self.value {
      SignalValue::Local(value) => value
        .clone()
        .downcast::<TrackedRefCell<T>>()
        .expect("to downcast signal type"),
      SignalValue::Sync(_) => unreachable!("expected local signal storage"),
    }
  }
}

/// Mutates a signal's value without notifying, for a caller owning its own
/// notification; `None` when the signal is disposed.
pub(crate) fn update_silently<T: 'static, R>(id: Id, write: impl FnOnce(&mut T) -> R) -> Option<R> {
  id.state().map(|state| state.update_untracked(write))
}
