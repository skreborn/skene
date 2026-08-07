use std::cell::Cell;

use skene_reactive::Owner;

thread_local! {
  static ROOT_OWNER: Cell<Option<Owner>> = const { Cell::new(None) };
}

/// Returns the root scope for the current thread.
///
/// If there is no scope yet, a new one is created.
pub fn root_owner() -> Owner {
  ROOT_OWNER.with(|current| {
    if let Some(scope) = current.get() {
      scope
    }
    else {
      let scope = Owner::new();

      current.set(Some(scope));

      scope
    }
  })
}

/// Drops the current thread's root scope, so the next [`root_owner`] call makes a fresh one.
///
/// A platform calls this when it uninstalls.
pub fn reset_root_owner() {
  ROOT_OWNER.with(|current| current.set(None));
}
