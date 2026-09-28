//! The task seam: a facade's platform installs its executor, the engine spawns through
//! it, and each task belongs to the owner it was spawned under.

use std::{any::Any, cell::RefCell, future::Future, pin::Pin, rc::Rc};

use crate::{
  graph::Graph,
  id::Id,
  runtime::{RUNTIME, try_with_graph, with_graph},
};

/// A boxed task, as the installed executor receives it.
pub type Task = Pin<Box<dyn Future<Output = ()>>>;

/// A running task's guard, as the installed executor hands it back: dropping it cancels
/// the task, which is never polled again.
pub type TaskGuard = Box<dyn Any>;

/// The installed executor.
type Spawner = dyn Fn(Task) -> TaskGuard;

thread_local! {
  static SPAWNER: RefCell<Option<Rc<Spawner>>> = const { RefCell::new(None) };
}

/// Installs the executor [`spawn`] hands its tasks to on this thread, replacing any
/// previous one; it outlives a [`Runtime::reset`](crate::Runtime::reset), as the
/// thread's executor does.
///
/// A platform installs its own, resuming on the UI thread, so a
/// [`Resource`](crate::Resource) loads the same way on every platform. The guard it
/// returns for a task cancels the task when dropped.
pub fn set_spawner(spawner: impl Fn(Task) -> TaskGuard + 'static) {
  let previous = SPAWNER.with(|current| current.borrow_mut().replace(Rc::new(spawner)));

  drop(previous);
}

/// A spawned task, to cancel it by; dropping the handle leaves the task running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskHandle {
  owner: Id,
  id: u64,
}

impl TaskHandle {
  /// Cancels the task: it is never polled again. Does nothing once it finished or its
  /// owner went.
  pub fn cancel(self) {
    let guard = try_with_graph(|graph| graph.take_task(self.owner, self.id)).flatten();

    drop(guard);
  }
}

/// Runs `future` as a task of the current owner on the installed executor.
///
/// The task ends with its owner—disposed, or run again for an effect or a memo—or
/// through the returned handle, and is then never polled again; one spawned under an
/// owner already gone never runs.
///
/// # Panics
///
/// Panics when no executor is installed on this thread.
pub fn spawn(future: impl Future<Output = ()> + 'static) -> TaskHandle {
  let spawner = SPAWNER
    .with(|current| current.borrow().clone())
    .expect("no task executor installed on this thread; a platform installs one");
  let owner = RUNTIME.with(|runtime| runtime.current_owner.get());
  let id = with_graph(Graph::next_task);
  let guard = spawner(Box::pin(async move {
    future.await;

    let guard = try_with_graph(|graph| graph.take_task(owner, id)).flatten();

    drop(guard);
  }));
  let refused = with_graph(|graph| graph.adopt_task(owner, id, guard));

  drop(refused);

  TaskHandle { owner, id }
}
