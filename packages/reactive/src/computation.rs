//! Running effects and memos: tracked runs, the pull path, and the queue step.

use std::{any::Any, rc::Rc};

use crate::{
  graph::flags::{DIRTY, PAUSED, PENDING, QUEUED, RUNNING},
  id::{Id, Kind},
  link::NONE,
  runtime::{RUNTIME, Runtime, try_with_graph, with_graph},
};

/// A computation's body, run under tracking as the computation itself.
pub(crate) trait Computation: Any {
  /// Runs the body; `true` when a memo's value changed.
  fn run(&self) -> bool;

  /// The body as a `dyn Any` for downcasting.
  fn as_any(&self) -> &dyn Any;
}

/// Allocates a computation owned by the current owner.
pub(crate) fn create(body: Rc<dyn Computation>, flags: u8) -> Id {
  RUNTIME.with(|runtime| {
    let owner = runtime.current_owner.get();
    let mut graph = runtime.graph();
    let id = graph.allocate_computation(body, flags);

    graph.adopt(id, owner);

    id
  })
}

/// Allocates a computation owned by nobody.
pub(crate) fn create_detached(body: Rc<dyn Computation>, flags: u8) -> Id {
  with_graph(|graph| graph.allocate_computation(body, flags))
}

/// Ends the tracked run of a computation, on return or unwind.
struct EndTracking(Id);

impl Drop for EndTracking {
  fn drop(&mut self) {
    let _ = try_with_graph(|graph| graph.end_tracking(self.0));
  }
}

/// Runs `f` as computation `id`: its reads become `id`'s dependencies, replacing last
/// run's, and anything reactive it creates belongs to `id`.
pub(crate) fn track<T>(id: Id, f: impl FnOnce() -> T) -> T {
  with_graph(|graph| graph.start_tracking(id));

  let _end = EndTracking(id);

  Runtime::with_tracking(Some(id), || Runtime::with_owner(id, f))
}

/// Re-runs computation `id` from scratch: what its last run made is torn down first.
///
/// The run is a batch: an effect it invalidates, itself included, runs once it ends.
pub(crate) fn run(id: Id) {
  let Some(body) = with_graph(|graph| graph.body(id))
  else {
    return;
  };

  let _batch = Runtime::enter_batch();

  id.dispose_run();
  with_graph(|graph| graph.reset_creation_counters(id));

  let changed = track(id, || body.run());

  if changed {
    with_graph(|graph| graph.bump_version(id));
  }
}

/// One queue step: runs a dirty effect, checks a pending one first, skips a paused one.
pub(crate) fn run_queued(id: Id) {
  let Some(flags) = with_graph(|graph| graph.update_flags(id, 0, QUEUED))
  else {
    return;
  };

  if flags & QUEUED == 0 || flags & PAUSED != 0 {
    return;
  }

  settle(id, flags);
}

/// Brings a memo up to date before it is read; a memo mid-run is left alone.
pub(crate) fn ensure_fresh(id: Id) {
  let Some(flags) = with_graph(|graph| graph.flags(id))
  else {
    return;
  };

  if flags & RUNNING != 0 {
    return;
  }

  settle(id, flags);
}

/// Runs an invalidated computation if a dependency really changed.
fn settle(id: Id, flags: u8) {
  if flags & DIRTY != 0 {
    run(id);
  }
  else if flags & PENDING != 0 {
    if check_dirty(id) {
      run(id);
    }
    else {
      with_graph(|graph| graph.update_flags(id, 0, PENDING));
    }
  }
}

/// Whether any dependency of `sub` changed since `sub` last read it, refreshing the
/// memos among them first.
pub(crate) fn check_dirty(sub: Id) -> bool {
  let mut index = with_graph(|graph| graph.first_dep(sub));

  while index != NONE {
    let link = with_graph(|graph| graph.link_at(index));

    if link.sub != sub {
      return true;
    }

    if link.dep.kind() == Kind::Computation {
      ensure_fresh(link.dep);
    }

    let (changed, next) = with_graph(|graph| {
      let current = graph.link_at(index);

      if current.sub != sub || current.dep != link.dep {
        return (true, NONE);
      }

      let changed = graph
        .dep_version(link.dep)
        .is_none_or(|version| version != current.version);

      (changed, current.next_dep)
    });

    if changed {
      return true;
    }

    index = next;
  }

  false
}

/// Subscribes the tracking computation, if any, to `dep`.
#[inline(always)]
pub(crate) fn subscribe(dep: Id) {
  RUNTIME.with(|runtime| {
    if let Some(sub) = runtime.tracking.get() {
      runtime.graph().link(dep, sub);
    }
  });
}

/// Tracks `id` as a dependency, whatever it is; `false` when it is disposed.
pub(crate) fn track_id(id: Id) -> bool {
  if id.kind() == Kind::Computation {
    if with_graph(|graph| graph.computation(id).is_none()) {
      return false;
    }

    ensure_fresh(id);
    subscribe(id);

    return true;
  }

  match id.state() {
    Some(state) => {
      state.subscribe();

      true
    }
    None => false,
  }
}
