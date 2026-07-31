use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
  effect::observer_clean_up,
  runtime::{RUNTIME, Runtime},
  signal::SignalState,
  sync_runtime::SYNC_RUNTIME,
};

/// An internal id which can reference a signal/`Effect`/`Scope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(u64);

impl Id {
  /// Creates a new `Id` that's next in order.
  pub(crate) fn next() -> Id {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    Id(COUNTER.fetch_add(1, Ordering::Relaxed))
  }

  /// Tries to get the signal that links with this `Id`.
  pub(crate) fn signal(&self) -> Option<SignalState> {
    if Runtime::is_ui_thread() {
      if let Some(signal) = RUNTIME.with(|runtime| {
        runtime
          .entities
          .borrow()
          .get(self)
          .and_then(|entity| entity.signal.clone())
      }) {
        return Some(signal);
      }

      SYNC_RUNTIME.get_signal(self).map(Into::into)
    }
    else {
      SYNC_RUNTIME.get_signal(self).map(Into::into)
    }
  }

  /// Reads the local (unsync) signal for this id, asserting the UI thread.
  ///
  /// For unsync signal handles, whose storage is always local: skips the
  /// local-vs-sync dispatch [`signal`](Self::signal) performs.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn local_signal(&self) -> Option<SignalState> {
    Runtime::assert_ui_thread();

