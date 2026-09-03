//! Context values provided by an owner and visible to its whole subtree.

use std::{
  any::{Any, TypeId},
  rc::Rc,
};

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
  let value: Rc<dyn Any> = Rc::new(value);
  let replaced = RUNTIME.with(|runtime| {
    let owner = runtime.current_owner.get();

    runtime
      .graph()
      .owner_data_mut(owner)
      .and_then(|data| data.contexts.get_or_insert_default().insert(ty, value))
  });

  drop(replaced);
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
  let found: Option<Rc<dyn Any>> = RUNTIME.with(|runtime| {
    let mut owner = runtime.current_owner.get();
    let graph = runtime.graph();

    loop {
      let data = graph.owner_data(owner)?;

      if let Some(contexts) = &data.contexts
        && let Some(value) = contexts.get(&ty)
      {
        return Some(value.clone());
      }

      owner = data.parent?;
    }
  });

  found?.downcast_ref::<T>().cloned()
}
