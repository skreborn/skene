//! The router in the terminal: pure state.

use skene_foundation::routing::{Route, Router};

use crate::ActivePlatform;

/// Creates the app's router at `fallback`, and provides it to the current scope.
/// Navigation stays in memory—the terminal owns no URL.
#[inline(always)]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  skene_foundation::routing::install_router::<ActivePlatform, R>(fallback)
}
