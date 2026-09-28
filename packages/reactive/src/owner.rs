use std::{fmt, marker::PhantomData};

use crate::{
  id::Id,
  path::Key,
  runtime::{RUNTIME, Runtime, with_graph},
};

/// Owns a subtree of the reactive graph: every signal, effect, memo, and child owner
/// created under it, disposed together.
///
/// Everything reactive is created under the current owner—implicitly the one the
/// running effect or enclosing [`enter`](Self::enter) established. Disposing an owner
/// disposes its whole subtree and runs its cleanups.
#[derive(Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct Owner(pub(crate) Id, pub(crate) PhantomData<*const ()>);

impl Default for Owner {
  fn default() -> Self {
    Self::new()
  }
}

impl fmt::Debug for Owner {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut r#struct = formatter.debug_struct("Owner");

    r#struct.field("id", &self.0);

    r#struct.finish()
  }
}

impl Owner {
  /// Creates a detached root owner, disposed only by [`dispose`](Self::dispose).
  #[inline(always)]
  pub fn new() -> Self {
    Self::from_id(with_graph(|graph| {
      let owner = graph.allocate_owner();

      graph.assign_root_ordinal(owner);

      owner
    }))
  }

  pub(crate) const fn from_id(id: Id) -> Self {
    Self(id, PhantomData)
  }

  /// The owner's id.
  #[inline(always)]
  pub fn id(&self) -> Id {
    self.0
  }

  /// The current owner: the one anything reactive created right now would belong to.
  #[inline(always)]
  pub fn current() -> Self {
    RUNTIME.with(|runtime| Self::from_id(runtime.current_owner.get()))
  }

  /// Creates a child of this owner, disposed with it.
  pub fn child(&self) -> Self {
    Self::from_id(with_graph(|graph| {
      let child = graph.allocate_owner();

      graph.adopt(child, self.0);

      child
    }))
  }

  /// Creates a child of this owner identified by `key` rather than by its position
  /// among the children, disposed with this owner.
  ///
  /// State created under the child keeps its snapshot identity wherever the child sits
  /// and however its siblings change, and a keyed child takes no position, so keying
  /// one shifts none of the positional siblings.
  pub fn keyed_child(&self, key: Key) -> Self {
    Self::from_id(with_graph(|graph| {
      let child = graph.allocate_owner();

      graph.key_next_child(self.0, key);
      graph.adopt(child, self.0);

      child
    }))
  }

  /// Creates an owner taking this one's place: the same parent and the same position
  /// in it, so state created under the replacement keeps this owner's snapshot
  /// identity. For a subtree rebuilt in place; this owner is disposed by its caller.
  pub fn replacement(&self) -> Self {
    Self::from_id(with_graph(|graph| graph.allocate_replacement(self.0)))
  }

  /// Reparents this owner under `parent`, unlinking it from its previous parent.
  pub fn set_parent(&self, parent: Owner) {
    with_graph(|graph| graph.adopt(self.0, parent.0));
  }

  /// The parent owner, if any.
  #[inline(always)]
  pub fn parent(&self) -> Option<Owner> {
    with_graph(|graph| graph.parent(self.0)).map(Self::from_id)
  }

  /// Runs `operation` with this owner as the current one, restoring the previous afterwards.
  ///
  /// # Panics
  ///
  /// In a debug build, panics when this owner was disposed.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn enter<T>(&self, operation: impl FnOnce() -> T) -> T {
    Runtime::assert_ui_thread();

    #[cfg(debug_assertions)]
    assert!(
      with_graph(|graph| graph.owner_data(self.0).is_some()),
      "an owner entered at {} after its disposal",
      std::panic::Location::caller()
    );

    Runtime::with_owner(self.0, operation)
  }

  /// Runs `cleanup` when this owner is disposed, after its children, in reverse registration
  /// order; on an effect or a memo, also before its next run; on an owner already
  /// disposed, at once.
  ///
  /// The hook for anything the runtime cannot clean up itself—cancelling a spawned
  /// task, clearing a timer, closing a subscription—so it ends with the owner that
  /// started it rather than outliving the view.
  ///
  /// # Panics
  ///
  /// Panics if called off the UI thread.
  pub fn on_cleanup(&self, cleanup: impl FnOnce() + 'static) {
    Runtime::assert_ui_thread();

    let unregistered = with_graph(|graph| match graph.owner_data_mut(self.0) {
      Some(data) => {
        data.cleanups.push(Box::new(cleanup));

        None
      }
      None => Some(cleanup),
    });

    if let Some(cleanup) = unregistered {
      cleanup();
    }
  }

  /// Disposes this owner: its signals, effects, contexts, and child owners.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn dispose(&self) {
    #[cfg(debug_assertions)]
    let _disposing = crate::obituary::Disposing::enter(std::panic::Location::caller());

    self.0.dispose();
  }
}

/// Runs `cleanup` when the current owner is disposed; see [`Owner::on_cleanup`].
pub fn on_cleanup(cleanup: impl FnOnce() + 'static) {
  Owner::current().on_cleanup(cleanup);
}
