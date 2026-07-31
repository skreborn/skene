use std::{
  any::Any,
  cell::{Cell, RefCell},
  marker::PhantomData,
  rc::Rc,
};

use rustc_hash::FxHashSet;

use crate::{
  SignalGet,
  SignalWith,
  effect::{EffectPriority, EffectTrait, observer_clean_up},
  id::Id,
  read::SignalTrack,
  runtime::{RUNTIME, Runtime},
  scope::Scope,
  signal::{ReadSignal, RwSignal, WriteSignal},
  write::SignalUpdate,
};

/// A memoized derived value.
///
/// Recomputes only when one of its tracked dependencies changes, and notifies its
/// own dependents only when the recomputed value differs.
#[must_use]
pub struct Memo<T: PartialEq + 'static> {
  getter: ReadSignal<T>,
  memo_id: Id,
}

impl<T: PartialEq + 'static> Copy for Memo<T> {}

impl<T: PartialEq + 'static> Clone for Memo<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: Clone + PartialEq + 'static> SignalGet<T> for Memo<T>
where
  ReadSignal<T>: SignalGet<T>,
{
  fn id(&self) -> Id {
    self.getter.id
  }

  fn get_untracked(&self) -> T
  where
    T: 'static,
  {
    self.ensure_fresh();
    self.getter.get_untracked()
  }

  fn get(&self) -> T
  where
    T: 'static,
  {
    self.ensure_fresh();
    self.getter.get()
  }

  fn try_get(&self) -> Option<T>
  where
    T: 'static,
  {
    self.ensure_fresh();
    self.getter.try_get()
  }

  fn try_get_untracked(&self) -> Option<T>
  where
    T: 'static,
  {
    self.ensure_fresh();
    self.getter.try_get_untracked()
  }
}

impl<T: PartialEq + 'static> SignalTrack<T> for Memo<T> {
  fn id(&self) -> Id {
    self.getter.id
  }
}

impl<T: PartialEq + 'static> SignalWith<T> for Memo<T>
where
  ReadSignal<T>: SignalWith<T>,
{
  fn id(&self) -> Id {
    self.getter.id
  }

  fn with<O>(&self, f: impl FnOnce(&T) -> O) -> O
  where
    T: 'static,
  {
    self.ensure_fresh();
    self.getter.with(f)
  }

  fn with_untracked<O>(&self, f: impl FnOnce(&T) -> O) -> O
  where
    T: 'static,
  {
    self.ensure_fresh();
    self.getter.with_untracked(f)
  }

  fn try_with<O>(&self, f: impl FnOnce(Option<&T>) -> O) -> O
  where
    T: 'static,
  {
    self.ensure_fresh();
    self.getter.try_with(f)
  }

  fn try_with_untracked<O>(&self, f: impl FnOnce(Option<&T>) -> O) -> O
  where
    T: 'static,
  {
    self.ensure_fresh();
    self.getter.try_with_untracked(f)
  }
}

impl<T: PartialEq + 'static> Memo<T> {
  /// Creates a memo computed by `f`, which receives the previous value or `None`
  /// on its first run.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(f: impl Fn(Option<&T>) -> T + 'static) -> Self {
    Runtime::assert_ui_thread();

    let memo_id = Id::next();
    let state = Rc::new(MemoState::new(memo_id, f));

    memo_id.set_scope();
    let effect: Rc<dyn EffectTrait> = state.clone();
    RUNTIME.with(|runtime| runtime.register_effect(&effect));

    let initial = state.compute_initial();
    let (getter, setter) = RwSignal::new_split(initial);
    state.set_signal(getter, setter);
    state.mark_clean();

    Memo { getter, memo_id }
  }

  fn ensure_fresh(&self) {
    self.with_state(|state| state.ensure_fresh());
  }

  fn with_state<O>(&self, f: impl FnOnce(&MemoState<T>) -> O) -> Option<O> {
    RUNTIME.with(|runtime| {
      runtime
        .get_effect(self.memo_id)
        .and_then(|effect| effect.as_any().downcast_ref::<MemoState<T>>().map(f))
    })
  }
}