    RUNTIME.with(|runtime| {
      runtime
        .entities
        .borrow()
        .get(self)
        .and_then(|entity| entity.signal.clone())
    })
  }

  /// Tries to set the signal to be linking with this `Id`.
  pub(crate) fn add_signal(&self, signal: SignalState) {
    RUNTIME.with(|runtime| {
      runtime
        .entities
        .borrow_mut()
        .entry(*self)
        .or_default()
        .signal = Some(signal);
    });
  }

  /// Makes this `Id` a child of the current `Scope`.
  pub(crate) fn set_scope(&self) {
    RUNTIME.with(|runtime| {
      let scope = *runtime.current_scope.borrow();
      let mut entities = runtime.entities.borrow_mut();

      entities
        .entry(scope)
        .or_default()
        .children
        .get_or_insert_default()
        .insert(*self);

      entities.entry(*self).or_default().parent = Some(scope);
    });
  }

  /// Disposes only the children of this `Id` without removing resources tied to the `Id` itself.
  pub(crate) fn dispose_children(&self) {
    if let Ok(Some(children)) = RUNTIME.try_with(|runtime| {
      runtime
        .entities
        .borrow_mut()
        .get_mut(self)
        .and_then(|entity| entity.children.take())
    }) {
      for child in *children {
        child.dispose();
      }
    }
  }

  /// Disposes the resources linked to this `Id` and all of its descendants.
  pub(crate) fn dispose(&self) {
    if !Runtime::is_ui_thread() {
      // Disposal must run on the UI thread.
      SYNC_RUNTIME.enqueue_disposals([*self]);

      return;
    }

    let Ok(entity) = RUNTIME.try_with(|runtime| {
      let mut entities = runtime.entities.borrow_mut();
      let entity = entities.remove(self);

      // Unlink from the parent, so a long-lived scope's children set does not accumulate
      // disposed ids.
      if let Some(parent) = entity.as_ref().and_then(|entity| entity.parent)
        && let Some(parent_entity) = entities.get_mut(&parent)
        && let Some(children) = parent_entity.children.as_mut()
      {
        children.remove(self);
      }

      entity
    })
    else {
      if let Some(signal) = SYNC_RUNTIME.remove_signal(self) {
        Self::cleanup_signal(Some(signal.into()));
      }

      return;
    };

    let Some(entity) = entity
    else {
      if let Some(signal) = SYNC_RUNTIME.remove_signal(self) {
        Self::cleanup_signal(Some(signal.into()));
      }

      return;
    };

    if let Some(children) = entity.children {
      for child in *children {
        child.dispose();
      }
    }

    // After the children, so a parent's cleanup sees a fully torn-down subtree. Reverse
    // registration order, so a later callback unwinds before what it was built on.
    for cleanup in entity.cleanups.into_iter().rev() {
      cleanup();
    }

    if let Some(effect) = entity.effect {
      observer_clean_up(&effect);
    }

    let mut signal = entity.signal;

    if signal.is_none() {
      signal = SYNC_RUNTIME.remove_signal(self).map(Into::into);
    }

    Self::cleanup_signal(signal);
  }

  fn cleanup_signal(signal: Option<SignalState>) {
    if let Some(signal) = signal {
      // Snapshot first: dispose may re-enter the subscriber set.
      for effect_id in signal.subscribers.snapshot() {
        effect_id.dispose();
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cell::{Cell, RefCell},
    rc::Rc,
  };

  use crate::{
    RwSignal,
    SignalTrack,
    SignalUpdate,
    effect::Effect,
    runtime::{RUNTIME, Runtime},
    scope::Scope,
  };

  #[test]
  fn effect_disposed_when_dependency_signal_disposed() {
    let parent = Scope::new();
    let signal_scope = parent.create_child();
    let (signal, setter) = signal_scope.create_signal(0);

    let count = Rc::new(Cell::new(0));
    parent.enter(|| {
      let count = count.clone();
      Effect::new(move |_| {
        signal.track();
        count.set(count.get() + 1);
      });
    });

    assert_eq!(count.get(), 1);

    // Disposing the signal's scope should clean up the subscribing effect.
    signal_scope.dispose();

    // Mutations after disposal should not rerun the effect.
    setter.set(1);
    Runtime::drain_pending_work();
    assert_eq!(count.get(), 1);

    // The effect should be removed from the runtime.
    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .values()
          .all(|entity| entity.effect.is_none())
      )
    });
  }

  #[test]
  fn signals_created_by_effect_are_disposed_with_effect() {
    let parent = Scope::new();
    let dep_scope = parent.create_child();
    let (dep_signal, dep_setter) = dep_scope.create_signal(0);

    let created_signal = Rc::new(RefCell::new(None));
    let run_count = Rc::new(Cell::new(0));

    parent.enter(|| {
      let created_signal = created_signal.clone();
      let run_count = run_count.clone();
      Effect::new(move |_| {
        dep_signal.track();
        run_count.set(run_count.get() + 1);
        if created_signal.borrow().is_none() {
          created_signal.replace(Some(RwSignal::new(0)));
        }
      });
    });

    assert_eq!(run_count.get(), 1);
    let inner_signal = created_signal.borrow().expect("signal created");
    assert!(inner_signal.id().signal().is_some());

    // Dispose the dependency scope; the effect should be disposed and clean up its children.
    dep_scope.dispose();
    Runtime::drain_pending_work();

    // Mutating the dependency after disposal should do nothing.
    dep_setter.set(1);
    Runtime::drain_pending_work();
    assert_eq!(run_count.get(), 1);

    assert!(inner_signal.id().signal().is_none());
    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .values()
          .all(|entity| entity.effect.is_none())
      )
    });
  }

  #[test]
  fn disposing_scope_drops_signals_and_effects() {
    let scope = Scope::new();
    let (signal, setter) = scope.create_signal(0);
    let signal_id = signal.id();

    let run_count = Rc::new(Cell::new(0));
    scope.enter(|| {
      let run_count = run_count.clone();
      Effect::new(move |_| {
        signal.track();
        run_count.set(run_count.get() + 1);
      });
    });

    // Sanity: effect ran and runtime holds signal/effect.
    assert_eq!(run_count.get(), 1);
    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .get(&signal_id)
          .is_some_and(|entity| entity.signal.is_some())
      );

      assert_eq!(
        runtime
          .entities
          .borrow()
          .values()
          .filter(|entity| entity.effect.is_some())
          .count(),
        1
      );

      assert!(
        runtime
          .entities
          .borrow()
          .get(&scope.0)
          .and_then(|entity| entity.children.as_ref())
          .is_some()
      );
    });

    // Dispose the scope; both signal and effect should be cleaned up.
    scope.dispose();
    Runtime::drain_pending_work();

    setter.set(1);
    Runtime::drain_pending_work();
    assert_eq!(run_count.get(), 1);

    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .get(&signal_id)
          .and_then(|entity| entity.signal.as_ref())
          .is_none()
      );

      assert!(
        runtime
          .entities
          .borrow()
          .values()
          .all(|entity| entity.effect.is_none())
      );

      assert!(
        runtime
          .entities
          .borrow()
          .get(&scope.0)
          .and_then(|entity| entity.children.as_ref())
          .is_none()
      );
    });
  }

  /// A disposed child leaves no stale id behind in its parent's children set.
  #[test]
  fn dispose_unlinks_from_parent_children() {
    let parent = Scope::new();
    let child = parent.create_child();

    child.dispose();

    RUNTIME.with(|runtime| {
      let entities = runtime.entities.borrow();
      let children = entities
        .get(&parent.0)
        .and_then(|entity| entity.children.as_ref());

      assert!(children.is_none_or(|children| !children.contains(&child.0)));
    });
  }

  #[test]
  fn set_parent_reparents_scope() {
    // Create two independent scopes (simulating eager construction)
    let parent = Scope::new();
    let child = Scope::new();

    // Initially child has no parent
    assert!(child.parent().is_none());

    // Re-parent child under parent
    child.set_parent(parent);

    // Verify parent relationship
    assert_eq!(child.parent().map(|s| s.0), Some(parent.0));

    // Verify child is in parent's children set
    RUNTIME.with(|runtime| {
      let entities = runtime.entities.borrow();
      let parent_children = entities
        .get(&parent.0)
        .and_then(|entity| entity.children.as_ref())
        .expect("parent should have children");

      assert!(parent_children.contains(&child.0));
    });
  }

  #[test]
  fn set_parent_disposes_with_new_parent() {
    let parent = Scope::new();
    let child = Scope::new();

    // Create a signal in the child scope
    let signal = child.create_rw_signal(42);
    let signal_id = signal.id();

    // Re-parent child under parent
    child.set_parent(parent);

    // Signal should still exist
    assert!(signal_id.signal().is_some());

    // Dispose parent - should also dispose child and its signal
    parent.dispose();
    Runtime::drain_pending_work();

    // Signal should be cleaned up
    assert!(signal_id.signal().is_none());
  }

  #[test]
  fn set_parent_removes_from_old_parent() {
    let old_parent = Scope::new();
    let new_parent = Scope::new();
    let child = old_parent.create_child();

    // Verify initial parent
    assert_eq!(child.parent().map(|s| s.0), Some(old_parent.0));

    // Re-parent to new parent
    child.set_parent(new_parent);

    // Verify new parent
    assert_eq!(child.parent().map(|s| s.0), Some(new_parent.0));

    // Verify removed from old parent's children
    RUNTIME.with(|runtime| {
      let entities = runtime.entities.borrow();
      if let Some(old_children) = entities
        .get(&old_parent.0)
        .and_then(|entity| entity.children.as_ref())
      {
        assert!(!old_children.contains(&child.0));
      }
    });

    // Verify added to new parent's children
    RUNTIME.with(|runtime| {
      let entities = runtime.entities.borrow();
      let new_children = entities
        .get(&new_parent.0)
        .and_then(|entity| entity.children.as_ref())
        .expect("new parent should have children");

      assert!(new_children.contains(&child.0));
    });
  }
}
