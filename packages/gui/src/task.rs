//! Spawning scope-bound tasks.

use std::{
  cell::Cell,
  future::Future,
  pin::Pin,
  rc::Rc,
  task::{Context, Poll},
};

use skene_core::Boundary;
use skene_reactive::Scope;

use crate::driver;

/// A future that stops at its next poll once its scope is gone.
///
/// The executor owns the future once it is handed over, so there is nothing left to drop from
/// the outside; a flag checked before each poll is what stops the work instead. It is checked
/// *before* delegating, so once the scope is disposed no more of the caller's code runs—the
/// task cannot wake up and write into a torn-down tree.
struct UntilDisposed {
  future: Pin<Box<dyn Future<Output = ()>>>,
  disposed: Rc<Cell<bool>>,
}

impl Future for UntilDisposed {
  type Output = ();

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
    // Both fields are `Unpin`, so this needs no projection and no `unsafe`.
    let this = self.get_mut();

    if this.disposed.get() {
      return Poll::Ready(());
    }

    this.future.as_mut().poll(context)
  }
}

/// Runs `future` as a task, until it finishes or the current [`Scope`] is disposed.
///
/// Scope-bound on purpose: a task started while building a view ends with that view—or with
/// any ancestor of it—rather than waking later to write into a tree that is gone.
///
/// The future runs on the target's platform executor: on the web that is the event loop, so it
/// resumes on the UI thread and may touch signals and views directly. Cancellation takes effect
/// at the future's next poll, so a task parked on a pending await stops there rather than
/// resuming into a disposed tree.
///
/// This is platform-independent: it wraps the future for scope-bound cancellation, then hands
/// it to the leaf's executor fill. A leaf implements only that fill.
#[inline(always)]
pub fn spawn(future: impl Future<Output = ()> + 'static) {
  let disposed = Rc::new(Cell::new(false));

  Scope::current().on_cleanup({
    let disposed = Rc::clone(&disposed);

    move || disposed.set(true)
  });

  // Report to the nearest enclosing boundary, so its subtree counts as pending until every
  // task started within it finishes. Captured here, while the boundary is in `Context`; the
  // future runs off the build scope, where it no longer is.
  let boundary = Boundary::current();

  if let Some(boundary) = boundary {
    boundary.enter();
  }

  let future = async move {
    future.await;

    if let Some(boundary) = boundary {
      boundary.leave();
    }
  };

  driver::drive(UntilDisposed {
    future: Box::pin(future),
    disposed,
  });
}
