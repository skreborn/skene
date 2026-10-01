//! Owner-bound task wrapping, shared by every facade's spawner.

use std::cell::Cell;

use skene_reactive::{Owner, TaskGuard};

use crate::spine::Boundary;

thread_local! {
  /// Whether an event handler is running.
  static HANDLING: Cell<bool> = const { Cell::new(false) };
}

/// Runs `handler` as `owner`, as an event handler: a task it spawns counts toward no
/// boundary.
pub(in crate::spine) fn run_handler<T>(owner: Owner, handler: impl FnOnce() -> T) -> T {
  /// Restores the previous state when the handler returns or unwinds.
  struct Restore(bool);

  impl Drop for Restore {
    fn drop(&mut self) {
      HANDLING.set(self.0);
    }
  }

  let _restore = Restore(HANDLING.replace(true));

  owner.enter(handler)
}

/// Leaves its boundary when dropped.
struct Counted(Boundary);

impl Drop for Counted {
  fn drop(&mut self) {
    self.0.leave();
  }
}

/// Counts the task `guard` stands for as pending work toward the nearest enclosing
/// [`Boundary`].
///
/// It counts until the guard drops, as it does once the task finishes or is cancelled;
/// one an event handler starts counts toward no boundary. A facade's spawner wraps the
/// guard its executor hands back in it.
#[must_use = "dropping the guard cancels the task"]
pub fn count_pending(guard: TaskGuard) -> TaskGuard {
  let boundary = if HANDLING.get() {
    None
  }
  else {
    Boundary::current()
  };

  let Some(boundary) = boundary
  else {
    return guard;
  };

  boundary.enter();

  Box::new((guard, Counted(boundary)))
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, future::poll_fn, rc::Rc, task::Poll};

  use skene_reactive::{Owner, provide_context, spawn};

  use crate::spine::{Boundary, executor};

  /// Sets its flag when dropped.
  struct DropFlag(Rc<Cell<bool>>);

  impl Drop for DropFlag {
    fn drop(&mut self) {
      self.0.set(true);
    }
  }

  /// A task its owner's disposal cancels leaves its boundary at once, and its future is
  /// dropped without another poll.
  #[test]
  fn a_cancelled_task_leaves_its_boundary_and_is_never_polled_again() {
    executor::install();

    let boundary = Boundary::new();
    let owner = Owner::new();
    let polls = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(false));

    owner.enter(|| {
      provide_context(boundary);

      let polls = Rc::clone(&polls);
      let flag = DropFlag(Rc::clone(&dropped));

      spawn(poll_fn(move |_| {
        let _flag = &flag;

        polls.set(polls.get() + 1);

        Poll::Pending
      }));
    });

    assert!(executor::pump());
    assert_eq!(polls.get(), 1);
    assert!(!boundary.is_settled());

    owner.dispose();

    assert!(boundary.is_settled());

    let _ = executor::pump();

    assert!(dropped.get());
    assert_eq!(polls.get(), 1);
    assert_eq!(executor::task_count(), 0);
  }

  /// A finished task leaves its boundary once and nothing of it stays alive.
  #[test]
  fn a_finished_task_leaves_its_boundary_once() {
    executor::install();

    let boundary = Boundary::new();
    let owner = Owner::new();

    owner.enter(|| {
      provide_context(boundary);
      spawn(async {});
    });

    assert!(!boundary.is_settled());
    assert!(executor::pump());
    assert!(boundary.is_settled());
    assert_eq!(executor::task_count(), 0);

    owner.dispose();

    assert!(boundary.is_settled());
  }
}
