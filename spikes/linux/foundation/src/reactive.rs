//! Fine-grained reactivity: a [`Signal`] holds a value, an [`Effect`] reruns whenever a signal
//! it read changes, a [`Memo`] is a value derived from signals, and an [`Owner`] scopes what is
//! created under it, so that disposing the owner disposes it all.
//!
//! The runtime is per thread. Effects a write makes stale run once the write returns, or at the
//! end of the outermost [`batch`]. Before an effect reruns, everything its previous run created
//! (effects, signals, owners, cleanups) is disposed.

use std::{
  any::Any,
  cell::{Cell, RefCell},
  collections::VecDeque,
  fmt,
  marker::PhantomData,
  rc::Rc,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Id {
  index: u32,
  generation: u32,
}

/// Slots reused under a new generation once freed.
struct Arena<T> {
  slots: Vec<(u32, Option<T>)>,
  free: Vec<u32>,
}

impl<T> Arena<T> {
  const fn new() -> Self {
    Self {
      slots: Vec::new(),
      free: Vec::new(),
    }
  }

  fn insert(&mut self, value: T) -> Id {
    if let Some(index) = self.free.pop() {
      let slot = &mut self.slots[index as usize];

      slot.0 += 1;
      slot.1 = Some(value);

      return Id {
        index,
        generation: slot.0,
      };
    }

    self.slots.push((0, Some(value)));

    Id {
      index: (self.slots.len() - 1) as u32,
      generation: 0,
    }
  }

  fn get(&self, id: Id) -> Option<&T> {
    match self.slots.get(id.index as usize) {
      Some((generation, Some(value))) if *generation == id.generation => Some(value),
      _ => None,
    }
  }

  fn get_mut(&mut self, id: Id) -> Option<&mut T> {
    match self.slots.get_mut(id.index as usize) {
      Some((generation, Some(value))) if *generation == id.generation => Some(value),
      _ => None,
    }
  }

  fn remove(&mut self, id: Id) -> Option<T> {
    let slot = self.slots.get_mut(id.index as usize)?;

    if slot.0 != id.generation {
      return None;
    }

    let value = slot.1.take()?;

    self.free.push(id.index);

    Some(value)
  }
}

struct SignalSlot {
  value: Rc<RefCell<Box<dyn Any>>>,
  subscribers: Vec<Id>,
}

struct EffectSlot {
  run: Rc<RefCell<dyn FnMut()>>,
  sources: Vec<Id>,
  owner: Id,
  queued: bool,
}

#[derive(Default)]
struct OwnerSlot {
  parent: Option<Id>,
  children: Vec<Id>,
  effects: Vec<Id>,
  signals: Vec<Id>,
  cleanups: Vec<Box<dyn FnOnce()>>,
}

struct Runtime {
  signals: RefCell<Arena<SignalSlot>>,
  effects: RefCell<Arena<EffectSlot>>,
  owners: RefCell<Arena<OwnerSlot>>,
  observer: Cell<Option<Id>>,
  owner: Cell<Option<Id>>,
  pending: RefCell<VecDeque<Id>>,
  batching: Cell<u32>,
  flushing: Cell<bool>,
}

thread_local! {
  static RUNTIME: Runtime = const {
    Runtime {
      signals: RefCell::new(Arena::new()),
      effects: RefCell::new(Arena::new()),
      owners: RefCell::new(Arena::new()),
      observer: Cell::new(None),
      owner: Cell::new(None),
      pending: RefCell::new(VecDeque::new()),
      batching: Cell::new(0),
      flushing: Cell::new(false),
    }
  };
}

impl Runtime {
  #[inline(always)]
  fn with<R>(body: impl FnOnce(&Self) -> R) -> R {
    RUNTIME.with(body)
  }

  /// Creates an owner under `parent`.
  fn new_owner(&self, parent: Option<Id>) -> Id {
    let id = self.owners.borrow_mut().insert(OwnerSlot {
      parent,
      ..OwnerSlot::default()
    });

    if let Some(parent) = parent
      && let Some(slot) = self.owners.borrow_mut().get_mut(parent)
    {
      slot.children.push(id);
    }

    id
  }

  /// Queues every effect that read `signal`.
  fn queue_subscribers(&self, signal: Id) {
    let subscribers = self
      .signals
      .borrow()
      .get(signal)
      .map(|slot| slot.subscribers.clone())
      .unwrap_or_default();
    let mut effects = self.effects.borrow_mut();
    let mut pending = self.pending.borrow_mut();

    for id in subscribers {
      if let Some(effect) = effects.get_mut(id)
        && !effect.queued
      {
        effect.queued = true;
        pending.push_back(id);
      }
    }
  }

  /// Runs queued effects until none is left, unless a flush or a batch is in progress.
  fn flush(&self) {
    if self.batching.get() > 0 || self.flushing.replace(true) {
      return;
    }

    loop {
      let next = self.pending.borrow_mut().pop_front();

      match next {
        Some(id) => self.run_effect(id),
        None => break,
      }
    }

    self.flushing.set(false);
  }

  /// Runs an effect once: forgets what it read, disposes what its last run created, and runs
  /// it under its own owner, tracking what it reads.
  fn run_effect(&self, id: Id) {
    let Some((run, owner, sources)) = self.effects.borrow_mut().get_mut(id).map(|effect| {
      effect.queued = false;

      (
        Rc::clone(&effect.run),
        effect.owner,
        std::mem::take(&mut effect.sources),
      )
    }) else {
      return;
    };

    {
      let mut signals = self.signals.borrow_mut();

      for source in sources {
        if let Some(slot) = signals.get_mut(source) {
          slot.subscribers.retain(|subscriber| *subscriber != id);
        }
      }
    }

    self.dispose_contents(owner);

    let observer = self.observer.replace(Some(id));
    let current = self.owner.replace(Some(owner));

    (run.borrow_mut())();
    self.observer.set(observer);
    self.owner.set(current);
  }

  /// Disposes everything created under `owner`, keeping the owner itself.
  fn dispose_contents(&self, owner: Id) {
    let Some((children, effects, signals, cleanups)) =
      self.owners.borrow_mut().get_mut(owner).map(|slot| {
        (
          std::mem::take(&mut slot.children),
          std::mem::take(&mut slot.effects),
          std::mem::take(&mut slot.signals),
          std::mem::take(&mut slot.cleanups),
        )
      })
    else {
      return;
    };

    for cleanup in cleanups.into_iter().rev() {
      cleanup();
    }

    for child in children.into_iter().rev() {
      self.dispose_owner(child, false);
    }

    for effect in effects {
      self.remove_effect(effect);
    }

    let mut slots = self.signals.borrow_mut();

    for signal in signals {
      slots.remove(signal);
    }
  }

  /// Removes an effect and its subscriptions, returning the owner its runs create under.
  fn remove_effect(&self, effect: Id) -> Option<Id> {
    let slot = self.effects.borrow_mut().remove(effect)?;
    let mut signals = self.signals.borrow_mut();

    for source in slot.sources {
      if let Some(signal) = signals.get_mut(source) {
        signal
          .subscribers
          .retain(|subscriber| *subscriber != effect);
      }
    }

    Some(slot.owner)
  }

  /// Disposes `owner` and everything created under it.
  fn dispose_owner(&self, owner: Id, detach: bool) {
    self.dispose_contents(owner);

    let parent = self
      .owners
      .borrow_mut()
      .remove(owner)
      .and_then(|slot| slot.parent);

    if detach
      && let Some(parent) = parent
      && let Some(slot) = self.owners.borrow_mut().get_mut(parent)
    {
      slot.children.retain(|child| *child != owner);
    }
  }
}

/// A value that notifies whoever read it when it changes; a copyable handle, so it moves
/// freely into closures.
///
/// A signal lives as long as the [`Owner`] it was created under; one created under none lives
/// as long as the thread.
pub struct Signal<T> {
  id: Id,
  ty: PhantomData<fn() -> T>,
}

impl<T> Clone for Signal<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for Signal<T> {}

impl<T> PartialEq for Signal<T> {
  #[inline(always)]
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T> Eq for Signal<T> {}

impl<T> fmt::Debug for Signal<T> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "Signal({}v{})",
      self.id.index, self.id.generation
    )
  }
}

