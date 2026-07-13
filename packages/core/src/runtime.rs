use std::cell::Cell;

use skene_reactive::Scope;

thread_local! {
  static ROOT_SCOPE: Cell<Option<Scope>> = const { Cell::new(None) };
}

/// Returns the root scope for the current thread.
///
/// If there is no scope yet, a new one is created.
pub fn root_scope() -> Scope {
  ROOT_SCOPE.with(|current| {
    if let Some(scope) = current.get() {
      scope
    }
    else {
      let scope = Scope::new();

      current.set(Some(scope));

      scope
    }
  })
}

/// Drops the current thread's root scope, so the next [`root_scope`] call makes a fresh one.
///
/// A platform calls this when it uninstalls.
pub fn reset_root_scope() {
  ROOT_SCOPE.with(|current| current.set(None));
}
