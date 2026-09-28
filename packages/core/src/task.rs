//! Owner-bound task wrapping, shared by every facade's spawner.

use std::{
  cell::{Cell, RefCell},
  future::Future,
  pin::Pin,
  rc::Rc,
  task::{Context, Poll, Waker},
};

use skene_reactive::Owner;

use crate::Boundary;

thread_local! {
  /// Whether an event handler is running.
  static HANDLING: Cell<bool> = const { Cell::new(false) };
}

/// Runs `handler` as `owner`, as an event handler: a task it spawns counts toward no
/// boundary.
pub(crate) fn run_handler<T>(owner: Owner, handler: impl FnOnce() -> T) -> T {
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

/// What a task shares with the cleanup that cancels it.
struct Cancel {
  /// Whether the task's scope is gone.
  disposed: Cell<bool>,
  /// The waker of the task's last pending poll.
  waker: RefCell<Option<Waker>>,
  /// The boundary the task still counts toward.
  boundary: Cell<Option<Boundary>>,
}

impl Cancel {
  /// Stops counting the task toward its boundary; later calls do nothing.
  fn leave(&self) {
    if let Some(boundary) = self.boundary.take() {
      boundary.leave();
    }
  }

  /// Cancels the task: it counts toward its boundary no longer and ends at its next
  /// poll, which this wakes.
  fn cancel(&self) {
    self.disposed.set(true);
    self.leave();

    if let Some(waker) = self.waker.take() {
      waker.wake();
    }
  }
}

/// A future that ends at its next poll once its scope is gone, without polling what it
/// wraps again.
struct UntilDisposed {
  future: Pin<Box<dyn Future<Output = ()>>>,
  cancel: Rc<Cancel>,
}

impl Future for UntilDisposed {
  type Output = ();

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
    let this = self.get_mut();

    if this.cancel.disposed.get() {
      return Poll::Ready(());
    }

    match this.future.as_mut().poll(context) {
      Poll::Ready(()) => {
        this.cancel.leave();

        Poll::Ready(())
      }
      Poll::Pending => {
        let mut waker = this.cancel.waker.borrow_mut();

        if !waker
          .as_ref()
          .is_some_and(|waker| waker.will_wake(context.waker()))
        {
          *waker = Some(context.waker().clone());
        }

        Poll::Pending
      }
    }
  }
}

/// Wraps `future` for a facade's executor: it ends at its next poll once the current
/// [`Owner`] is disposed, woken so that poll comes at once, and it counts as pending
/// work toward the nearest enclosing [`Boundary`] until it finishes or is cancelled.
///
/// Owner-binding is the cancellation contract every facade's `spawn` shares: a task
/// started while building a view ends with that view—or with any ancestor of it—rather
/// than waking later to write into a tree that is gone. A task started under an owner
/// already disposed never runs, and one an event handler starts counts toward no
/// boundary.
pub fn scope_bound(future: impl Future<Output = ()> + 'static) -> impl Future<Output = ()> {
  let boundary = if HANDLING.get() {
    None
  }
  else {
    Boundary::current()
  };

  if let Some(boundary) = boundary {
    boundary.enter();
  }

  let cancel = Rc::new(Cancel {
    disposed: Cell::new(false),
    waker: RefCell::new(None),
    boundary: Cell::new(boundary),
  });

  Owner::current().on_cleanup({
    let cancel = Rc::clone(&cancel);

    move || cancel.cancel()
  });

  UntilDisposed {
    future: Box::pin(future),
    cancel,
  }
}

#[cfg(test)]
mod tests {
  use std::{
    future::{Future, pending},
    pin::pin,
    sync::{
      Arc,
      atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
  };

  use skene_reactive::{Owner, provide_context};

  use super::scope_bound;
  use crate::Boundary;

  /// Counts its wakes.
  #[derive(Default)]
  struct Wakes(AtomicUsize);

  impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
      self.0.fetch_add(1, Ordering::Relaxed);
    }
  }

  #[test]
  fn a_cancelled_task_leaves_its_boundary_and_wakes() {
    let boundary = Boundary::new();
    let owner = Owner::new();
    let task = owner.enter(|| {
      provide_context(boundary);

      scope_bound(pending())
    });
    let counted = Arc::new(Wakes::default());
    let waker = Waker::from(Arc::clone(&counted));
    let mut task = pin!(task);

    assert!(
      task
        .as_mut()
        .poll(&mut Context::from_waker(&waker))
        .is_pending()
    );
    assert!(!boundary.is_settled());

    owner.dispose();

    assert!(boundary.is_settled());
    assert_eq!(counted.0.load(Ordering::Relaxed), 1);
    assert!(
      task
        .as_mut()
        .poll(&mut Context::from_waker(&waker))
        .is_ready()
    );
  }

  #[test]
  fn a_finished_task_leaves_its_boundary_once() {
    let boundary = Boundary::new();
    let owner = Owner::new();
    let task = owner.enter(|| {
      provide_context(boundary);

      scope_bound(async {})
    });

    assert_eq!(
      pin!(task).poll(&mut Context::from_waker(Waker::noop())),
      Poll::Ready(())
    );
    assert!(boundary.is_settled());

    owner.dispose();

    assert!(boundary.is_settled());
  }

  #[test]
  fn a_task_started_under_a_disposed_owner_never_runs() {
    let owner = Owner::new();

    owner.dispose();

    let task = owner.enter(|| scope_bound(async { panic!("the task ran") }));

    assert_eq!(
      pin!(task).poll(&mut Context::from_waker(Waker::noop())),
      Poll::Ready(())
    );
  }
}
