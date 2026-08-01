#[cfg(feature = "test")]
use std::mem::take;
#[cfg(debug_assertions)]
use std::panic::Location;
use std::{
  any::{Any, TypeId},
  cell::{Cell, RefCell},
  cmp::Reverse,
  rc::Rc,
  sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
  },
  thread::{self, ThreadId},
};

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::{
  effect::{EffectPriority, EffectTrait, run_effect},
  id::Id,
  signal::SignalState,
  sync_runtime::SYNC_RUNTIME,
};

/// Type alias for context storage within a scope.
pub(crate) type ScopeContexts = FxHashMap<TypeId, Box<dyn Any>>;

thread_local! {
  pub static RUNTIME: Runtime = Runtime::new();
}

// The designation is a set-once global in production; the `test` feature makes it
// resettable so a multi-threaded test harness can designate each test's thread in turn
// (serialized by the test suite).
#[cfg(not(feature = "test"))]
static UI_THREAD_ID: OnceLock<ThreadId> = OnceLock::new();
#[cfg(feature = "test")]
static UI_THREAD_ID: std::sync::RwLock<Option<ThreadId>> = std::sync::RwLock::new(None);
#[cfg(debug_assertions)]
static UI_THREAD_SET_LOCATION: OnceLock<&'static Location<'static>> = OnceLock::new();
static ENFORCE_UI_THREAD: AtomicBool = AtomicBool::new(false);

/// The designated UI thread, if any.
fn ui_thread_id() -> Option<ThreadId> {
  #[cfg(not(feature = "test"))]
  {
    UI_THREAD_ID.get().copied()
  }

  #[cfg(feature = "test")]
  {
    *UI_THREAD_ID.read().expect("read the UI thread id")
  }
}

/// Designates `current` as the UI thread; returns the existing designation when one is
/// already set to a different thread.
fn set_ui_thread_id(current: ThreadId) -> Result<(), ThreadId> {
  #[cfg(not(feature = "test"))]
  {
    match UI_THREAD_ID.set(current) {
      Ok(()) => Ok(()),
      Err(_) => {
        let existing = *UI_THREAD_ID.get().expect("a set id");

        if existing == current {
          Ok(())
        }
        else {
          Err(existing)
        }
      }
    }
  }

  #[cfg(feature = "test")]
  {
    let mut slot = UI_THREAD_ID.write().expect("write the UI thread id");

    match *slot {
      Some(existing) if existing != current => Err(existing),
      _ => {
        *slot = Some(current);

        Ok(())
      }
    }
  }
}

/// The callbacks registered to run when one scope is disposed.
type Cleanups = Vec<Box<dyn FnOnce()>>;

/// One id's runtime record: whichever of the signal, effect, and scope facets it carries.
#[derive(Default)]
pub(crate) struct Entity {
  pub(crate) signal: Option<SignalState>,
  pub(crate) effect: Option<Rc<dyn EffectTrait>>,
  pub(crate) parent: Option<Id>,
  pub(crate) children: Option<Box<FxHashSet<Id>>>,
  pub(crate) contexts: Option<Box<ScopeContexts>>,
  pub(crate) cleanups: Cleanups,
}

/// Clears the flushing flag when the flush unwinds.
struct FlushGuard<'a>(&'a Cell<bool>);

impl Drop for FlushGuard<'_> {
  fn drop(&mut self) {
    self.0.set(false);
  }
}

/// The internal reactive `Runtime` which stores all the reactive system states in a
/// thread local.
pub struct Runtime {
  pub(crate) current_effect: RefCell<Option<Rc<dyn EffectTrait>>>,
  pub(crate) current_scope: RefCell<Id>,
  /// Every live id's record, keyed once for all of its facets.
  pub(crate) entities: RefCell<FxHashMap<Id, Entity>>,
  pub(crate) batching: Cell<bool>,
  pub(crate) pending_effects: RefCell<SmallVec<[Id; 10]>>,
  pending_effects_set: RefCell<FxHashSet<Id>>,
  /// Whether a flush is currently draining; a write during it enqueues instead of
  /// re-entering the flush.
  flushing: Cell<bool>,
}

impl Default for Runtime {
  fn default() -> Self {
    Self::new()
  }
}

impl Runtime {
  pub(crate) fn new() -> Self {
    Self {
      current_effect: RefCell::new(None),
      current_scope: RefCell::new(Id::next()),
      entities: RefCell::new(FxHashMap::default()),
      batching: Cell::new(false),
      pending_effects: RefCell::new(SmallVec::new()),
      pending_effects_set: RefCell::new(FxHashSet::default()),
      flushing: Cell::new(false),
    }
  }

