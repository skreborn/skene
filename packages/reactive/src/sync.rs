use std::{
  any::Any,
  mem::take,
  sync::{
    Arc,
    LazyLock,
    OnceLock,
    RwLock,
    atomic::{AtomicBool, Ordering},
  },
};

use parking_lot::Mutex;

use crate::{
  id::Id,
  signal::{SignalState, SignalValue},
};

pub(crate) static SYNC: LazyLock<SyncRegistry> = LazyLock::new(SyncRegistry::new);

/// One sync signal's record: its value, the local owner it was created under, if any,
/// and whether a UI-thread effect ever subscribed to it.
pub(crate) struct SyncSignal {
  pub(crate) value: Arc<dyn Any + Send + Sync>,
  pub(crate) parent: Option<Id>,
  subscribed: AtomicBool,
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
  pending_writes: Mutex<Vec<Id>>,
  pending_disposals: Mutex<Vec<(Id, Option<Id>)>>,
  waker: OnceLock<Arc<dyn Fn() + Send + Sync>>,
}

impl SyncRegistry {
  fn new() -> Self {
    Self {
      slab: RwLock::new(SyncSlab::default()),
      pending_writes: Mutex::new(Vec::new()),
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
      value,
      parent,
      subscribed: AtomicBool::new(false),
    });

    Some(id)
  }

  /// The signal `id` names, as a state the caller can act on without the lock.
  pub(crate) fn get(&self, id: Id) -> Option<SignalState> {
    let slab = self.slab.read().ok()?;
    let signal = slab.get(id)?;

    Some(SignalState {
      id,
      value: SignalValue::Sync(signal.value.clone()),
    })
  }

  pub(crate) fn remove(&self, id: Id) -> Option<SyncSignal> {
    self.slab.write().ok()?.remove(id)
  }

  /// Records that a UI-thread effect subscribed to `id`.
  pub(crate) fn mark_subscribed(&self, id: Id) {
    if let Ok(slab) = self.slab.read()
      && let Some(signal) = slab.get(id)
    {
      signal.subscribed.store(true, Ordering::Relaxed);
    }
  }

  /// Whether a UI-thread effect ever subscribed to `id`.
  pub(crate) fn is_subscribed(&self, id: Id) -> bool {
    self
      .slab
      .read()
      .ok()
      .and_then(|slab| {
        slab
          .get(id)
          .map(|signal| signal.subscribed.load(Ordering::Relaxed))
      })
      .unwrap_or(false)
  }

  /// Queues an off-thread write for the UI thread and wakes it.
  pub(crate) fn enqueue_write(&self, id: Id) {
    self.pending_writes.lock().push(id);
    self.wake();
  }

  /// Queues off-thread disposals for the UI thread and wakes it.
  pub(crate) fn enqueue_disposals(&self, ids: impl IntoIterator<Item = (Id, Option<Id>)>) {
    self.pending_disposals.lock().extend(ids);
    self.wake();
  }

  fn wake(&self) {
    if let Some(waker) = self.waker.get() {
      waker();
    }
  }

  pub(crate) fn take_pending_writes(&self) -> Vec<Id> {
    take(&mut *self.pending_writes.lock())
  }

  pub(crate) fn take_pending_disposals(&self) -> Vec<(Id, Option<Id>)> {
    take(&mut *self.pending_disposals.lock())
  }

  pub(crate) fn has_pending_work(&self) -> bool {
    !self.pending_writes.lock().is_empty() || !self.pending_disposals.lock().is_empty()
  }

  pub(crate) fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
    let _ = self.waker.set(Arc::new(waker));
  }
}
