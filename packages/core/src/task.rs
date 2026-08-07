//! Owner-bound task wrapping, shared by every facade's spawner.

use std::{
  cell::Cell,
  future::Future,
  pin::Pin,
  rc::Rc,
  task::{Context, Poll},
};

use skene_reactive::Owner;

use crate::Boundary;

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

/// Wraps `future` for a facade's executor: it stops at its next poll once the current
/// [`Owner`] is disposed, and it counts as pending work toward the nearest enclosing
/// [`Boundary`] until it finishes.
///
/// Owner-binding is the cancellation contract every facade's `spawn` shares: a task
/// started while building a view ends with that view—or with any ancestor of it—rather
/// than waking later to write into a tree that is gone. The boundary is captured here,
/// while it is in `Context`; the future runs off the build scope, where it no longer is.
pub fn scope_bound(future: impl Future<Output = ()> + 'static) -> impl Future<Output = ()> {
  let disposed = Rc::new(Cell::new(false));

  Owner::current().on_cleanup({
    let disposed = Rc::clone(&disposed);

    move || disposed.set(true)
  });

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

  UntilDisposed {
    future: Box::pin(future),
    disposed,
  }
}
