//! The router in the terminal: pure state.

use skene_foundation::routing::{Route, Router};

use crate::Active;

/// Creates the app's router at `fallback`, and provides it to the current scope.
/// Navigation stays in memory.
#[inline(always)]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  skene_foundation::routing::install_router::<Active, R>(fallback)
}
