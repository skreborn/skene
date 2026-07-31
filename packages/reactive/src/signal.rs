#[cfg(debug_assertions)]
use std::cell::Cell;
#[cfg(debug_assertions)]
use std::panic::Location;
use std::{
  any::Any,
  cell::{Ref, RefCell, RefMut},
  fmt,
  marker::PhantomData,
  ops::{Deref, DerefMut},
  rc::Rc,
  sync::Arc,
};

use parking_lot::Mutex;
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::{
  SignalGet,
  SignalUpdate,
  id::Id,
  read::{LocalReadRef, ReadRef, SignalRead, SignalTrack, SignalWith, SyncReadRef},
  runtime::{RUNTIME, Runtime},
  storage::{Storage, SyncStorage, UnsyncStorage},
  sync_runtime::{SYNC_RUNTIME, SyncSignal},
  write::{LocalWriteRef, SignalWrite, SyncWriteRef, WriteRef},
};

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
  pub(crate) fn new(value: T) -> Self {
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
  fn borrow_at(&self, caller: &'static Location<'static>) -> TrackedRef<'_, T> {
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
  fn borrow_mut_at(&self, caller: &'static Location<'static>) -> TrackedRefMut<'_, T> {
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

/// A shared borrow guard over a thread-local signal's value.
#[must_use]
pub struct TrackedRef<'a, T> {
  inner: Ref<'a, T>,
  #[cfg(debug_assertions)]
  cell: &'a TrackedRefCell<T>,
}

impl<'a, T> Drop for TrackedRef<'a, T> {
  fn drop(&mut self) {
    #[cfg(debug_assertions)]
    self.cell.release_shared();
  }
}

impl<'a, T> Deref for TrackedRef<'a, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

/// An exclusive borrow guard over a thread-local signal's value.
pub struct TrackedRefMut<'a, T> {
  inner: RefMut<'a, T>,
  #[cfg(debug_assertions)]
  cell: &'a TrackedRefCell<T>,
}

impl<'a, T> Drop for TrackedRefMut<'a, T> {
  fn drop(&mut self) {
    #[cfg(debug_assertions)]
    self.cell.release_mut();
  }
}

impl<'a, T> Deref for TrackedRefMut<'a, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<'a, T> DerefMut for TrackedRefMut<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

/// A thread-safe [`RwSignal`].
pub type SyncRwSignal<T> = RwSignal<T, SyncStorage>;
/// A thread-safe [`ReadSignal`].
pub type SyncReadSignal<T> = ReadSignal<T, SyncStorage>;
/// A thread-safe [`WriteSignal`].
pub type SyncWriteSignal<T> = WriteSignal<T, SyncStorage>;

impl<T, S> SignalTrack<T> for RwSignal<T, S> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T, S> SignalTrack<T> for ReadSignal<T, S> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: Any + 'static> Storage<T> for UnsyncStorage {
  fn create(value: T) -> Id {
    SignalState::new(value)
  }
}

impl<T: Any + Send + Sync + 'static> Storage<T> for SyncStorage {
  fn create(value: T) -> Id {
    SignalState::new_sync(value)
  }
}

/// A read-write signal that acts as both a getter and a setter.
#[must_use]
pub struct RwSignal<T, S = UnsyncStorage> {
  pub(crate) id: Id,
  pub(crate) ty: PhantomData<T>,
  pub(crate) st: PhantomData<S>,
}

impl<T, S> RwSignal<T, S> {
  /// The signal's `Id`.
  pub fn id(&self) -> Id {
    self.id
  }
}

impl<T, S> Copy for RwSignal<T, S> {}

impl<T, S> Clone for RwSignal<T, S> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T, S> Eq for RwSignal<T, S> {}

impl<T, S> PartialEq for RwSignal<T, S> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T, S> fmt::Debug for RwSignal<T, S> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut r#struct = formatter.debug_struct("RwSignal");

    r#struct.field("id", &self.id);
    r#struct.field("ty", &self.ty);

    r#struct.finish()
  }
}

impl<T: Default + 'static> Default for RwSignal<T> {
  fn default() -> Self {
    RwSignal::new(T::default())
  }
}

