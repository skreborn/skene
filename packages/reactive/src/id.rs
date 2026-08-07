use std::sync::atomic::{AtomicU16, Ordering};

use crate::{
  effect::observer_clean_up,
  runtime::{RUNTIME, Runtime},
  signal::SignalState,
  sync::SYNC,
};

/// Identifies one reactive entity: a signal, an effect, or an [`Owner`](crate::Owner).
///
/// An id names a generational slot in its allocating runtime—the calling thread's for
/// local entities, the process-wide registry for sync signals. A stale id (its slot
/// freed or reused, or another thread's runtime) resolves to nothing rather than to a
/// different entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct Id(u64);

const SYNC_BIT: u64 = 1 << 63;
const NONCE_MASK: u16 = 0x7FFF;

/// The next local runtime's nonce; distinguishes ids across threads and resets.
static NEXT_NONCE: AtomicU16 = AtomicU16::new(0);

/// Allocates a nonce for a fresh local runtime.
pub(crate) fn next_nonce() -> u16 {
  NEXT_NONCE.fetch_add(1, Ordering::Relaxed) & NONCE_MASK
}

impl Id {
  /// Packs a local id from its runtime's nonce, slot generation, and slot index.
  #[inline(always)]
  pub(crate) const fn local(nonce: u16, generation: u16, index: u32) -> Self {
    Self((((nonce & NONCE_MASK) as u64) << 48) | ((generation as u64) << 32) | index as u64)
  }

  /// Packs a sync id from its slot generation and slot index.
  #[inline(always)]
  pub(crate) const fn sync(generation: u16, index: u32) -> Self {
    Self(SYNC_BIT | ((generation as u64) << 32) | index as u64)
  }

  /// Whether this id names a sync signal rather than a local entity.
  #[inline(always)]
  pub(crate) const fn is_sync(self) -> bool {
    self.0 & SYNC_BIT != 0
  }

  /// The allocating local runtime's nonce.
  #[inline(always)]
  pub(crate) const fn nonce(self) -> u16 {
    ((self.0 >> 48) as u16) & NONCE_MASK
  }

  /// The slot generation this id was minted for.
  #[inline(always)]
  pub(crate) const fn generation(self) -> u16 {
    (self.0 >> 32) as u16
  }

  /// The slot index in the allocating runtime.
  #[inline(always)]
  pub(crate) const fn index(self) -> u32 {
    self.0 as u32
  }

  /// The signal state this id names, resolved in the id's own realm.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn state(self) -> Option<SignalState> {
    if self.is_sync() {
      return SYNC.get(self).map(Into::into);
    }

    Runtime::assert_ui_thread();

    RUNTIME.with(|runtime| {
      runtime
        .arena
        .borrow()
        .get(self)
        .and_then(|entity| entity.signal.clone())
    })
  }

  /// Attaches this id to the current owner, so disposing the owner disposes it.
  pub(crate) fn set_owner(self) {
    RUNTIME.with(|runtime| {
      let owner = runtime.current_owner.get();
      let mut arena = runtime.arena.borrow_mut();

      if let Some(entity) = arena.get_mut(owner) {
        entity.children.get_or_insert_default().insert(self);
      }

      if !self.is_sync()
        && let Some(entity) = arena.get_mut(self)
      {
        entity.parent = Some(owner);
      }
    });
  }

  /// Disposes only the children of this id, keeping the entity itself.
  pub(crate) fn dispose_children(self) {
    let children = RUNTIME.try_with(|runtime| {
      runtime
        .arena
        .borrow_mut()
        .get_mut(self)
        .and_then(|entity| entity.children.take())
    });

    if let Ok(Some(children)) = children {
      for child in *children {
        child.dispose();
      }
    }
  }

  /// Disposes the entity this id names and every descendant.
  pub(crate) fn dispose(self) {
    if self.is_sync() {
      Self::dispose_sync(self);

      return;
    }

    if !Runtime::is_ui_thread() {
      // Local disposal must run on the entity's own thread; route it there.
      SYNC.enqueue_disposals([self]);

      return;
    }

    let Ok(entity) = RUNTIME.try_with(|runtime| {
      let mut arena = runtime.arena.borrow_mut();
      let entity = arena.remove(self);

      // Unlink from the parent, so a long-lived owner's children set does not
      // accumulate disposed ids.
      if let Some(parent) = entity.as_ref().and_then(|entity| entity.parent)
        && let Some(parent_entity) = arena.get_mut(parent)
        && let Some(children) = parent_entity.children.as_mut()
      {
        children.remove(&self);
      }

      entity
    })
    else {
      return;
    };

    let Some(entity) = entity
    else {
      return;
    };

    if let Some(children) = entity.children {
      for child in *children {
        child.dispose();
      }
    }

    // After the children, so a parent's cleanup sees a fully torn-down subtree. Reverse
    // registration order, so a later callback unwinds before what it was built on.
    for cleanup in entity.cleanups.into_iter().rev() {
      cleanup();
    }

    if let Some(effect) = entity.effect {
      observer_clean_up(&effect);
    }

    if let Some(signal) = entity.signal {
      // Snapshot first: dispose may re-enter the subscriber set.
      for subscriber in signal.subscribers.snapshot() {
        subscriber.dispose();
      }
    }
  }

  /// Removes a sync signal and disposes its subscribers—UI-thread effects—on the UI
  /// thread, directly or through the pending-disposal queue.
  fn dispose_sync(id: Id) {
    let Some(signal) = SYNC.remove(id)
    else {
      return;
    };

    let subscribers = signal
      .subscribers
      .lock()
      .iter()
      .copied()
      .collect::<Vec<_>>();

    if Runtime::is_ui_thread() {
      if let Some(parent) = signal.parent {
        let _ = RUNTIME.try_with(|runtime| {
          if let Some(entity) = runtime.arena.borrow_mut().get_mut(parent)
            && let Some(children) = entity.children.as_mut()
          {
            children.remove(&id);
          }
        });
      }

      for subscriber in subscribers {
        subscriber.dispose();
      }
    }
    else {
      SYNC.enqueue_disposals(subscribers);
    }
  }
}