type ComputeFn<T> = Box<dyn Fn(Option<&T>) -> T>;

struct MemoState<T: PartialEq + 'static> {
  id: Id,
  compute: ComputeFn<T>,
  getter: RefCell<Option<ReadSignal<T>>>,
  setter: RefCell<Option<WriteSignal<T>>>,
  dirty: Cell<bool>,
  observers: RefCell<FxHashSet<Id>>,
  _phantom: PhantomData<T>,
}

impl<T: PartialEq + 'static> MemoState<T> {
  fn new(id: Id, compute: impl Fn(Option<&T>) -> T + 'static) -> Self {
    Self {
      id,
      compute: Box::new(compute),
      getter: RefCell::new(None),
      setter: RefCell::new(None),
      dirty: Cell::new(true),
      observers: RefCell::new(FxHashSet::default()),
      _phantom: PhantomData,
    }
  }

  fn compute_initial(&self) -> T {
    let effect = RUNTIME
      .with(|runtime| runtime.get_effect(self.id))
      .expect("memo registered before initial compute");

    let prev_effect = RUNTIME.with(|runtime| runtime.current_effect.borrow_mut().replace(effect));
    let scope = Scope(self.id, PhantomData);
    let value = scope.enter(|| (self.compute)(None));

    RUNTIME.with(|runtime| *runtime.current_effect.borrow_mut() = prev_effect);
    value
  }

  fn set_signal(&self, getter: ReadSignal<T>, setter: WriteSignal<T>) {
    self.getter.replace(Some(getter));
    self.setter.replace(Some(setter));
  }

  fn mark_clean(&self) {
    self.dirty.set(false);
  }

  fn ensure_fresh(&self) {
    if !self.dirty.get() {
      return;
    }

    self.recompute();
  }

  fn recompute(&self) {
    let getter = self
      .getter
      .borrow()
      .as_ref()
      .copied()
      .expect("memo getter set");

    Runtime::assert_ui_thread();

    let effect = RUNTIME
      .with(|runtime| runtime.get_effect(self.id))
      .expect("memo registered");

    observer_clean_up(&effect);

    let prev_effect = RUNTIME.with(|runtime| runtime.current_effect.borrow_mut().replace(effect));
    let scope = Scope(self.id, PhantomData);
    let (changed, new_value) = scope.enter(|| {
      getter.try_with_untracked(|prev| {
        let new_value = (self.compute)(prev);
        let changed = match prev {
          Some(previous) => new_value != *previous,
          None => true,
        };

        (changed, new_value)
      })
    });

    RUNTIME.with(|runtime| *runtime.current_effect.borrow_mut() = prev_effect);

    if changed && let Some(setter) = self.setter.borrow().as_ref() {
      setter.set(new_value);
    }

    self.dirty.set(false);
  }
}

impl<T: PartialEq + 'static> Drop for MemoState<T> {
  fn drop(&mut self) {
    if RUNTIME
      .try_with(|runtime| runtime.remove_effect(self.id))
      .is_ok()
    {
      self.id.dispose();
    }
  }
}

impl<T> EffectTrait for MemoState<T>
where
  T: PartialEq + 'static,
{
  #[inline(always)]
  fn id(&self) -> Id {
    self.id
  }

  fn run(&self) -> bool {
    self.dirty.set(true);
    self.ensure_fresh();
    true
  }

  fn add_observer(&self, id: Id) {
    self.observers.borrow_mut().insert(id);
  }

  fn clear_observers(&self) -> Vec<Id> {
    self.observers.borrow_mut().drain().collect()
  }

  fn mark_stale(&self) -> Option<Id> {
    if self.dirty.get() {
      return None;
    }

    self.dirty.set(true);

    self.getter.borrow().as_ref().map(|getter| getter.id)
  }

  fn priority(&self) -> EffectPriority {
    EffectPriority::High
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
