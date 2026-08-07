use std::{any::Any, cell::RefCell, marker::PhantomData, rc::Rc};

use rustc_hash::FxHashSet;

use crate::{
  id::Id,
  owner::Owner,
  runtime::{RUNTIME, Runtime},
};

/// The order in which an effect runs relative to others in the same flush.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) enum EffectPriority {
  /// Runs after every high-priority effect.
  #[default]
  Normal,
  /// Runs before every normal-priority effect.
  High,
}

/// A type-erased running effect the runtime schedules and re-runs.
pub(crate) trait AnyEffect: Any {
  /// The effect's identifier.
  fn id(&self) -> Id;

  /// Runs the effect.
  fn run(&self);

  /// Records `id` as a signal the effect subscribed to on its last run.
  fn add_observer(&self, id: Id);

  /// Clears and returns the effect's subscribed signals.
  fn clear_observers(&self) -> Vec<Id>;

  /// Marks the effect's cached output stale, returning the signal carrying that output
  /// so the staleness propagates to its subscribers. `None` when there is nothing
  /// cached downstream, or when the output was already stale.
  fn mark_stale(&self) -> Option<Id> {
    None
  }

  /// The effect's scheduling priority.
  fn priority(&self) -> EffectPriority {
    EffectPriority::Normal
  }

  /// The effect as a `dyn Any` for downcasting.
  fn as_any(&self) -> &dyn Any;
}

/// A side effect that re-runs whenever a signal it read changes.
pub struct Effect<T, F>
where
  T: 'static,
  F: Fn(Option<T>) -> T,
{
  id: Id,
  f: F,
  value: RefCell<Option<T>>,
  observers: RefCell<FxHashSet<Id>>,
  ty: PhantomData<T>,
}

impl<T, F> Drop for Effect<T, F>
where
  T: 'static,
  F: Fn(Option<T>) -> T,
{
  fn drop(&mut self) {
    if RUNTIME
      .try_with(|runtime| runtime.remove_effect(self.id))
      .is_ok()
    {
      self.id.dispose();
    }
  }
}

impl<T, F> Effect<T, F>
where
  T: Any + 'static,
  F: Fn(Option<T>) -> T + 'static,
{
  /// Runs `f` immediately and re-runs it whenever a signal it read changes. `f`
  /// receives its previous return value, or `None` on the first run. The effect lives
  /// until the current [`Owner`] is disposed.
  #[expect(clippy::new_ret_no_self)]
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(f: F) {
    Runtime::assert_ui_thread();

    let id = Runtime::allocate();
    let effect: Rc<dyn AnyEffect> = Rc::new(Self {
      id,
      f,
      value: RefCell::new(None),
      observers: RefCell::new(FxHashSet::default()),
      ty: PhantomData,
    });

    id.set_owner();
    RUNTIME.with(|runtime| runtime.register_effect(&effect));
    run_initial_effect(effect);
  }
}

impl Effect<(), fn(Option<()>) -> ()> {
  /// Runs `compute` immediately and returns its value; whenever a signal it read
  /// changes, recomputes and passes the fresh value to `on_change`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn watch<R>(compute: impl Fn() -> R + 'static, on_change: impl Fn(R) + 'static) -> R
  where
    R: 'static,
  {
    Runtime::assert_ui_thread();

    let id = Runtime::allocate();
    let effect = Rc::new(WatchEffect {
      id,
      compute,
      on_change,
      observers: RefCell::new(FxHashSet::default()),
      ty: PhantomData,
    });

    id.set_owner();

    let effect_dyn: Rc<dyn AnyEffect> = effect.clone();

    RUNTIME.with(|runtime| runtime.register_effect(&effect_dyn));

    Runtime::with_effect(Some(effect_dyn), || {
      Owner::from_id(id).enter(|| (effect.compute)())
    })
  }
}

/// Runs `f` without subscribing the current effect to any signal it reads.
#[cfg_attr(debug_assertions, track_caller)]
pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
  Runtime::assert_ui_thread();
  Runtime::with_effect(None, f)
}

/// Runs `f`, deferring dependent effects until it returns so a batch of signal writes
/// triggers each effect at most once.
#[cfg_attr(debug_assertions, track_caller)]
pub fn batch<T>(f: impl FnOnce() -> T) -> T {
  let already_batching = RUNTIME.with(|runtime| {
    let batching = runtime.batching.get();

    if !batching {
      runtime.batching.set(true);
    }

    batching
  });

  let result = f();

  if !already_batching {
    RUNTIME.with(|runtime| {
      runtime.batching.set(false);
      runtime.run_pending_effects();
    });
  }

  result
}