impl<T, S> RwSignal<T, S> {
  /// Creates a read-only `ReadSignal` of this signal.
  #[inline(always)]
  pub fn read_only(&self) -> ReadSignal<T, S> {
    ReadSignal {
      id: self.id,
      ty: PhantomData,
      st: PhantomData,
    }
  }

  /// Creates a write-only `WriteSignal` of this signal.
  #[inline(always)]
  pub fn write_only(&self) -> WriteSignal<T, S> {
    WriteSignal {
      id: self.id,
      ty: PhantomData,
      st: PhantomData,
    }
  }
}

impl<T: Send + Sync + 'static> RwSignal<T, SyncStorage> {
  /// Creates a sync signal. When called off the UI thread, the signal is left
  /// unscoped, so callers must ensure it is disposed manually.
  pub fn new_sync(value: T) -> Self {
    let id = SignalState::new_sync(value);

    if Runtime::is_ui_thread() {
      id.set_scope();
    }

    RwSignal {
      id,
      ty: PhantomData,
      st: PhantomData,
    }
  }

  /// Creates a sync signal with separate read/write handles. Off-UI calls
  /// leave the signal unscoped; the caller is responsible for disposal.
  pub fn new_sync_split(value: T) -> (ReadSignal<T, SyncStorage>, WriteSignal<T, SyncStorage>) {
    let sig = Self::new_sync(value);

    (sig.read_only(), sig.write_only())
  }
}

impl<T: 'static> RwSignal<T, UnsyncStorage> {
  /// Creates a signal holding `value`, scoped to the current reactive scope.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(value: T) -> Self {
    Runtime::assert_ui_thread();

    let id = <UnsyncStorage as Storage<T>>::create(value);
    id.set_scope();

    RwSignal {
      id,
      ty: PhantomData,
      st: PhantomData,
    }
  }

  /// Creates a signal with separate read/write handles.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new_split(value: T) -> (ReadSignal<T, UnsyncStorage>, WriteSignal<T, UnsyncStorage>) {
    let sig = Self::new(value);

    (sig.read_only(), sig.write_only())
  }
}

/// A read-only signal.
#[must_use]
pub struct ReadSignal<T, S = UnsyncStorage> {
  pub(crate) id: Id,
  pub(crate) ty: PhantomData<T>,
  pub(crate) st: PhantomData<S>,
}

impl<T, S> ReadSignal<T, S> {
  /// The signal's `Id`.
  pub fn id(&self) -> Id {
    self.id
  }
}

impl<T, S> Copy for ReadSignal<T, S> {}

impl<T, S> Clone for ReadSignal<T, S> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T, S> Eq for ReadSignal<T, S> {}

impl<T, S> PartialEq for ReadSignal<T, S> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

/// A write-only handle to a signal.
#[must_use]
pub struct WriteSignal<T, S = UnsyncStorage> {
  pub(crate) id: Id,
  pub(crate) ty: PhantomData<T>,
  pub(crate) st: PhantomData<S>,
}

impl<T, S> WriteSignal<T, S> {
  /// The signal's `Id`.
  pub fn id(&self) -> Id {
    self.id
  }
}

impl<T, S> Copy for WriteSignal<T, S> {}

impl<T, S> Clone for WriteSignal<T, S> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T, S> Eq for WriteSignal<T, S> {}

impl<T, S> PartialEq for WriteSignal<T, S> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

/// Internal state for a signal; stores the value and subscriber set.
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
  pub(crate) fn local() -> Subscribers {
    Subscribers::Local(Rc::new(RefCell::new(SmallVec::new())))
  }

  /// Adds `id`, if not already present.
  fn insert(&self, id: Id) {
    match self {
      Subscribers::Local(subscribers) => {
        let mut subscribers = subscribers.borrow_mut();

        if !subscribers.contains(&id) {
          subscribers.push(id);
        }
      }
      Subscribers::Sync(subscribers) => {
        subscribers.lock().insert(id);
      }
    }
  }

  /// Removes `id`, if present.
  pub(crate) fn remove(&self, id: Id) {
    match self {
      Subscribers::Local(subscribers) => subscribers.borrow_mut().retain(|other| *other != id),
      Subscribers::Sync(subscribers) => {
        subscribers.lock().remove(&id);
      }
    }
  }

  /// Returns a detached copy of the current subscriber ids.
  pub(crate) fn snapshot(&self) -> SmallVec<[Id; 4]> {
    match self {
      Subscribers::Local(subscribers) => subscribers.borrow().iter().copied().collect(),
      Subscribers::Sync(subscribers) => subscribers.lock().iter().copied().collect(),
    }
  }
}