  /// Records the calling thread as the UI thread; call once during initialization.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn init_on_ui_thread() {
    let current = thread::current().id();

    assert!(
      set_ui_thread_id(current).is_ok(),
      "UI thread id already set to a different thread"
    );

    #[cfg(debug_assertions)]
    {
      let caller = Location::caller();
      let _ = UI_THREAD_SET_LOCATION.set(caller);
    }

    ENFORCE_UI_THREAD.store(true, Ordering::Relaxed);
  }

  /// Panics if enforcement is on and the caller is not the UI thread.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn assert_ui_thread() {
    if !ENFORCE_UI_THREAD.load(Ordering::Relaxed) {
      return;
    }

    let current = thread::current().id();

    match ui_thread_id() {
      Some(ui_id) => {
        if ui_id != current {
          #[cfg(debug_assertions)]
          {
            let caller = Location::caller();
            let set_at = UI_THREAD_SET_LOCATION.get();
            let set_info = set_at
              .map(|loc| format!(" (set at {}:{})", loc.file(), loc.line()))
              .unwrap_or_default();

            panic!(
              "Unsync runtime access from non-UI thread\n  expected UI thread: {:?}{}\n  current thread: {:?}\n  caller: {}:{}",
              ui_id,
              set_info,
              current,
              caller.file(),
              caller.line(),
            );
          }
          #[cfg(not(debug_assertions))]
          {
            assert_eq!(
              ui_id, current,
              "Unsync runtime access from non-UI thread: expected {:?}, got {:?}",
              ui_id, current
            );
          }
        }
      }
      None => {
        // Once enforcement is on, first access defines the UI thread.
        let _ = set_ui_thread_id(current);
      }
    }
  }

  /// Whether a UI thread has been designated via
  /// [`init_on_ui_thread`](Self::init_on_ui_thread).
  ///
  /// When it has not, an off-thread `Sync` signal write cannot be routed to the UI thread and
  /// its effects are silently dropped—the case [`SignalState::warn_if_undesignated`] flags.
  pub(crate) fn ui_thread_designated() -> bool {
    ENFORCE_UI_THREAD.load(Ordering::Relaxed)
  }

  /// Whether the caller is the UI thread, or `true` when enforcement is off.
  pub fn is_ui_thread() -> bool {
    if !ENFORCE_UI_THREAD.load(Ordering::Relaxed) {
      true
    }
    else {
      ui_thread_id().is_some_and(|thread| thread == thread::current().id())
    }
  }

  /// Resets the calling thread's reactive runtime to a fresh, empty state.
  ///
  /// Clears all signals, effects, scopes, and pending work. Available only with
  /// the `test` feature.
  #[cfg(feature = "test")]
  pub fn reset() {
    // Take the entities out and drop them after every borrow is released: an effect's
    // `Drop` re-enters the runtime (`remove_effect`, `dispose`), and a cleanup may own
    // anything at all, so clearing the map in place could borrow it twice. Reset discards
    // cleanups rather than running them: it is a teardown to a blank runtime, not a
    // graceful disposal.
    let entities = RUNTIME.with(|runtime| {
      *runtime.current_effect.borrow_mut() = None;
      *runtime.current_scope.borrow_mut() = Id::next();
      runtime.batching.set(false);
      runtime.pending_effects.borrow_mut().clear();
      runtime.pending_effects_set.borrow_mut().clear();
      runtime.flushing.set(false);

      take(&mut *runtime.entities.borrow_mut())
    });

    drop(entities);

    #[cfg(feature = "snapshot")]
    crate::snapshot::clear();

    // Release the process-wide UI-thread designation, so the next (serialized) test can
    // designate its own thread.
    *UI_THREAD_ID.write().expect("write the UI thread id") = None;
    ENFORCE_UI_THREAD.store(false, Ordering::Relaxed);
  }

  pub(crate) fn register_effect(&self, effect: &Rc<dyn EffectTrait>) {
    self
      .entities
      .borrow_mut()
      .entry(effect.id())
      .or_default()
      .effect = Some(effect.clone());
  }

  pub(crate) fn remove_effect(&self, id: Id) {
    if let Some(entity) = self.entities.borrow_mut().get_mut(&id) {
      entity.effect = None;
    }
  }

  pub(crate) fn get_effect(&self, id: Id) -> Option<Rc<dyn EffectTrait>> {
    self
      .entities
      .borrow()
      .get(&id)
      .and_then(|entity| entity.effect.clone())
  }

  pub(crate) fn add_pending_effect(&self, effect_id: Id) {
    let mut set = self.pending_effects_set.borrow_mut();
    if set.insert(effect_id) {
      self.pending_effects.borrow_mut().push(effect_id);
    }
  }

  /// Marks every stale-able effect among `ids` stale, propagating through each one's
  /// output signal to its own subscribers—so a read anywhere downstream pulls a fresh
  /// value before the scheduled re-runs happen.
  pub(crate) fn mark_stale(&self, ids: &[Id]) {
    let mut stack: SmallVec<[Id; 8]> = SmallVec::from_slice(ids);

    while let Some(id) = stack.pop() {
      let Some(effect) = self.get_effect(id)
      else {
        continue;
      };

      let Some(downstream) = effect.mark_stale()
      else {
        continue;
      };

      let subscribers = self
        .entities
        .borrow()
        .get(&downstream)
        .and_then(|entity| entity.signal.as_ref())
        .map(|signal| signal.subscribers.snapshot());

      if let Some(subscribers) = subscribers {
        stack.extend(subscribers);
      }
    }
  }

  /// Drains the pending effects until none remain. Re-entrant calls return immediately:
  /// the running flush picks up whatever a mid-flush write enqueued.
  pub(crate) fn run_pending_effects(&self) {
    if self.flushing.get() {
      return;
    }

    self.flushing.set(true);

    let _guard = FlushGuard(&self.flushing);

    loop {
      let mut pending_effects = self.pending_effects.take();

      if pending_effects.is_empty() {
        break;
      }

      pending_effects.sort_by_key(|id| {
        let priority = self
          .get_effect(*id)
          .map(|effect| effect.priority())
          .unwrap_or(EffectPriority::Normal);

        (Reverse(priority), *id)
      });

      for effect_id in pending_effects {
        self.pending_effects_set.borrow_mut().remove(&effect_id);

        if let Some(effect) = self.get_effect(effect_id) {
          run_effect(effect);
        }
      }
    }
  }

  /// Drains any queued work from the sync runtime and runs pending UI effects.
  pub fn drain_pending_work() {
    Runtime::assert_ui_thread();
    let pending_effects = SYNC_RUNTIME.take_pending_effects();
    let pending_disposals = SYNC_RUNTIME.take_pending_disposals();

    RUNTIME.with(|runtime| {
      runtime.mark_stale(&pending_effects);

      for id in pending_effects {
        runtime.add_pending_effect(id);
      }

      for id in pending_disposals {
        id.dispose();
      }

      runtime.run_pending_effects();
    });
  }

  /// Returns true if there is queued work for this runtime or the sync runtime.
  pub fn has_pending_work() -> bool {
    RUNTIME.with(|runtime| !runtime.pending_effects.borrow().is_empty())
      || SYNC_RUNTIME.has_pending_effects()
      || SYNC_RUNTIME.has_pending_disposals()
  }

  /// Sets a waker that will be called when a sync signal is updated off the UI thread.
  /// The waker should nudge the UI event loop (e.g., by sending a proxy event).
  pub fn set_sync_effect_waker(waker: impl Fn() + Send + Sync + 'static) {
    SYNC_RUNTIME.set_waker(waker);
  }

  /// The effect currently tracking signal reads, if any.
  pub fn get_current_effect() -> Option<Rc<dyn EffectTrait>> {
    RUNTIME.with(|rt| rt.current_effect.borrow().clone())
  }

  /// Sets the effect that tracks signal reads.
  pub fn set_current_effect(effect: Option<Rc<dyn EffectTrait>>) {
    RUNTIME.with(|rt| *rt.current_effect.borrow_mut() = effect);
  }

  /// Runs `f` with `effect` as the tracking effect, restoring the previous one
  /// afterwards.
  pub fn with_effect<T>(effect: Option<Rc<dyn EffectTrait>>, f: impl FnOnce() -> T) -> T {
    struct EffectRestoreGuard(Option<Rc<dyn EffectTrait>>);

    impl Drop for EffectRestoreGuard {
      fn drop(&mut self) {
        Runtime::set_current_effect(self.0.clone());
      }
    }

    let saved_effect = Runtime::get_current_effect();
    let _restore = EffectRestoreGuard(saved_effect);
    Runtime::set_current_effect(effect);
    f()
  }
}
