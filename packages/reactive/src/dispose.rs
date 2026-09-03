//! Disposal: moving an entity out of its arena, then tearing it down outside the borrow.

use crate::{
  id::{Id, Kind},
  runtime::{Runtime, try_with_graph},
  sync::SYNC,
};

impl Id {
  /// Disposes the entity this id names and every descendant.
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

    dispose_local(self);
  }

  /// Disposes only the children of this id, keeping the entity itself.
  pub(crate) fn dispose_children(self) {
    if let Some(Some(children)) = try_with_graph(|graph| graph.take_children(self)) {
      for child in *children {
        child.dispose();
      }
    }
  }
}

fn dispose_local(id: Id) {
  match id.kind() {
    Kind::Signal => {
      let Some(Some(removed)) = try_with_graph(|graph| graph.remove_signal(id))
      else {
        return;
      };

      for sub in removed.subs {
        sub.dispose();
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

      for sub in removed.subs {
        sub.dispose();
      }
    }
    Kind::Sync => {}
  }
}

/// Removes a sync signal from the registry and disposes its UI-thread subscribers,
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

/// The UI-thread half of a sync signal's disposal: unlinks it from `parent` and
/// disposes its subscribers here.
pub(crate) fn dispose_sync_local(id: Id, parent: Option<Id>) {
  let Some(subs) = try_with_graph(|graph| {
    if let Some(parent) = parent {
      graph.detach_child(parent, id);
    }

    graph.remove_sync_subs(id)
  })
  else {
    return;
  };

  for sub in subs {
    sub.dispose();
  }
}