impl SignalState {
  pub(crate) fn from_parts(id: Id, value: SignalValue, subscribers: Subscribers) -> SignalState {
    SignalState(Rc::new(SignalStateInner {
      id,
      value,
      subscribers,
    }))
  }

  #[expect(clippy::new_ret_no_self)]
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new<T>(value: T) -> Id
  where
    T: Any + 'static,
  {
    Runtime::assert_ui_thread();
    let id = Id::next();
    let value = TrackedRefCell::new(value);
    let signal =
      SignalState::from_parts(id, SignalValue::Local(Rc::new(value)), Subscribers::local());
    id.add_signal(signal);
    id
  }

  pub fn new_sync<T>(value: T) -> Id
  where
    T: Any + Send + Sync + 'static,
  {
    let id = Id::next();
    let value = Arc::new(Mutex::new(value));
    let subscribers = Arc::new(Mutex::new(FxHashSet::default()));

    SYNC_RUNTIME.insert_signal(
      id,
      SyncSignal {
        id,
        value,
        subscribers,
      },
    );
    id
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn get_untracked<T: Clone + 'static>(&self) -> T {
    match &self.value {
      SignalValue::Sync(v) => {
        let v = v
          .as_ref()
          .downcast_ref::<Mutex<T>>()
          .expect("to downcast signal type");
        v.lock().clone()
      }
      SignalValue::Local(v) => {
        let v = v
          .as_ref()
          .downcast_ref::<TrackedRefCell<T>>()
          .expect("to downcast signal type");
        #[cfg(debug_assertions)]
        {
          v.borrow_at(Location::caller()).clone()
        }
        #[cfg(not(debug_assertions))]
        {
          v.borrow().clone()
        }
      }
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn get<T: Clone + 'static>(&self) -> T {
    self.subscribe();
    self.get_untracked()
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn with_untracked<O, T: 'static>(&self, f: impl FnOnce(&T) -> O) -> O {
    match &self.value {
      SignalValue::Sync(v) => {
        let v = v
          .as_ref()
          .downcast_ref::<Mutex<T>>()
          .expect("to downcast signal type");
        f(&v.lock())
      }
      SignalValue::Local(v) => {
        let v = v
          .as_ref()
          .downcast_ref::<TrackedRefCell<T>>()
          .expect("to downcast signal type");
        #[cfg(debug_assertions)]
        {
          f(&v.borrow_at(Location::caller()))
        }
        #[cfg(not(debug_assertions))]
        {
          f(&v.borrow())
        }
      }
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn with<O, T: 'static>(&self, f: impl FnOnce(&T) -> O) -> O {
    self.subscribe();
    self.with_untracked(f)
  }

  pub(crate) fn update_value_sync<U, T: Send + Sync + 'static>(
    &self,
    f: impl FnOnce(&mut T) -> U,
  ) -> U {
    let value = self.as_sync::<T>();
    let mut guard = value.lock();
    let result = f(&mut *guard);
    drop(guard);
    self.run_effects();
    result
  }

  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn update_value_local<U, T: 'static>(&self, f: impl FnOnce(&mut T) -> U) -> U {
    let value = self.as_local::<T>();
    #[cfg(debug_assertions)]
    let mut guard = value.borrow_mut_at(Location::caller());
    #[cfg(not(debug_assertions))]
    let mut guard = value.borrow_mut();
    let result = f(&mut *guard);
    drop(guard);
    self.run_effects();
    result
  }

