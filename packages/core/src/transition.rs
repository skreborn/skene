//! A transitional reactive subtree: each rebuild takes over only once it settles.

use std::{cell::Cell, rc::Rc};

use skene_reactive::{Effect, Owner, provide_context};

use crate::{Boundary, NodeId, tree::TreeOps};

/// Anchors a reactive subtree under a fresh group node, each rebuild replacing the
/// previous content only once its async work settles.
///
/// Like [`derived`](crate::derived), `build` reruns whenever its reactive dependencies
/// change; unlike it, each rebuild runs under a fresh [`Boundary`], stays detached while
/// that boundary is pending, and swaps in when it settles—immediately, when it has no
/// async work—so the previous content holds the screen instead of a placeholder. A
/// rebuild arriving while one is still pending discards the pending one: the latest
/// wins. Toward an enclosing boundary an unsettled rebuild counts as one pending task,
/// so a server render and the stream settle through it; pair with a
/// [`Suspense`](crate::Suspense) for the first load, which has no previous content to
/// hold.
pub fn transition<Ops: TreeOps>(build: impl Fn() -> NodeId + 'static) -> NodeId {
  let container = Ops::default().create_group();
  let parent_scope = Owner::current();
  let parent = Boundary::current();
  let current = Rc::new(Cell::new(None::<(NodeId, Owner)>));
  let pending = Rc::new(Cell::new(None::<(NodeId, Owner)>));
  // Whether unsettled rebuilds currently count as pending toward the enclosing
  // boundary, so a disposal mid-pend still balances the count.
  let entered = Rc::new(Cell::new(false));

  {
    let pending = Rc::clone(&pending);
    let entered = Rc::clone(&entered);

    parent_scope.on_cleanup(move || {
      if let Some((node, scope)) = pending.take() {
        Ops::default().destroy(node);
        scope.dispose();
      }

      if entered.get()
        && let Some(parent) = parent
      {
        parent.leave();
      }
    });
  }

  Effect::new(move |_| {
    // The latest rebuild wins: a still-pending previous one is discarded.
    if let Some((node, scope)) = pending.take() {
      Ops::default().destroy(node);
      scope.dispose();
    }

    let scope = parent_scope.child();
    let boundary = Boundary::new();
    let node = scope.enter(|| {
      provide_context(boundary);

      build()
    });

    pending.set(Some((node, scope)));

    let current = Rc::clone(&current);
    let pending = Rc::clone(&pending);
    let entered = Rc::clone(&entered);

    // The swap watcher lives in the rebuild's scope, so a discarded rebuild takes its
    // watcher with it.
    scope.enter(|| {
      Effect::new(move |done: Option<bool>| {
        if done == Some(true) {
          return true;
        }

        if !boundary.is_settled() {
          if !entered.replace(true)
            && let Some(parent) = parent
          {
            parent.enter();
          }

          return false;
        }

        let Some((node, scope)) = pending.take()
        else {
          return true;
        };

        Ops::default().attach(container, node, None);

        if let Some((old_node, old_scope)) = current.take() {
          Ops::default().detach(container, old_node);
          Ops::default().destroy(old_node);
          old_scope.dispose();
        }

        current.set(Some((node, scope)));

        if entered.replace(false)
          && let Some(parent) = parent
        {
          parent.leave();
        }

        true
      });
    });
  });

  container
}
