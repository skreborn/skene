use std::{fmt, marker::PhantomData};

use crate::{
  id::Id,
  runtime::{RUNTIME, Runtime},
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
    Self::from_id(Runtime::allocate())
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
    let child = Runtime::allocate();

    RUNTIME.with(|runtime| {
      let mut arena = runtime.arena.borrow_mut();

      if let Some(entity) = arena.get_mut(self.0) {
        entity.children.get_or_insert_default().insert(child);
      }

      if let Some(entity) = arena.get_mut(child) {
        entity.parent = Some(self.0);
      }
    });

    Self::from_id(child)
  }

  /// Reparents this owner under `parent`, unlinking it from its previous parent.
  pub fn set_parent(&self, parent: Owner) {
    RUNTIME.with(|runtime| {
      let mut arena = runtime.arena.borrow_mut();

      if let Some(previous) = arena
        .get_mut(self.0)
        .and_then(|entity| entity.parent.take())
        && let Some(children) = arena
          .get_mut(previous)
          .and_then(|entity| entity.children.as_mut())
      {
        children.remove(&self.0);
      }

      if let Some(entity) = arena.get_mut(parent.0) {
        entity.children.get_or_insert_default().insert(self.0);
      }

      if let Some(entity) = arena.get_mut(self.0) {
        entity.parent = Some(parent.0);
      }
    });
  }

  /// The parent owner, if any.
  #[inline(always)]
  pub fn parent(&self) -> Option<Owner> {
    RUNTIME.with(|runtime| {
      runtime
        .arena
        .borrow()
        .get(self.0)
        .and_then(|entity| entity.parent)
        .map(Self::from_id)
    })
  }

  /// Runs `f` with this owner as the current one, restoring the previous afterwards.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn enter<T>(&self, f: impl FnOnce() -> T) -> T {
    Runtime::assert_ui_thread();

    let previous = RUNTIME.with(|runtime| runtime.current_owner.replace(self.0));
    let result = f();

    RUNTIME.with(|runtime| runtime.current_owner.set(previous));

    result
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

    RUNTIME.with(|runtime| {
      if let Some(entity) = runtime.arena.borrow_mut().get_mut(self.0) {
        entity.cleanups.push(Box::new(f));
      }
    });
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