  pub(crate) fn run_effects(&self) {
    let ids = self.subscribers.snapshot();

    if !ids.is_empty() && matches!(self.value, SignalValue::Sync(_)) {
      Self::warn_if_undesignated();
    }

    let on_ui_thread = Runtime::is_ui_thread();

    if !on_ui_thread {
      SYNC_RUNTIME.enqueue_effects(ids);
      return;
    }

    RUNTIME.with(|r| {
      // Mark before scheduling, so a read that happens before the effects run (inside a
      // batch, or mid-flush) pulls a fresh value instead of a stale cache.
      r.mark_stale(&ids);

      for id in &ids {
        r.add_pending_effect(*id);
      }

      if !r.batching.get() {
        r.run_pending_effects();
      }
    });
  }

  /// Warns once when a `Sync` signal with subscribers runs effects but no UI thread was ever
  /// designated. Without one, an off-thread write routes its effects to the writer's own
  /// runtime and they never reach the UI thread — a silent loss this at least makes loud.
  fn warn_if_undesignated() {
    if Runtime::ui_thread_designated() {
      return;
    }

    static WARNED: std::sync::Once = std::sync::Once::new();

    WARNED.call_once(|| {
      eprintln!(
        "skene-reactive: a Sync signal has subscribed effects but Runtime::init_on_ui_thread() \
         was never called — an off-thread write's effects are silently dropped. Call \
         init_on_ui_thread() during setup."
      );
    });
  }

  pub(crate) fn subscribe(&self) {
    RUNTIME.with(|runtime| {
      if let Some(effect) = runtime.current_effect.borrow().as_ref() {
        self.subscribers.insert(effect.id());
        effect.add_observer(self.id);
      }
    });
  }

  fn as_sync<T: Send + Sync + 'static>(&self) -> Arc<Mutex<T>> {
    match &self.value {
      SignalValue::Sync(v) => v
        .clone()
        .downcast::<Mutex<T>>()
        .expect("to downcast signal type"),
      SignalValue::Local(_) => unreachable!("expected sync signal storage"),
    }
  }

  fn as_local<T: 'static>(&self) -> Rc<TrackedRefCell<T>> {
    match &self.value {
      SignalValue::Local(v) => v
        .clone()
        .downcast::<TrackedRefCell<T>>()
        .expect("to downcast signal type"),
      SignalValue::Sync(_) => unreachable!("expected local signal storage"),
    }
  }
}

// Sync storage trait impls (requires Send + Sync)
impl<T: Clone + Send + Sync> SignalGet<T> for RwSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: Send + Sync> SignalWith<T> for RwSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: Send + Sync> SignalRead<T> for RwSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn try_read(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static,
  {
    self.id().signal().map(|signal| {
      signal.subscribe();
      ReadRef::Sync(SyncReadRef::new(signal.as_sync::<T>()))
    })
  }

  fn try_read_untracked(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static,
  {
    self
      .id()
      .signal()
      .map(|signal| ReadRef::Sync(SyncReadRef::new(signal.as_sync::<T>())))
  }
}

impl<T: Send + Sync> SignalUpdate<T> for RwSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn set(&self, new_value: T)
  where
    T: 'static,
  {
    if let Some(signal) = self.id().signal() {
      signal.update_value_sync(|v| *v = new_value);
    }
  }

  fn update(&self, f: impl FnOnce(&mut T))
  where
    T: 'static,
  {
    if let Some(signal) = self.id().signal() {
      signal.update_value_sync(f);
    }
  }

  fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
  where
    T: 'static,
  {
    self.id().signal().map(|signal| signal.update_value_sync(f))
  }
}

impl<T: Send + Sync> SignalWrite<T> for RwSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn write(&self) -> WriteRef<'_, T>
  where
    T: 'static,
  {
    self.try_write().unwrap()
  }

  fn try_write(&self) -> Option<WriteRef<'_, T>>
  where
    T: 'static,
  {
    self
      .id()
      .signal()
      .map(|signal| WriteRef::Sync(SyncWriteRef::new(signal.id, signal.as_sync::<T>())))
  }
}