impl<T: 'static> Signal<T> {
  /// A signal holding `value`, owned by the current owner.
  pub fn new(value: T) -> Self {
    Runtime::with(|runtime| {
      let id = runtime.signals.borrow_mut().insert(SignalSlot {
        value: Rc::new(RefCell::new(Box::new(value))),
        subscribers: Vec::new(),
      });

      if let Some(owner) = runtime.owner.get()
        && let Some(slot) = runtime.owners.borrow_mut().get_mut(owner)
      {
        slot.signals.push(id);
      }

      Self {
        id,
        ty: PhantomData,
      }
    })
  }

  fn cell(&self, track: bool) -> Rc<RefCell<Box<dyn Any>>> {
    Runtime::with(|runtime| {
      let mut signals = runtime.signals.borrow_mut();
      let slot = signals
        .get_mut(self.id)
        .expect("a signal is used only while its owner lives");

      if track
        && let Some(observer) = runtime.observer.get()
        && !slot.subscribers.contains(&observer)
      {
        slot.subscribers.push(observer);

        if let Some(effect) = runtime.effects.borrow_mut().get_mut(observer) {
          effect.sources.push(self.id);
        }
      }

      Rc::clone(&slot.value)
    })
  }

  /// Reads the value through `read`, subscribing the running effect.
  pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
    let cell = self.cell(true);
    let value = cell.borrow();

    read(value.downcast_ref::<T>().expect("a signal holds its type"))
  }

  /// Reads the value through `read` without subscribing anything.
  pub fn with_untracked<R>(&self, read: impl FnOnce(&T) -> R) -> R {
    let cell = self.cell(false);
    let value = cell.borrow();

    read(value.downcast_ref::<T>().expect("a signal holds its type"))
  }

  /// The value now, subscribing the running effect.
  #[must_use]
  pub fn get(&self) -> T
  where
    T: Clone,
  {
    self.with(T::clone)
  }

  /// The value now, subscribing nothing.
  #[must_use]
  pub fn get_untracked(&self) -> T
  where
    T: Clone,
  {
    self.with_untracked(T::clone)
  }

  /// Replaces the value and reruns whatever read it.
  pub fn set(&self, value: T) {
    self.update(|slot| *slot = value);
  }

  /// Changes the value in place and reruns whatever read it.
  pub fn update(&self, change: impl FnOnce(&mut T)) {
    let cell = self.cell(false);

    change(
      cell
        .borrow_mut()
        .downcast_mut::<T>()
        .expect("a signal holds its type"),
    );

    Runtime::with(|runtime| {
      runtime.queue_subscribers(self.id);
      runtime.flush();
    });
  }
}

