//! The router wired to the active platform's URL.

use skene_common::routing::{Route, Router, StaticRoutes};

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
/// [`Switch`](crate::Switch) keyed on [`route`](Router::route); read it anywhere
/// beneath with [`Router::get`].
#[cfg(skene_leaf = "browser")]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  let router = Router::new(fallback);
  let _ = router.apply_path(&skene_web::current_path());

  router.set_sink(skene_web::push_path);
  router.set_commit(skene_web::commit_navigation);

  {
    let router = router.clone();

    skene_web::on_pop(move |path| {
      let _ = router.apply_path(&path);
    });
  }

  router.provide();

  router
}

/// Creates the app's router from the requested path, and provides it to the current
/// scope. Navigation stays in memory—a server render is one route.
#[cfg(skene_leaf = "server")]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  let router = Router::new(fallback);
  let _ = router.apply_path(&skene_web::current_path());

  router.provide();

  router
}

/// Creates the app's router at `fallback`, and provides it to the current scope.
/// Navigation stays in memory—the host owns no URL.
#[cfg(skene_leaf = "host")]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  let router = Router::new(fallback);

  router.provide();

  router
}

/// [`install_router`], for a route type whose statically-renderable routes are
/// enumerable.
///
/// The enumeration also reaches the build tool, so `skene build web --static` renders
/// every route without being told them.
#[cfg(not(skene_leaf = "server"))]
#[inline(always)]
pub fn install_static_router<R: StaticRoutes>(fallback: R) -> Router<R> {
  install_router(fallback)
}

/// [`install_router`], recording the enumerable routes for the build tool: `skene
/// build web --static` renders every route without being told them.
#[cfg(skene_leaf = "server")]
pub fn install_static_router<R: StaticRoutes>(fallback: R) -> Router<R> {
  skene_web::record_static_routes(R::all().iter().map(Route::to_path).collect());

  install_router(fallback)
}