impl<T: Clone + Send + Sync> SignalGet<T> for ReadSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: Send + Sync> SignalWith<T> for ReadSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: Send + Sync> SignalRead<T> for ReadSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn try_read(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static,
  {
    self.id().signal().map(|signal| {
      signal.subscribe();
      ReadRef::Sync(SyncReadRef::new(signal.as_sync::<T>()))
    })
  }

  fn try_read_untracked(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static,
  {
    self
      .id()
      .signal()
      .map(|signal| ReadRef::Sync(SyncReadRef::new(signal.as_sync::<T>())))
  }
}

impl<T: Send + Sync> SignalUpdate<T> for WriteSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn set(&self, new_value: T)
  where
    T: 'static,
  {
    if let Some(signal) = self.id().signal() {
      signal.update_value_sync(|v| *v = new_value);
    }
  }

  fn update(&self, f: impl FnOnce(&mut T))
  where
    T: 'static,
  {
    if let Some(signal) = self.id().signal() {
      signal.update_value_sync(f);
    }
  }

  fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
  where
    T: 'static,
  {
    self.id().signal().map(|signal| signal.update_value_sync(f))
  }
}

impl<T: Send + Sync> SignalWrite<T> for WriteSignal<T, SyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn try_write(&self) -> Option<WriteRef<'_, T>>
  where
    T: 'static,
  {
    self
      .id()
      .signal()
      .map(|signal| WriteRef::Sync(SyncWriteRef::new(signal.id, signal.as_sync::<T>())))
  }
}

// Unsync storage trait impls (no Send + Sync required)
impl<T: Clone> SignalGet<T> for RwSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T> SignalWith<T> for RwSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T> SignalRead<T> for RwSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn try_read(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static,
  {
    self.id().local_signal().map(|signal| {
      signal.subscribe();
      ReadRef::Local(LocalReadRef::new(signal.as_local::<T>()))
    })
  }

  fn try_read_untracked(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static,
  {
    self
      .id()
      .local_signal()
      .map(|signal| ReadRef::Local(LocalReadRef::new(signal.as_local::<T>())))
  }
}

impl<T> SignalUpdate<T> for RwSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn set(&self, new_value: T)
  where
    T: 'static,
  {
    if let Some(signal) = self.id().local_signal() {
      signal.update_value_local(|v| *v = new_value);
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn update(&self, f: impl FnOnce(&mut T))
  where
    T: 'static,
  {
    if let Some(signal) = self.id().local_signal() {
      signal.update_value_local(f);
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
  where
    T: 'static,
  {
    self
      .id()
      .local_signal()
      .map(|signal| signal.update_value_local(f))
  }
}

impl<T> SignalWrite<T> for RwSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn write(&self) -> WriteRef<'_, T>
  where
    T: 'static,
  {
    self.try_write().unwrap()
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn try_write(&self) -> Option<WriteRef<'_, T>>
  where
    T: 'static,
  {
    self
      .id()
      .local_signal()
      .map(|signal| WriteRef::Local(LocalWriteRef::new(signal.id, signal.as_local::<T>())))
  }
}

impl<T> SignalUpdate<T> for WriteSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn set(&self, new_value: T)
  where
    T: 'static,
  {
    if let Some(signal) = self.id().local_signal() {
      signal.update_value_local(|v| *v = new_value);
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn update(&self, f: impl FnOnce(&mut T))
  where
    T: 'static,
  {
    if let Some(signal) = self.id().local_signal() {
      signal.update_value_local(f);
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
  where
    T: 'static,
  {
    self
      .id()
      .local_signal()
      .map(|signal| signal.update_value_local(f))
  }
}

impl<T> SignalWrite<T> for WriteSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn try_write(&self) -> Option<WriteRef<'_, T>>
  where
    T: 'static,
  {
    self
      .id()
      .local_signal()
      .map(|signal| WriteRef::Local(LocalWriteRef::new(signal.id, signal.as_local::<T>())))
  }
}

impl<T: Clone> SignalGet<T> for ReadSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T> SignalWith<T> for ReadSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }
}

impl<T> SignalRead<T> for ReadSignal<T, UnsyncStorage> {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn try_read(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static,
  {
    self.id().local_signal().map(|signal| {
      signal.subscribe();
      ReadRef::Local(LocalReadRef::new(signal.as_local::<T>()))
    })
  }

  fn try_read_untracked(&self) -> Option<ReadRef<'_, T>>
  where
    T: 'static,
  {
    self
      .id()
      .local_signal()
      .map(|signal| ReadRef::Local(LocalReadRef::new(signal.as_local::<T>())))
  }
}
