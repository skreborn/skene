//! Running effects and memos: tracked runs, the pull path, and the queue step.

use std::{any::Any, rc::Rc};

use smallvec::SmallVec;

use crate::{
  graph::{
    CheckStep,
    flags::{CHECKING, DIRTY, PAUSED, PENDING, QUEUED, RUNNING},
  },
  id::{Id, Kind},
  link::{LinkIdx, NONE},
  runtime::{RUNTIME, Runtime, with_graph},
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

/// A computation's tracked run: on return or unwind, restores the tracking computation
/// and the owner it replaced, then drops the edges the run did not read.
struct Tracked<'a> {
  runtime: &'a Runtime,
  id: Id,
  tracking: Option<Id>,
  owner: Id,
}

impl Drop for Tracked<'_> {
  fn drop(&mut self) {
    self.runtime.tracking.set(self.tracking);
    self.runtime.current_owner.set(self.owner);
    self.runtime.graph().end_tracking(self.id);
  }
}

/// Runs `operation` as computation `id`: its reads become `id`'s dependencies, replacing last
/// run's, and anything reactive it creates belongs to `id`.
pub(crate) fn track<T>(id: Id, operation: impl FnOnce() -> T) -> T {
  RUNTIME.with(|runtime| track_in(runtime, id, operation))
}

/// [`track`] on `runtime`, this thread's.
fn track_in<T>(runtime: &Runtime, id: Id, operation: impl FnOnce() -> T) -> T {
  runtime.graph().start_tracking(id);

  let _tracked = Tracked {
    runtime,
    id,
    tracking: runtime.tracking.replace(Some(id)),
    owner: runtime.current_owner.replace(id),
  };

  operation()
}

/// Re-runs computation `id` from scratch: what its last run made is torn down first.
///
/// The run is a batch: an effect it invalidates, itself included, runs once it ends.
pub(crate) fn run(id: Id) {
  RUNTIME.with(|runtime| {
    let Some((body, remains)) = runtime.graph().begin_run(id)
    else {
      return;
    };

    let _batch = runtime.enter_run_batch();

    if let Some(remains) = remains {
      remains.dispose();
    }

    let changed = track_in(runtime, id, || body.run());

    if changed {
      runtime.graph().bump_version(id);
    }
  });
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
      with_graph(|graph| graph.update_flags(id, 0, PENDING | CHECKING));
    }
  }
}

/// Whether any dependency of `root` changed since `root` last read it, refreshing the
/// memos among them first; `root` is left on the check, for its run or the caller to
/// take it off.
///
/// The walk keeps its own stack rather than the call stack: a pending memo is checked
/// in turn, and one whose dependency changed runs before the check it sits in goes on,
/// deepest first, so a run finds its dependencies fresh and nests nothing. A memo
/// already on the stack is not descended into again, so a cycle ends.
#[inline(never)]
pub(crate) fn check_dirty(root: Id) -> bool {
  let mut stack: SmallVec<[(Id, LinkIdx); 16]> = SmallVec::new();

  stack.push((root, with_graph(|graph| graph.begin_check(root))));

  loop {
    let (sub, index) = *stack.last().expect("the root's frame");

    if index == NONE {
      if stack.len() == 1 {
        break false;
      }

      stack.pop();
      with_graph(|graph| graph.update_flags(sub, 0, PENDING | CHECKING));

      continue;
    }

    let unchanged = match with_graph(|graph| graph.check_edge(sub, index)) {
      CheckStep::Changed => None,
      CheckStep::Unchanged(next) => Some(next),
      CheckStep::Run(dep) => {
        run(dep);

        with_graph(|graph| graph.unchanged_edge(sub, index, dep))
      }
      CheckStep::Descend(dep, first) => {
        stack.push((dep, first));

        continue;
      }
    };

    if let Some(next) = unchanged {
      stack.last_mut().expect("the frame").1 = next;

      continue;
    }

    if stack.len() == 1 {
      break true;
    }

    stack.pop();
    run(sub);
  }
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
