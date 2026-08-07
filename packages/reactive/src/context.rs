//! Context values provided by an owner and visible to its whole subtree.

use std::any::{Any, TypeId};

use crate::runtime::RUNTIME;

/// Stores `value` in the current owner, shadowing any ancestor's value of the type.
///
/// The value is visible to the current owner and all of its descendants through
/// [`use_context`], and is dropped when the owner is disposed.
pub fn provide_context<T>(value: T)
where
  T: Clone + 'static,
{
  let ty = TypeId::of::<T>();

  RUNTIME.with(|runtime| {
    let owner = runtime.current_owner.get();

    if let Some(entity) = runtime.arena.borrow_mut().get_mut(owner) {
      entity
        .contexts
        .get_or_insert_default()
        .insert(ty, Box::new(value) as Box<dyn Any>);
    }
  });
}

/// The nearest `T` provided by the current owner or one of its ancestors, if any.
///
/// Lookup walks up the owner tree, so a child's [`provide_context`] shadows its
/// ancestors' for the child's subtree.
pub fn use_context<T>() -> Option<T>
where
  T: Clone + 'static,
{
  let ty = TypeId::of::<T>();

  RUNTIME.with(|runtime| {
    let mut owner = runtime.current_owner.get();
    let arena = runtime.arena.borrow();

    loop {
      let entity = arena.get(owner)?;

      if let Some(contexts) = &entity.contexts
        && let Some(value) = contexts.get(&ty)
      {
        return value.downcast_ref::<T>().cloned();
      }

      match entity.parent {
        Some(parent) => owner = parent,
        None => return None,
      }
    }
  })
}
