use std::rc::Rc;

use crate::{
  access::{Set, With},
  runtime::with_graph,
  signal::Signal,
};

/// A valueless signal: notify it to re-run every effect that tracked it.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct Trigger {
  signal: Signal<()>,
}

impl Trigger {
  /// Creates a trigger owned by the current [`Owner`](crate::Owner).
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      signal: Signal::transient(()),
    }
  }

  /// Re-runs every effect that tracked this trigger.
  #[inline(always)]
  pub fn notify(&self) {
    self.signal.set(());
  }

  /// Subscribes the running effect to this trigger.
  #[inline(always)]
  pub fn track(&self) {
    self.signal.with(|()| {});
  }

  /// Disposes the trigger, and `hook` with it, once nothing tracks it any more.
  pub(crate) fn dispose_when_unobserved(self, hook: impl Fn() + 'static) {
    let id = self.signal.id();

    with_graph(|graph| {
      graph.on_unobserved(
        id,
        Rc::new(move || {
          hook();
          id.dispose();
        }),
      );
    });
  }
}

impl Default for Trigger {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cell::{Cell, RefCell},
    rc::Rc,
  };

  use super::Trigger;
  use crate::{Effect, Owner, Runtime};

  /// The flushes a test's scheduler was handed, in order.
  type Flushes = Rc<RefCell<Vec<Box<dyn FnOnce()>>>>;

  /// A trigger's hook runs, and the trigger goes, once its last reader stops tracking
  /// it, and not while one still does.
  #[test]
  fn a_trigger_goes_once_nothing_tracks_it() {
    let owner = Owner::new();
    let trigger = owner.enter(Trigger::new);
    let gone = Rc::new(Cell::new(0));
    let readers: Vec<Owner> = (0..2).map(|_| owner.child()).collect();

    trigger.dispose_when_unobserved({
      let gone = Rc::clone(&gone);

      move || gone.set(gone.get() + 1)
    });

    for reader in &readers {
      reader.enter(|| {
        Effect::new(move |_| trigger.track());
      });
    }

    readers[0].dispose();

    assert_eq!(gone.get(), 0, "one reader still tracks it");

    readers[1].dispose();

    assert_eq!(gone.get(), 1);

    owner.dispose();
  }

  /// Under a scheduler, the hook runs with the flush that the last reader's disposal
  /// schedules.
  #[test]
  fn a_scheduled_flush_runs_the_hook() {
    let flushes = Flushes::default();

    Runtime::set_scheduler({
      let flushes = Rc::clone(&flushes);

      move |flush| flushes.borrow_mut().push(flush)
    });

    let owner = Owner::new();
    let trigger = owner.enter(Trigger::new);
    let gone = Rc::new(Cell::new(false));
    let reader = owner.child();

    trigger.dispose_when_unobserved({
      let gone = Rc::clone(&gone);

      move || gone.set(true)
    });

    reader.enter(|| {
      Effect::new(move |_| trigger.track());
    });

    Runtime::flush_sync();
    flushes.borrow_mut().clear();
    reader.dispose();

    assert!(!gone.get(), "the hook waits for the flush");

    let pending = flushes.take();

    assert_eq!(pending.len(), 1, "the disposal schedules a flush");

    for flush in pending {
      flush();
    }

    assert!(gone.get());

    owner.dispose();
    Runtime::reset();
  }
}
