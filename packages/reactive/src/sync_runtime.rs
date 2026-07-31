use std::{
  any::Any,
  mem::take,
  sync::{Arc, LazyLock, OnceLock, RwLock},
};

use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  id::Id,
  signal::{SignalState, SignalValue, Subscribers},
};

pub(crate) static SYNC_RUNTIME: LazyLock<SyncRuntime> = LazyLock::new(SyncRuntime::new);

/// Sync-only signal representation for the global runtime.
#[derive(Clone)]
pub(crate) struct SyncSignal {
  pub(crate) id: Id,
  pub(crate) value: Arc<dyn Any + Send + Sync>,
  pub(crate) subscribers: Arc<Mutex<FxHashSet<Id>>>,
}

impl From<SyncSignal> for SignalState {
  fn from(sync: SyncSignal) -> Self {
    SignalState::from_parts(
      sync.id,
      SignalValue::Sync(sync.value),
      Subscribers::Sync(sync.subscribers),
    )
  }
}

/// Global runtime for sync signals (shared across threads).
pub(crate) struct SyncRuntime {
  signals: RwLock<FxHashMap<Id, SyncSignal>>,
  pending_effects: Mutex<Vec<Id>>,
  pending_disposals: Mutex<Vec<Id>>,
  waker: OnceLock<Arc<dyn Fn() + Send + Sync>>,
}

impl SyncRuntime {
  pub(crate) fn new() -> Self {
    Self {
      signals: RwLock::new(FxHashMap::default()),
      pending_effects: Mutex::new(Vec::new()),
      pending_disposals: Mutex::new(Vec::new()),
      waker: OnceLock::new(),
    }
  }

  pub(crate) fn get_signal(&self, id: &Id) -> Option<SyncSignal> {
    self.signals.read().ok()?.get(id).cloned()
  }

  pub(crate) fn insert_signal(&self, id: Id, signal: SyncSignal) {
    if let Ok(mut signals) = self.signals.write() {
      signals.insert(id, signal);
    }
  }

  pub(crate) fn remove_signal(&self, id: &Id) -> Option<SyncSignal> {
    self.signals.write().ok()?.remove(id)
  }

  pub(crate) fn enqueue_effects(&self, ids: impl IntoIterator<Item = Id>) {
    let waker = {
      let mut queue = self.pending_effects.lock();
      queue.extend(ids);
      self.waker.get().cloned()
    };

    if let Some(waker) = waker {
      waker();
    }
  }

  pub(crate) fn enqueue_disposals(&self, ids: impl IntoIterator<Item = Id>) {
    let waker = {
      let mut queue = self.pending_disposals.lock();
      queue.extend(ids);
      self.waker.get().cloned()
    };

    if let Some(waker) = waker {
      waker();
    }
  }

  pub(crate) fn take_pending_effects(&self) -> Vec<Id> {
    take(&mut *self.pending_effects.lock())
  }

  pub(crate) fn take_pending_disposals(&self) -> Vec<Id> {
    take(&mut *self.pending_disposals.lock())
  }

  pub(crate) fn has_pending_effects(&self) -> bool {
    !self.pending_effects.lock().is_empty()
  }

  pub(crate) fn has_pending_disposals(&self) -> bool {
    !self.pending_disposals.lock().is_empty()
  }

  pub(crate) fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
    let _ = self.waker.set(Arc::new(waker));
  }
}