fn run_initial_effect(effect: Rc<dyn AnyEffect>) {
  Runtime::assert_ui_thread();

  let effect_id = effect.id();

  Runtime::with_effect(Some(effect.clone()), || {
    Owner::from_id(effect_id).enter(|| {
      effect.run();
    });
  });
}

pub(crate) fn run_effect(effect: Rc<dyn AnyEffect>) {
  Runtime::assert_ui_thread();

  let effect_id = effect.id();

  effect_id.dispose_children();
  observer_clean_up(&effect);

  Runtime::with_effect(Some(effect.clone()), || {
    Owner::from_id(effect_id).enter(|| {
      effect.run();
    });
  });
}

/// Drops `effect` from the subscriber set of every signal it was observing.
pub(crate) fn observer_clean_up(effect: &Rc<dyn AnyEffect>) {
  let effect_id = effect.id();
  let observers = effect.clear_observers();

  RUNTIME.with(|runtime| {
    let arena = runtime.arena.borrow();

    for observer in observers {
      if observer.is_sync() {
        if let Some(signal) = crate::sync::SYNC.get(observer) {
          signal.subscribers.lock().remove(&effect_id);
        }
      }
      else if let Some(signal) = arena
        .get(observer)
        .and_then(|entity| entity.signal.as_ref())
      {
        signal.subscribers.remove(effect_id);
      }
    }
  });
}

impl<T, F> AnyEffect for Effect<T, F>
where
  T: 'static,
  F: Fn(Option<T>) -> T + 'static,
{
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn run(&self) {
    let previous = self.value.borrow_mut().take();
    let value = (self.f)(previous);

    *self.value.borrow_mut() = Some(value);
  }

  fn add_observer(&self, id: Id) {
    self.observers.borrow_mut().insert(id);
  }

  fn clear_observers(&self) -> Vec<Id> {
    self.observers.borrow_mut().drain().collect()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

/// The effect behind [`Effect::watch`]: recompute, then hand the value on.
struct WatchEffect<R, C, U> {
  id: Id,
  compute: C,
  on_change: U,
  observers: RefCell<FxHashSet<Id>>,
  ty: PhantomData<R>,
}

impl<R, C, U> AnyEffect for WatchEffect<R, C, U>
where
  R: 'static,
  C: Fn() -> R + 'static,
  U: Fn(R) + 'static,
{
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn run(&self) {
    (self.on_change)((self.compute)());
  }

  fn add_observer(&self, id: Id) {
    self.observers.borrow_mut().insert(id);
  }

  fn clear_observers(&self) -> Vec<Id> {
    self.observers.borrow_mut().drain().collect()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl<R, C, U> Drop for WatchEffect<R, C, U> {
  fn drop(&mut self) {
    if RUNTIME
      .try_with(|runtime| runtime.remove_effect(self.id))
      .is_ok()
    {
      self.id.dispose();
    }
  }
}

/// Re-runs a callback whenever a signal read during the last [`track`](Self::track)
/// call changes, until the reaction is dropped.
#[must_use]
pub struct Reaction {
  id: std::cell::Cell<Option<Id>>,
  on_invalidate: Rc<dyn Fn()>,
}

impl Drop for Reaction {
  fn drop(&mut self) {
    if let Some(id) = self.id.take() {
      id.dispose();
    }
  }
}

impl Reaction {
  /// Creates a reaction that runs `on_invalidate` when a tracked signal changes.
  pub fn new(on_invalidate: impl Fn() + 'static) -> Self {
    Self {
      id: std::cell::Cell::new(None),
      on_invalidate: Rc::new(on_invalidate),
    }
  }

  /// Runs `f` and returns its value, subscribing to every signal it reads so a later
  /// change re-runs the reaction's callback. Replaces any previous subscription.
  pub fn track<T: 'static>(&self, f: impl FnOnce() -> T) -> T {
    Runtime::assert_ui_thread();

    if let Some(previous) = self.id.take() {
      previous.dispose();
    }

    let id = Runtime::allocate();

    self.id.set(Some(id));

    let effect: Rc<dyn AnyEffect> = Rc::new(ReactionEffect {
      id,
      observers: RefCell::new(FxHashSet::default()),
      on_invalidate: self.on_invalidate.clone(),
    });

    RUNTIME.with(|runtime| runtime.register_effect(&effect));

    Runtime::with_effect(Some(effect), || Owner::from_id(id).enter(f))
  }
}

struct ReactionEffect {
  id: Id,
  observers: RefCell<FxHashSet<Id>>,
  on_invalidate: Rc<dyn Fn()>,
}

impl AnyEffect for ReactionEffect {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn run(&self) {
    (self.on_invalidate)();
  }

  fn add_observer(&self, id: Id) {
    self.observers.borrow_mut().insert(id);
  }

  fn clear_observers(&self) -> Vec<Id> {
    self.observers.borrow_mut().drain().collect()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
