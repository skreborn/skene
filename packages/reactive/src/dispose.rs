//! Disposal: moving an entity out of its arena, then tearing it down outside the borrow.

use crate::{
  graph::RunRemains,
  id::{Id, Kind},
  runtime::{RUNTIME, Runtime, try_with_graph},
};

impl Id {
  /// Disposes the entity this id names and every descendant; its subscribers only
  /// lose it as a dependency, and effects its cleanups invalidate run once it ends.
  pub(crate) fn dispose(self) {
    RUNTIME.with(|runtime| {
      let _batch = runtime.enter_batch();

      Runtime::with_tracking(None, || dispose_local(self));
    });
  }
}

impl RunRemains {
  /// Tears down what a computation's last run made, before it runs again, untracked:
  /// disposes its children newest first, cancels its tasks, runs its cleanups in
  /// reverse registration order and drops the contexts it provided.
  pub(crate) fn dispose(self) {
    Runtime::with_tracking(None, || {
      if let Some(children) = self.children {
        for child in children.iter().rev() {
          child.dispose();
        }
      }

      drop(self.tasks);

      for cleanup in self.cleanups.into_iter().rev() {
        cleanup();
      }

      drop(self.contexts);
    });
  }
}

fn dispose_local(id: Id) {
  match id.kind() {
    Kind::Signal => {
      let Some(Some(removed)) = try_with_graph(|graph| graph.remove_signal(id))
      else {
        return;
      };

      if removed.snapshotted {
        crate::snapshot::unregister(id);
      }

      drop(removed.value);
    }
    Kind::Computation | Kind::Owner => {
      let Some(Some(removed)) = try_with_graph(|graph| graph.remove_node(id))
      else {
        return;
      };

      if let Some(children) = removed.children {
        for child in children.iter().rev() {
          child.dispose();
        }
      }

      drop(removed.tasks);

      // After the children, so a parent's cleanup sees a fully torn-down subtree.
      // Reverse registration order, so a later callback unwinds before what it was
      // built on.
      for cleanup in removed.cleanups.into_iter().rev() {
        cleanup();
      }

      drop(removed.body);
      drop(removed.contexts);
    }
  }
}
