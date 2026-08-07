#[cfg(debug_assertions)]
use std::panic::Location;
use std::{any::Any, cell::RefCell, fmt, marker::PhantomData, ops::Deref, rc::Rc, sync::Arc};

use parking_lot::Mutex;
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::{
  guards::{ReadGuard, WriteGuard},
  id::Id,
  runtime::{RUNTIME, Runtime},
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

impl<T: Default + 'static> Default for Signal<T> {
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<T: 'static> Signal<T> {
  /// Creates a signal holding `value`, owned by the current [`Owner`](crate::Owner).
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(value: T) -> Self {
    let id = SignalState::create(value);

    id.set_owner();

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
      id.set_owner();
    }

    Self::from_id(id)
  }
}

/// Creates a signal holding `value` and returns its two halves.
#[cfg_attr(debug_assertions, track_caller)]
pub fn signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
  let signal = Signal::new(value);

  signal.split()
}

/// Creates a thread-safe signal holding `value` and returns its two halves; see
/// [`Signal::new_sync`] for the ownership contract.
pub fn sync_signal<T: Send + Sync + 'static>(value: T) -> (SyncReadSignal<T>, SyncWriteSignal<T>) {
  let signal = Signal::new_sync(value);

  signal.split()
}

/// One signal's state: its value cell and subscriber set, shared out of the runtime so
/// callers can act on it after releasing the runtime borrow.
#[derive(Clone)]
pub(crate) struct SignalState(Rc<SignalStateInner>);

pub(crate) struct SignalStateInner {
  pub(crate) id: Id,
  pub(crate) value: SignalValue,
  pub(crate) subscribers: Subscribers,
}

impl Deref for SignalState {
  type Target = SignalStateInner;

  #[inline(always)]
  fn deref(&self) -> &SignalStateInner {
    &self.0
  }
}

/// A signal's value cell: local (single-thread) or sync (cross-thread).
#[derive(Clone)]
pub(crate) enum SignalValue {
  Sync(Arc<dyn Any + Send + Sync>),
  Local(Rc<dyn Any>),
}

/// A signal's subscriber set: local (single-thread) or sync (cross-thread).
#[derive(Clone)]
pub(crate) enum Subscribers {
  Local(Rc<RefCell<SmallVec<[Id; 4]>>>),
  Sync(Arc<Mutex<FxHashSet<Id>>>),
}

impl Subscribers {
  pub(crate) fn local() -> Self {
    Self::Local(Rc::new(RefCell::new(SmallVec::new())))
  }

  /// Adds `id`, if not already present.
  fn insert(&self, id: Id) {
    match self {
      Self::Local(subscribers) => {
        let mut subscribers = subscribers.borrow_mut();

        if !subscribers.contains(&id) {
          subscribers.push(id);
        }
      }
      Self::Sync(subscribers) => {
        subscribers.lock().insert(id);
      }
    }
  }

  /// Removes `id`, if present.
  pub(crate) fn remove(&self, id: Id) {
    match self {
      Self::Local(subscribers) => subscribers.borrow_mut().retain(|other| *other != id),
      Self::Sync(subscribers) => {
        subscribers.lock().remove(&id);
      }
    }
  }

  /// Returns a detached copy of the current subscriber ids.
  pub(crate) fn snapshot(&self) -> SmallVec<[Id; 4]> {
    match self {
      Self::Local(subscribers) => subscribers.borrow().iter().copied().collect(),
      Self::Sync(subscribers) => subscribers.lock().iter().copied().collect(),
    }
  }
}

impl SignalState {
  pub(crate) fn from_parts(id: Id, value: SignalValue, subscribers: Subscribers) -> Self {
    Self(Rc::new(SignalStateInner {
      id,
      value,
      subscribers,
    }))
  }

  /// Creates a local signal holding `value` and returns its id, unowned.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn create<T: Any + 'static>(value: T) -> Id {
    Runtime::assert_ui_thread();

    let value = Rc::new(TrackedRefCell::new(value));

    RUNTIME.with(|runtime| {
      let mut arena = runtime.arena.borrow_mut();
      let id = arena.allocate();
      let state = Self::from_parts(id, SignalValue::Local(value), Subscribers::local());

      arena.get_mut(id).expect("freshly allocated slot").signal = Some(state);

      id
    })
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

  /// Mutates the value through `f` and re-runs subscribers.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn update<U, T: 'static>(&self, f: impl FnOnce(&mut T) -> U) -> U {
    let result = self.update_untracked(f);

    self.run_effects();

    result
  }

  /// Mutates the value without re-running subscribers, for a caller owning its own
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

  /// A write guard over the value; dropping it re-runs subscribers.
  pub(crate) fn write_guard<'a, T: 'static>(&self) -> WriteGuard<'a, T> {
    match &self.value {
      SignalValue::Local(_) => WriteGuard::local(self.id, self.as_local::<T>()),
      SignalValue::Sync(value) => WriteGuard::sync(self.id, value.clone()),
    }
  }

  /// Marks dependent memos stale and schedules this signal's subscribers, running them
  /// unless a batch is open; an off-UI write enqueues to the sync registry instead.
  pub(crate) fn run_effects(&self) {
    let ids = self.subscribers.snapshot();

    if !ids.is_empty() && matches!(self.value, SignalValue::Sync(_)) {
      Self::warn_if_undesignated();
    }

    if !Runtime::is_ui_thread() {
      SYNC.enqueue_effects(ids);

      return;
    }

    RUNTIME.with(|runtime| {
      // Mark before scheduling, so a read that happens before the effects run (inside a
      // batch, or mid-flush) pulls a fresh value instead of a stale cache.
      runtime.mark_stale(&ids);

      for id in &ids {
        runtime.add_pending_effect(*id);
      }

      if !runtime.batching.get() {
        runtime.run_pending_effects();
      }
    });
  }

  /// Warns once when a sync signal with subscribers runs effects but no UI thread was
  /// ever designated. Without one, an off-thread write routes its effects to the
  /// writer's own runtime and they never reach the UI thread—a silent loss this at
  /// least makes loud.
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

  /// Subscribes the running effect to this signal.
  ///
  /// Sync subscribers must be UI-thread effects; an off-thread read stays untracked.
  pub(crate) fn subscribe(&self) {
    if matches!(self.value, SignalValue::Sync(_)) && !Runtime::is_ui_thread() {
      return;
    }

    RUNTIME.with(|runtime| {
      if let Some(effect) = runtime.current_effect.borrow().as_ref() {
        self.subscribers.insert(effect.id());
        effect.add_observer(self.id);
      }
    });
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
