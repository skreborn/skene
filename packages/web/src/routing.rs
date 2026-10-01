//! The router wired to the active platform's URL.

use skene_foundation::routing::{Route, Router, StaticRoutes};

use crate::ActivePlatform;

/// Creates the app's router from the platform's current URL, wires navigation both ways,
/// and provides it to the current scope.
///
/// In the browser the initial route parses from `location.pathname` (or `fallback` when
/// nothing matches), [`navigate`](Router::navigate) pushes a history entry, and
/// back/forward navigation flows back into the route; with
/// [`view_transitions::enable`](crate::view_transitions::enable) each change runs under
/// a view transition. On a server render the route parses from the requested path and
/// navigation stays in memory; off the web the router starts at `fallback` and owns
/// no URL.
///
/// Pair the returned router with a [`derived`](crate::derived) subtree or a
/// [`OneOf`](crate::OneOf) keyed on [`route`](Router::route); read it anywhere
/// beneath with [`Router::current`].
#[inline(always)]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  skene_foundation::routing::install_router::<ActivePlatform, R>(fallback)
}

/// [`install_router`], for a route type whose statically-renderable routes are
/// enumerable.
///
/// A server render records them for the build tool, so `skene build web --static`
/// renders every route without being told them.
#[inline(always)]
pub fn install_static_router<R: StaticRoutes>(fallback: R) -> Router<R> {
  skene_foundation::routing::install_static_router::<ActivePlatform, R>(fallback)
}
