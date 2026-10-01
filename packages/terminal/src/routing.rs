//! The router in the terminal: pure state.

use skene_foundation::routing::{Route, Router};

/// Creates the app's router at `fallback`, and provides it to the current scope.
/// Navigation stays in memory—the terminal owns no URL.
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  let router = Router::new(fallback);

  router.provide();

  router
}
