use std::{
  any::Any,
  cell::{Cell, RefCell},
  marker::PhantomData,
  rc::Rc,
};

use crate::{
  computation::{self, Computation},
  graph::flags::{PAUSED, RENDER},
  id::Id,
  runtime::{RUNTIME, Runtime, with_graph},
};

/// A side effect that re-runs whenever a signal it read changes.
///
/// The type is the constructors' namespace; an effect is addressed through the
/// [`EffectHandle`] they return and lives until its [`Owner`](crate::Owner) is disposed.
pub struct Effect<T, F>
where
  T: 'static,
  F: Fn(Option<T>) -> T,
{
  ty: PhantomData<(T, F)>,
}

/// Addresses one effect: a `Copy` id that disposes, pauses or resumes it.
///
/// Dropping a handle does nothing; the effect belongs to its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectHandle {
  id: Id,
}

impl EffectHandle {
  /// The effect's id.
  #[inline(always)]
  pub fn id(&self) -> Id {
    self.id
  }

  /// Disposes the effect: it never runs again and its children are disposed.
  pub fn dispose(&self) {
    self.id.dispose();
  }

  /// Stops the effect from running; invalidations still accumulate.
  pub fn pause(&self) {
    with_graph(|graph| graph.update_flags(self.id, PAUSED, 0));
  }

  /// Lets the effect run again, immediately if a dependency changed while it was
  /// paused.
  pub fn resume(&self) {
    let queued = RUNTIME.with(|runtime| {
      let mut graph = runtime.graph();

      graph.update_flags(self.id, 0, PAUSED);

      graph.enqueue(self.id)
    });

    if queued {
      RUNTIME.with(Runtime::request_flush);
    }
  }
}

/// The body behind [`Effect::new`] and [`Effect::render`].
struct EffectBody<T, F> {
  f: F,
  value: RefCell<Option<T>>,
}

impl<T, F> Computation for EffectBody<T, F>
where
  T: 'static,
  F: Fn(Option<T>) -> T + 'static,
{
  fn run(&self) -> bool {
    let previous = self.value.borrow_mut().take();
    let next = (self.f)(previous);

    *self.value.borrow_mut() = Some(next);

    false
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl<T, F> Effect<T, F>
where
  T: Any + 'static,
  F: Fn(Option<T>) -> T + 'static,
{
  /// Runs `f` immediately and re-runs it whenever a signal it read changes. `f`
  /// receives its previous return value, or `None` on the first run. The effect lives
  /// until the current [`Owner`](crate::Owner) is disposed.
  #[expect(clippy::new_ret_no_self)]
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(f: F) -> EffectHandle {
    Self::create(f, 0)
  }

  /// Like [`new`](Self::new), but a render effect: in a flush, every render effect
  /// runs before any ordinary effect, ordered by owner depth then creation.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn render(f: F) -> EffectHandle {
    Self::create(f, RENDER)
  }

  fn create(f: F, flags: u8) -> EffectHandle {
    Runtime::assert_ui_thread();

    let body: Rc<dyn Computation> = Rc::new(EffectBody {
      f,
      value: RefCell::new(None),
    });
    let id = computation::create(body, flags);

    computation::run(id);

    EffectHandle { id }
  }
}

/// The body behind [`Effect::watch`]: recompute, then hand the value on.
struct WatchBody<C, U> {
  compute: C,
  on_change: U,
  first: Cell<bool>,
}

impl<R, C, U> Computation for WatchBody<C, U>
where
  R: 'static,
  C: Fn() -> R + 'static,
  U: Fn(R) + 'static,
{
  fn run(&self) -> bool {
    let value = (self.compute)();

    if self.first.replace(false) {
      return false;
    }

    (self.on_change)(value);

    false
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl Effect<(), fn(Option<()>) -> ()> {
  /// Runs `compute` immediately; whenever a signal it read changes, recomputes and
  /// passes the fresh value to `on_change`, which does not run for the first compute.
  ///
  /// The fine-grained binder form: only `compute` is tracked, so `on_change` may read
  /// anything without subscribing.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn watch<R>(
    compute: impl Fn() -> R + 'static,
    on_change: impl Fn(R) + 'static,
  ) -> EffectHandle
  where
    R: 'static,
  {
    Runtime::assert_ui_thread();

    let body: Rc<dyn Computation> = Rc::new(WatchBody {
      compute,
      on_change,
      first: Cell::new(true),
    });
    let id = computation::create(body, 0);

    computation::run(id);

    EffectHandle { id }
  }
}

/// Runs `f` without subscribing the current effect to any signal it reads.
#[cfg_attr(debug_assertions, track_caller)]
pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
  Runtime::assert_ui_thread();
  Runtime::with_tracking(None, f)
}

/// Runs `f`, deferring dependent effects until it returns so a batch of signal writes
/// triggers each effect at most once.
#[cfg_attr(debug_assertions, track_caller)]
pub fn batch<T>(f: impl FnOnce() -> T) -> T {
  let _guard = Runtime::enter_batch();

  f()
}

/// The body behind [`Reaction`]: fires the callback, subscribing to nothing.
struct ReactionBody {
  on_invalidate: Rc<dyn Fn()>,
}

impl Computation for ReactionBody {
  fn run(&self) -> bool {
    (self.on_invalidate)();

    false
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

/// Re-runs a callback whenever a signal read during the last [`track`](Self::track)
/// call changes, until the reaction is dropped.
#[must_use]
pub struct Reaction {
  id: Cell<Option<Id>>,
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
      id: Cell::new(None),
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

    let body: Rc<dyn Computation> = Rc::new(ReactionBody {
      on_invalidate: self.on_invalidate.clone(),
    });
    let id = computation::create_detached(body, 0);

    self.id.set(Some(id));

    computation::track(id, f)
  }
}
