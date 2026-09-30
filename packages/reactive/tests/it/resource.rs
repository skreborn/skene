//! Resources: loading through the installed executor, keeping the last value across a
//! reload.

use std::{
  cell::{Cell, RefCell},
  future::Future,
  pin::Pin,
  rc::Rc,
  task::{Context, Poll, Waker},
};

use skene_reactive::{Effect, LoadState, Owner, Resource, Task, TaskGuard, set_spawner};

/// A queued task, and whether its guard cancelled it.
type Queued = (Rc<Cell<bool>>, Task);

thread_local! {
  static TASKS: RefCell<Vec<Queued>> = const { RefCell::new(Vec::new()) };
}

/// Cancels its task: the queue drops it unpolled.
struct Cancel(Rc<Cell<bool>>);

impl Drop for Cancel {
  fn drop(&mut self) {
    self.0.set(true);
  }
}

/// Queues `task` until [`pump`] polls it, guarded by a [`Cancel`].
fn queue(task: Task) -> TaskGuard {
  let cancelled = Rc::new(Cell::new(false));

  TASKS.with(|tasks| tasks.borrow_mut().push((Rc::clone(&cancelled), task)));

  Box::new(Cancel(cancelled))
}

/// Installs a queue executor: tasks wait until [`pump`] polls them.
fn install_executor() {
  set_spawner(queue);
}

/// Polls every queued task once, requeueing the pending ones and dropping the
/// cancelled ones unpolled.
fn pump() {
  let tasks = TASKS.with(|tasks| std::mem::take(&mut *tasks.borrow_mut()));
  let mut context = Context::from_waker(Waker::noop());

  for (cancelled, mut task) in tasks {
    if !cancelled.get() && task.as_mut().poll(&mut context).is_pending() {
      TASKS.with(|tasks| tasks.borrow_mut().push((cancelled, task)));
    }
  }
}

/// A future ready on its `n`th poll.
struct After(u32);

impl Future for After {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
    if self.0 == 0 {
      return Poll::Ready(());
    }

    self.0 -= 1;

    Poll::Pending
  }
}

/// A resource is loading until its task lands, and a reader wakes when it does.
#[test]
fn a_resource_loads_through_the_executor() {
  install_executor();

  let scope = Owner::new();
  let (resource, seen) = scope.enter(|| {
    let resource = Resource::new(|| async {
      After(1).await;

      7u32
    });
    let seen = Rc::new(Cell::new(None));

    Effect::new({
      let seen = Rc::clone(&seen);

      move |_| seen.set(resource.get())
    });

    (resource, seen)
  });

  assert_eq!(resource.state(), LoadState::Loading);
  assert!(resource.is_loading());

  pump();
  pump();

  assert_eq!(resource.state(), LoadState::Ready);
  assert_eq!(seen.get(), Some(7));
}

/// A reload keeps the last value readable as refetching, and an overtaken load lands
/// nowhere.
#[test]
fn reload_keeps_the_last_value_until_the_next_lands() {
  install_executor();

  let scope = Owner::new();
  let runs = Rc::new(Cell::new(0u32));
  let resource = scope.enter(|| {
    Resource::new({
      let runs = Rc::clone(&runs);

      move || {
        let run = runs.get();

        runs.set(run + 1);

        async move {
          After(run).await;

          run
        }
      }
    })
  });

  pump();

  assert_eq!(resource.get(), Some(0));

  resource.reload();

  assert_eq!(resource.state(), LoadState::Refetching);
  assert!(resource.is_refetching());
  assert_eq!(resource.get(), Some(0), "the last value stays readable");

  resource.reload();
  pump();
  pump();
  pump();

  assert_eq!(resource.state(), LoadState::Ready);
  assert_eq!(resource.get(), Some(2), "the overtaken load did not land");
}

/// `ready` starts settled without running the loader; `unloaded` never starts it.
#[test]
fn ready_and_unloaded_skip_the_first_load() {
  install_executor();

  let scope = Owner::new();
  let (ready, unloaded) = scope.enter(|| {
    (
      Resource::ready(3u8, || async { panic!("the loader does not run") }),
      Resource::unloaded(|| async { panic!("the loader does not run") }),
    )
  });

  pump();

  assert_eq!(ready.state(), LoadState::Ready);
  assert_eq!(ready.get(), Some(3));
  assert_eq!(unloaded.state(), LoadState::Loading);
  assert_eq!(unloaded.get(), None::<u8>);
}

/// Every load runs under the owner the resource was created under, a reload asked for
/// from elsewhere included.
#[test]
fn a_reload_runs_under_the_resources_owner() {
  let spawned_under = Rc::new(RefCell::new(Vec::new()));

  set_spawner({
    let spawned_under = Rc::clone(&spawned_under);

    move |task| {
      spawned_under.borrow_mut().push(Owner::current());

      queue(task)
    }
  });

  let scope = Owner::new();
  let resource = scope.enter(|| Resource::new(|| async { 7u32 }));
  let elsewhere = Owner::new();

  elsewhere.enter(|| resource.reload());

  assert_eq!(*spawned_under.borrow(), [scope, scope]);
}

/// A reload cancels the load it overtakes: that load is dropped, never polled to its
/// end.
#[test]
fn a_reload_cancels_the_load_it_overtakes() {
  install_executor();

  let scope = Owner::new();
  let finished = Rc::new(Cell::new(0u32));
  let resource = scope.enter(|| {
    Resource::new({
      let finished = Rc::clone(&finished);

      move || {
        let finished = Rc::clone(&finished);

        async move {
          After(1).await;
          finished.set(finished.get() + 1);

          1u32
        }
      }
    })
  });

  resource.reload();
  pump();
  pump();

  assert_eq!(finished.get(), 1, "only the last load ran to its end");
  assert_eq!(resource.get(), Some(1));
  assert_eq!(TASKS.with(|tasks| tasks.borrow().len()), 0);
}
