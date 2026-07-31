use std::{any::Any, cell::RefCell, marker::PhantomData, rc::Rc};

use rustc_hash::FxHashSet;

use crate::{
  id::Id,
  runtime::{RUNTIME, Runtime},
  scope::Scope,
  sync_runtime::SYNC_RUNTIME,
};

/// The order in which an effect runs relative to others in the same batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum EffectPriority {
  /// Runs after every high-priority effect.
  #[default]
  Normal,
  /// Runs before every normal-priority effect.
  High,
}

/// A type-erased running effect the runtime schedules and re-runs.
pub trait EffectTrait: Any {
  /// The effect's identifier.
  fn id(&self) -> Id;

  /// Runs the effect, returning whether its value changed.
  fn run(&self) -> bool;

  /// Records `id` as a signal the effect subscribed to on its last run.
  fn add_observer(&self, id: Id);

  /// Clears and returns the effect's subscribed signals.
  fn clear_observers(&self) -> Vec<Id>;

  /// Marks the effect's cached output stale, returning the signal carrying that output so
  /// the staleness propagates to its subscribers. `None` when there is nothing cached
  /// downstream, or when the output was already stale.
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

/// A handle for a running effect. Prefer `Effect::new` over `Scope::create_effect`.
pub struct Effect<T, F>
where
  T: 'static,
  F: Fn(Option<T>) -> T,
{
  id: Id,
  f: F,
  value: RefCell<Option<T>>,
  observers: RefCell<FxHashSet<Id>>,
  ts: PhantomData<()>,
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
  /// receives its previous return value, or `None` on the first run.
  #[expect(clippy::new_ret_no_self)]
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(f: F) {
    Runtime::assert_ui_thread();
    let id = Id::next();
    let effect: Rc<dyn EffectTrait> = Rc::new(Self {
      id,
      f,
      value: RefCell::new(None),
      observers: RefCell::new(FxHashSet::default()),
      ts: PhantomData,
    });

    id.set_scope();
    RUNTIME.with(|runtime| runtime.register_effect(&effect));

    run_initial_effect(effect);
  }
}

/// An internal updater-effect handle, created via `Scope::create_updater`.
pub struct UpdaterEffect<T, I, C, U> {
  id: Id,
  compute: C,
  on_change: U,
  value: RefCell<Option<T>>,
  observers: RefCell<FxHashSet<Id>>,
  _phantom: PhantomData<I>,
}

impl<T, I, C, U> Drop for UpdaterEffect<T, I, C, U> {
  fn drop(&mut self) {
    if RUNTIME
      .try_with(|runtime| runtime.remove_effect(self.id))
      .is_ok()
    {
      self.id.dispose();
    }
  }
}

impl UpdaterEffect<(), (), (), ()> {
  /// Runs `on_change` whenever a signal read during `compute` changes. Runs
  /// `compute` once immediately and returns its value.
  #[expect(clippy::new_ret_no_self)]
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new<R>(compute: impl Fn() -> R + 'static, on_change: impl Fn(R) + 'static) -> R
  where
    R: 'static,
  {
    UpdaterEffect::new_stateful(move |_| (compute(), ()), move |r, _| on_change(r))
  }

  /// Like [`UpdaterEffect::new`], but threads state `T` out of `compute` into each
  /// `on_change` and back.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new_stateful<T, R>(
    compute: impl Fn(Option<T>) -> (R, T) + 'static,
    on_change: impl Fn(R, T) -> T + 'static,
  ) -> R
  where
    T: Any + 'static,
    R: 'static,
  {
    Runtime::assert_ui_thread();
    let id = Id::next();
    let effect = Rc::new(UpdaterEffect {
      id,
      compute,
      on_change,
      value: RefCell::new(None),
      observers: RefCell::new(FxHashSet::default()),
      _phantom: PhantomData,
    });

    id.set_scope();
    let effect_dyn: Rc<dyn EffectTrait> = effect.clone();
    RUNTIME.with(|runtime| runtime.register_effect(&effect_dyn));

    run_initial_updater_effect(effect)
  }
}

impl Effect<(), fn(Option<()>) -> ()> {
  /// Runs `f` without subscribing the current effect to any signal it reads.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    Runtime::assert_ui_thread();
    let prev_effect = RUNTIME.with(|runtime| runtime.current_effect.borrow_mut().take());
    let result = f();

    RUNTIME.with(|runtime| {
      *runtime.current_effect.borrow_mut() = prev_effect;
    });

    result
  }

  /// Runs `f`, deferring dependent effects until it returns so a batch of signal
  /// writes triggers each effect at most once.
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
}

fn run_initial_effect(effect: Rc<dyn EffectTrait>) {
  Runtime::assert_ui_thread();
  let effect_id = effect.id();

  RUNTIME.with(|runtime| {
    *runtime.current_effect.borrow_mut() = Some(effect.clone());

    let effect_scope = Scope(effect_id, PhantomData);
    effect_scope.enter(|| {
      effect.run();
    });

    *runtime.current_effect.borrow_mut() = None;
  });
}

