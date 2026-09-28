//! Disposal: moving an entity out of its arena, then tearing it down outside the borrow.

use crate::{
  id::{Id, Kind},
  runtime::{Runtime, try_with_graph},
  sync::SYNC,
};

impl Id {
  /// Disposes the entity this id names and every descendant; its subscribers only
  /// lose it as a dependency, and effects its cleanups invalidate run once it ends.
  pub(crate) fn dispose(self) {
    if self.is_sync() {
      dispose_sync(self);

      return;
    }

    if !Runtime::is_ui_thread() {
      // Local disposal must run on the entity's own thread; route it there.
      SYNC.enqueue_disposals([(self, None)]);

      return;
    }

    let _batch = Runtime::enter_batch();

    Runtime::with_tracking(None, || dispose_local(self));
  }

  /// Tears down what this computation's last run made, before it runs again: disposes
  /// its children, runs its cleanups in reverse registration order and drops the
  /// contexts it provided.
  pub(crate) fn dispose_run(self) {
    let Some(Some(remains)) = try_with_graph(|graph| graph.take_run_remains(self))
    else {
      return;
    };

    Runtime::with_tracking(None, || {
      if let Some(children) = remains.children {
        for child in *children {
          child.dispose();
        }
      }

      for cleanup in remains.cleanups.into_iter().rev() {
        cleanup();
      }

      drop(remains.contexts);
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
        for child in *children {
          child.dispose();
        }
      }

      // After the children, so a parent's cleanup sees a fully torn-down subtree.
      // Reverse registration order, so a later callback unwinds before what it was
      // built on.
      for cleanup in removed.cleanups.into_iter().rev() {
        cleanup();
      }

      drop(removed.body);
      drop(removed.contexts);
    }
    Kind::Sync => {}
  }
}

/// Removes a sync signal from the registry and unlinks its UI-thread subscribers,
/// directly or through the pending-disposal queue.
fn dispose_sync(id: Id) {
  let Some(signal) = SYNC.remove(id)
  else {
    return;
  };

  if Runtime::is_ui_thread() {
    dispose_sync_local(id, signal.parent);
  }
  else {
    SYNC.enqueue_disposals([(id, signal.parent)]);
  }

  drop(signal.value);
}

/// The UI-thread half of a sync signal's disposal: unlinks it from `parent` and from
/// its subscribers here.
pub(crate) fn dispose_sync_local(id: Id, parent: Option<Id>) {
  let _ = try_with_graph(|graph| {
    if let Some(parent) = parent {
      graph.detach_child(parent, id);
    }

    graph.remove_sync_subs(id);
  });
}