/// A computation that reruns whenever a signal it read changes.
///
/// It runs once when created, under an owner of its own: what a run creates is disposed before
/// the next one, and everything is disposed with the owner the effect was created under.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Effect(Id);

impl fmt::Debug for Effect {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "Effect({}v{})", self.0.index, self.0.generation)
  }
}

impl Effect {
  /// Runs `run` now and whenever a signal it read changes.
  pub fn new(run: impl FnMut() + 'static) -> Self {
    Runtime::with(|runtime| {
      let parent = runtime.owner.get();
      let owner = runtime.new_owner(parent);
      let id = runtime.effects.borrow_mut().insert(EffectSlot {
        run: Rc::new(RefCell::new(run)),
        sources: Vec::new(),
        owner,
        queued: false,
      });

      if let Some(parent) = parent
        && let Some(slot) = runtime.owners.borrow_mut().get_mut(parent)
      {
        slot.effects.push(id);
      }

      let flushing = runtime.flushing.replace(true);

      runtime.run_effect(id);
      runtime.flushing.set(flushing);
      runtime.flush();

      Self(id)
    })
  }

  /// Stops the effect and disposes what it created.
  pub fn dispose(self) {
    Runtime::with(|runtime| {
      if let Some(owner) = runtime.remove_effect(self.0) {
        runtime.dispose_owner(owner, true);
      }
    });
  }
}

/// A value derived from signals, recomputed when one of them changes and notifying its readers
/// only when the result differs.
pub struct Memo<T> {
  value: Signal<Option<T>>,
}

impl<T> Clone for Memo<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for Memo<T> {}

impl<T> fmt::Debug for Memo<T> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "Memo({:?})", self.value)
  }
}

impl<T: Clone + PartialEq + 'static> Memo<T> {
  /// The value `compute` returns, kept current.
  pub fn new(compute: impl Fn() -> T + 'static) -> Self {
    let value = Signal::new(None);

    Effect::new(move || {
      let next = compute();

      if value.with_untracked(|now| now.as_ref() != Some(&next)) {
        value.set(Some(next));
      }
    });

    Self { value }
  }

  /// The value now, subscribing the running effect.
  #[must_use]
  pub fn get(&self) -> T {
    self
      .value
      .with(|value| value.clone().expect("a memo computes on creation"))
  }

  /// Reads the value through `read`, subscribing the running effect.
  pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
    self
      .value
      .with(|value| read(value.as_ref().expect("a memo computes on creation")))
  }
}

/// A scope that owns the signals, effects and owners created while it runs code, and disposes
/// them all when it is disposed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Owner(Id);

impl fmt::Debug for Owner {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "Owner({}v{})", self.0.index, self.0.generation)
  }
}

impl Owner {
  /// A new owner under the current one, or a root where there is none.
  #[must_use]
  pub fn new() -> Self {
    Runtime::with(|runtime| Self(runtime.new_owner(runtime.owner.get())))
  }

