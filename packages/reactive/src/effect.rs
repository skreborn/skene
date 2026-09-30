use std::{any::Any, cell::RefCell, marker::PhantomData, rc::Rc};

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

impl<T, F> std::fmt::Debug for Effect<T, F>
where
  T: 'static,
  F: Fn(Option<T>) -> T,
{
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Effect").finish_non_exhaustive()
  }
}

/// Addresses one effect: a `Copy` id that disposes, pauses or resumes it.
///
/// Dropping a handle does nothing; the effect belongs to its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectHandle {
  id: Id,
  local: PhantomData<*const ()>,
}

impl EffectHandle {
  /// The effect's id.
  #[inline(always)]
  pub const fn id(&self) -> Id {
    self.id
  }

  /// Disposes the effect: it never runs again and its children are disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn dispose(&self) {
    #[cfg(debug_assertions)]
    let _disposing = crate::obituary::Disposing::enter(std::panic::Location::caller());

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
  body: F,
  value: RefCell<Option<T>>,
}

impl<T, F> Computation for EffectBody<T, F>
where
  T: 'static,
  F: Fn(Option<T>) -> T + 'static,
{
  fn run(&self) -> bool {
    let previous = self.value.borrow_mut().take();
    let next = (self.body)(previous);

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
  /// Runs `body` immediately and re-runs it whenever a signal it read changes. `body`
  /// receives its previous return value, or `None` on the first run. The effect lives
  /// until the current [`Owner`](crate::Owner) is disposed.
  #[expect(
    clippy::new_ret_no_self,
    reason = "the effect lives in the runtime, and `new` hands back the handle controlling it"
  )]
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(body: F) -> EffectHandle {
    Self::create(body, 0)
  }

  /// Like [`new`](Self::new), but a render effect: in a flush, every render effect
  /// runs before any ordinary effect, ordered by owner depth then creation.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn render(body: F) -> EffectHandle {
    Self::create(body, RENDER)
  }

  fn create(body: F, flags: u8) -> EffectHandle {
    let body: Rc<dyn Computation> = Rc::new(EffectBody {
      body,
      value: RefCell::new(None),
    });
    let id = computation::create(body, flags);

    computation::run(id);

    EffectHandle {
      id,
      local: PhantomData,
    }
  }
}

/// Runs `operation` without subscribing the current effect to any signal it reads.
#[cfg_attr(debug_assertions, track_caller)]
pub fn untrack<T>(operation: impl FnOnce() -> T) -> T {
  Runtime::with_tracking(None, operation)
}

/// Runs `operation`, deferring dependent effects until it returns so a batch of signal writes
/// triggers each effect at most once.
#[cfg_attr(debug_assertions, track_caller)]
pub fn batch<T>(operation: impl FnOnce() -> T) -> T {
  RUNTIME.with(|runtime| {
    let _batch = runtime.enter_batch();

    operation()
  })
}
