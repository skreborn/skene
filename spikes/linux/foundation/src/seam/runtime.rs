//! Spawning and timers on the platform's main loop, bound to the current owner.

use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
  time::Duration,
};

use crate::{Owner, seam::Realization};

/// A task the realization polls on the platform's main loop.
pub type Task = Pin<Box<dyn Future<Output = ()>>>;

/// Cancels what it was returned for: a task or a timer.
#[must_use]
pub struct Cancel(pub Option<Box<dyn FnOnce()>>);

impl Cancel {
  /// Cancels it, where it has not run.
  pub fn cancel(mut self) {
    if let Some(cancel) = self.0.take() {
      cancel();
    }
  }
}

/// A future polled under the owner it was spawned under.
struct Owned<F> {
  owner: Option<Owner>,
  future: Pin<Box<F>>,
}

impl<F: Future<Output = ()>> Future for Owned<F> {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
    let owner = self.owner;
    let future = self.future.as_mut();

    match owner {
      Some(owner) => owner.run(|| future.poll(context)),
      None => future.poll(context),
    }
  }
}

/// Runs `task` on the platform's main loop under the current owner: what it creates belongs
/// to that owner, and it is dropped, never polled again, when that owner is disposed.
pub fn spawn<R: Realization>(task: impl Future<Output = ()> + 'static) {
  let owner = Owner::current();
  let cancel = R::spawn_local(Box::pin(Owned {
    owner,
    future: Box::pin(task),
  }));

  if owner.is_some() {
    Owner::on_cleanup(move || cancel.cancel());
  } else {
    std::mem::forget(cancel);
  }
}

/// Runs `action` once after `delay` on the platform's clock, under the current owner, and
/// cancelled when that owner is disposed first.
pub fn schedule<R: Realization>(delay: Duration, action: impl FnOnce() + 'static) {
  let owner = Owner::current();
  let cancel = R::schedule(
    delay,
    Box::new(move || match owner {
      Some(owner) => owner.run(action),
      None => action(),
    }),
  );

  if owner.is_some() {
    Owner::on_cleanup(move || cancel.cancel());
  } else {
    std::mem::forget(cancel);
  }
}
