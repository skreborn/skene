use std::{fmt, marker::PhantomData};

use crate::{
  id::Id,
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

  /// Runs `f` with this owner as the current one, restoring the previous afterwards.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn enter<T>(&self, f: impl FnOnce() -> T) -> T {
    Runtime::assert_ui_thread();
    Runtime::with_owner(self.0, f)
  }

  /// Runs `f` when this owner is disposed.
  ///
  /// The hook for anything the runtime cannot clean up itself—cancelling a spawned
  /// task, clearing a timer, closing a subscription—so it ends with the owner that
  /// started it rather than outliving the view. Callbacks run after the owner's
  /// children are disposed, in reverse registration order.
  ///
  /// # Panics
  ///
  /// Panics if called off the UI thread.
  pub fn on_cleanup(&self, f: impl FnOnce() + 'static) {
    Runtime::assert_ui_thread();

    let unregistered = with_graph(|graph| match graph.owner_data_mut(self.0) {
      Some(data) => {
        data.cleanups.push(Box::new(f));

        None
      }
      None => Some(f),
    });

    drop(unregistered);
  }

  /// Disposes this owner: its signals, effects, contexts, and child owners.
  pub fn dispose(&self) {
    self.0.dispose();
  }
}

/// Runs `f` when the current owner is disposed; see [`Owner::on_cleanup`].
pub fn on_cleanup(f: impl FnOnce() + 'static) {
  Owner::current().on_cleanup(f);
}