pub(crate) fn run_effect(effect: Rc<dyn EffectTrait>) {
  Runtime::assert_ui_thread();
  let effect_id = effect.id();
  effect_id.dispose_children();

  observer_clean_up(&effect);

  RUNTIME.with(|runtime| {
    *runtime.current_effect.borrow_mut() = Some(effect.clone());

    let effect_scope = Scope(effect_id, PhantomData);
    effect_scope.enter(move || {
      effect.run();
    });

    *runtime.current_effect.borrow_mut() = None;
  });
}

fn run_initial_updater_effect<T, I, C, U>(effect: Rc<UpdaterEffect<T, I, C, U>>) -> I
where
  T: 'static,
  I: 'static,
  C: Fn(Option<T>) -> (I, T) + 'static,
  U: Fn(I, T) -> T + 'static,
{
  Runtime::assert_ui_thread();
  let effect_id = effect.id();

  RUNTIME.with(|runtime| {
    *runtime.current_effect.borrow_mut() = Some(effect.clone());

    let effect_scope = Scope(effect_id, PhantomData);
    let (result, new_value) = effect_scope.enter(|| (effect.compute)(None));

    *effect.value.borrow_mut() = Some(new_value);

    *runtime.current_effect.borrow_mut() = None;

    result
  })
}

/// Drops `effect` from the subscriber set of every signal it was observing.
pub(crate) fn observer_clean_up(effect: &Rc<dyn EffectTrait>) {
  let effect_id = effect.id();
  let observers = effect.clear_observers();

  RUNTIME.with(|runtime| {
    let entities = runtime.entities.borrow();

    for observer in observers {
      if let Some(signal) = entities
        .get(&observer)
        .and_then(|entity| entity.signal.as_ref())
      {
        signal.subscribers.remove(effect_id);
      }
      else if let Some(signal) = SYNC_RUNTIME.get_signal(&observer) {
        signal.subscribers.lock().remove(&effect_id);
      }
    }
  });
}

impl<T, F> EffectTrait for Effect<T, F>
where
  T: 'static,
  F: Fn(Option<T>) -> T + 'static,
{
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn run(&self) -> bool {
    let curr_value = self.value.borrow_mut().take();
    let new_value = (self.f)(curr_value);

    *self.value.borrow_mut() = Some(new_value);

    true
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

impl<T, I, C, U> EffectTrait for UpdaterEffect<T, I, C, U>
where
  T: 'static,
  I: 'static,
  C: Fn(Option<T>) -> (I, T) + 'static,
  U: Fn(I, T) -> T + 'static,
{
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn run(&self) -> bool {
    let curr_value = self.value.borrow_mut().take();
    let (i, t) = (self.compute)(curr_value);
    let new_value = (self.on_change)(i, t);

    *self.value.borrow_mut() = Some(new_value);

    true
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

/// Re-runs `on_change` whenever a signal read during the last [`SignalTracker::track`]
/// call changes, until the tracker is dropped.
pub struct SignalTracker {
  id: Id,
  on_change: Rc<dyn Fn()>,
}

impl Drop for SignalTracker {
  fn drop(&mut self) {
    self.id.dispose();
  }
}

impl SignalTracker {
  /// Creates a [`SignalTracker`] that subscribes to any changes in signals used within `on_change`.
  pub fn new(on_change: impl Fn() + 'static) -> Self {
    let id = Id::next();

    SignalTracker {
      id,
      on_change: Rc::new(on_change),
    }
  }

  /// Runs `f` and returns its value, subscribing to every signal it reads so a
  /// later change re-runs `on_change`. Replaces any previous subscription.
  pub fn track<T: 'static>(&self, f: impl FnOnce() -> T) -> T {
    Runtime::assert_ui_thread();
    self.id.dispose();

    let prev_effect = RUNTIME.with(|runtime| runtime.current_effect.borrow_mut().take());

    let tracking_effect: Rc<dyn EffectTrait> = Rc::new(TrackingEffect {
      id: self.id,
      observers: RefCell::new(FxHashSet::default()),
      on_change: self.on_change.clone(),
    });

    RUNTIME.with(|runtime| {
      runtime.register_effect(&tracking_effect);
      *runtime.current_effect.borrow_mut() = Some(tracking_effect.clone());
    });

    let effect_scope = Scope(self.id, PhantomData);
    let result = effect_scope.enter(|| {
      effect_scope.track();
      f()
    });

    RUNTIME.with(|runtime| {
      *runtime.current_effect.borrow_mut() = prev_effect;
    });

    result
  }
}

struct TrackingEffect {
  id: Id,
  observers: RefCell<FxHashSet<Id>>,
  on_change: Rc<dyn Fn()>,
}

impl EffectTrait for TrackingEffect {
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn run(&self) -> bool {
    (self.on_change)();
    true
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