  /// A new owner under none, disposed only when disposed itself.
  #[must_use]
  pub fn root() -> Self {
    Runtime::with(|runtime| Self(runtime.new_owner(None)))
  }

  /// The owner code runs under now.
  #[must_use]
  pub fn current() -> Option<Self> {
    Runtime::with(|runtime| runtime.owner.get().map(Self))
  }

  /// Runs `body` under this owner, untracked: what it creates belongs to this owner, and what
  /// it reads subscribes no enclosing effect.
  pub fn run<R>(&self, body: impl FnOnce() -> R) -> R {
    let (owner, observer) =
      Runtime::with(|runtime| (runtime.owner.replace(Some(self.0)), runtime.observer.take()));
    let result = body();

    Runtime::with(|runtime| {
      runtime.owner.set(owner);
      runtime.observer.set(observer);
    });

    result
  }

  /// Runs `cleanup` when the current owner is disposed, or before its effect reruns; with no
  /// current owner it never runs.
  pub fn on_cleanup(cleanup: impl FnOnce() + 'static) {
    Runtime::with(|runtime| {
      if let Some(owner) = runtime.owner.get()
        && let Some(slot) = runtime.owners.borrow_mut().get_mut(owner)
      {
        slot.cleanups.push(Box::new(cleanup));
      }
    });
  }

  /// Disposes everything created under this owner, and the owner itself.
  pub fn dispose(self) {
    Runtime::with(|runtime| runtime.dispose_owner(self.0, true));
  }
}

impl Default for Owner {
  fn default() -> Self {
    Self::new()
  }
}

/// Runs `body` with the effects its writes make stale deferred to its end.
pub fn batch<R>(body: impl FnOnce() -> R) -> R {
  Runtime::with(|runtime| runtime.batching.set(runtime.batching.get() + 1));

  let result = body();

  Runtime::with(|runtime| {
    runtime.batching.set(runtime.batching.get() - 1);
    runtime.flush();
  });

  result
}

/// Runs `body` without subscribing the running effect to what it reads.
pub fn untrack<R>(body: impl FnOnce() -> R) -> R {
  let observer = Runtime::with(|runtime| runtime.observer.take());
  let result = body();

  Runtime::with(|runtime| runtime.observer.set(observer));

  result
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::*;

  #[test]
  fn an_effect_reruns_when_what_it_read_changes() {
    let count = Signal::new(1);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&seen);

    Effect::new(move || log.borrow_mut().push(count.get()));
    count.set(2);

    batch(|| {
      count.set(3);
      count.set(4);
    });

    assert_eq!(*seen.borrow(), [1, 2, 4]);
  }

  #[test]
  fn a_memo_notifies_only_on_a_different_value() {
    let count = Signal::new(1);
    let parity = Memo::new(move || count.get() % 2);
    let runs = Rc::new(RefCell::new(0));
    let counter = Rc::clone(&runs);

    Effect::new(move || {
      let _ = parity.get();
      *counter.borrow_mut() += 1;
    });

    count.set(3);
    count.set(4);

    assert_eq!(*runs.borrow(), 2);
  }

  #[test]
  fn a_rerun_disposes_what_the_last_run_created() {
    let switch = Signal::new(0);
    let inner_runs = Rc::new(RefCell::new(0));
    let runs = Rc::clone(&inner_runs);
    let tick = Signal::new(0);

    Effect::new(move || {
      let _ = switch.get();
      let runs = Rc::clone(&runs);

      Effect::new(move || {
        let _ = tick.get();
        *runs.borrow_mut() += 1;
      });
    });
    switch.set(1);
    tick.set(1);

    assert_eq!(*inner_runs.borrow(), 3);
  }

  #[test]
  fn disposing_an_owner_leaves_no_subscription_behind() {
    let long_lived = Signal::new(0);

    for _ in 0..100 {
      let owner = Owner::root();

      owner.run(|| {
        Effect::new(move || {
          let _ = long_lived.get();
        })
      });
      owner.dispose();
    }

    let subscribers = Runtime::with(|runtime| {
      runtime
        .signals
        .borrow()
        .get(long_lived.id)
        .map(|slot| slot.subscribers.len())
    });

    assert_eq!(subscribers, Some(0));
  }

  #[test]
  fn disposing_an_owner_stops_its_effects() {
    let count = Signal::new(0);
    let runs = Rc::new(RefCell::new(0));
    let counter = Rc::clone(&runs);
    let owner = Owner::new();

    owner.run(|| {
      Effect::new(move || {
        let _ = count.get();
        *counter.borrow_mut() += 1;
      })
    });
    owner.dispose();
    count.set(1);

    assert_eq!(*runs.borrow(), 1);
  }
}
