use std::{
  any::Any,
  mem::take,
  sync::{Arc, LazyLock, OnceLock, RwLock},
};

use parking_lot::Mutex;
use rustc_hash::FxHashSet;

use crate::{
  id::Id,
  signal::{SignalState, SignalValue, Subscribers},
};

pub(crate) static SYNC: LazyLock<SyncRegistry> = LazyLock::new(SyncRegistry::new);

/// One sync signal's record: its value, its UI-thread subscriber effects, and the
/// local owner it was created under, if any.
#[derive(Clone)]
pub(crate) struct SyncSignal {
  pub(crate) id: Id,
  pub(crate) value: Arc<dyn Any + Send + Sync>,
  pub(crate) subscribers: Arc<Mutex<FxHashSet<Id>>>,
  pub(crate) parent: Option<Id>,
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

/// One generational slot of the sync slab.
struct SyncSlot {
  generation: u16,
  signal: Option<SyncSignal>,
}

/// The generational slab holding every sync signal.
#[derive(Default)]
struct SyncSlab {
  slots: Vec<SyncSlot>,
  free: Vec<u32>,
}

impl SyncSlab {
  /// Allocates a slot and returns the id minted for it.
  fn allocate(&mut self) -> Id {
    if let Some(index) = self.free.pop() {
      return Id::sync(self.slots[index as usize].generation, index);
    }

    let index = self.slots.len() as u32;

    self.slots.push(SyncSlot {
      generation: 0,
      signal: None,
    });

    Id::sync(0, index)
  }

  fn get(&self, id: Id) -> Option<&SyncSignal> {
    let slot = self.slots.get(id.index() as usize)?;

    if slot.generation != id.generation() {
      return None;
    }

    slot.signal.as_ref()
  }

  fn remove(&mut self, id: Id) -> Option<SyncSignal> {
    let index = id.index();
    let slot = self.slots.get_mut(index as usize)?;

    if slot.generation != id.generation() {
      return None;
    }

    let signal = slot.signal.take()?;

    slot.generation = slot.generation.wrapping_add(1);

    // Retire the slot once its generations are spent; a wrapped generation would
    // resurrect long-stale ids.
    if slot.generation != 0 {
      self.free.push(index);
    }

    Some(signal)
  }
}

/// The process-wide registry for sync signals, shared across threads, plus the queues
/// carrying off-thread work to the UI thread.
pub(crate) struct SyncRegistry {
  slab: RwLock<SyncSlab>,
  pending_effects: Mutex<Vec<Id>>,
  pending_disposals: Mutex<Vec<Id>>,
  waker: OnceLock<Arc<dyn Fn() + Send + Sync>>,
}

impl SyncRegistry {
  fn new() -> Self {
    Self {
      slab: RwLock::new(SyncSlab::default()),
      pending_effects: Mutex::new(Vec::new()),
      pending_disposals: Mutex::new(Vec::new()),
      waker: OnceLock::new(),
    }
  }

  /// Inserts a sync signal built from `value` and `parent`, returning its id.
  pub(crate) fn insert(&self, value: Arc<dyn Any + Send + Sync>, parent: Option<Id>) -> Option<Id> {
    let mut slab = self.slab.write().ok()?;
    let id = slab.allocate();
    let slot = &mut slab.slots[id.index() as usize];

    slot.signal = Some(SyncSignal {
      id,
      value,
      subscribers: Arc::new(Mutex::new(FxHashSet::default())),
      parent,
    });

    Some(id)
  }

  pub(crate) fn get(&self, id: Id) -> Option<SyncSignal> {
    self.slab.read().ok()?.get(id).cloned()
  }

  pub(crate) fn remove(&self, id: Id) -> Option<SyncSignal> {
    self.slab.write().ok()?.remove(id)
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
