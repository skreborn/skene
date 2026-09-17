//! The task seam: a facade's platform installs its executor, the engine spawns through
//! it.

use std::{cell::RefCell, future::Future, pin::Pin, rc::Rc};

/// A boxed task, as the installed executor receives it.
pub type Task = Pin<Box<dyn Future<Output = ()>>>;

/// The installed executor.
type Spawner = dyn Fn(Task);

thread_local! {
  static SPAWNER: RefCell<Option<Rc<Spawner>>> = const { RefCell::new(None) };
}

/// Installs the executor [`spawn`] hands its tasks to on this thread, replacing any
/// previous one; it outlives a [`Runtime::reset`](crate::Runtime::reset), as the
/// thread's executor does.
///
/// A platform installs its own—resuming on the UI thread and bound to the spawning
/// owner—so a [`Resource`](crate::Resource) loads the same way on every platform.
pub fn set_spawner(spawner: impl Fn(Task) + 'static) {
  let previous = SPAWNER.with(|current| current.borrow_mut().replace(Rc::new(spawner)));

  drop(previous);
}

/// Runs `future` as a task on the installed executor.
///
/// # Panics
///
/// Panics when no executor is installed on this thread.
pub fn spawn(future: impl Future<Output = ()> + 'static) {
  let spawner = SPAWNER
    .with(|current| current.borrow().clone())
    .expect("no task executor installed on this thread; a platform installs one");

  spawner(Box::pin(future));
}
