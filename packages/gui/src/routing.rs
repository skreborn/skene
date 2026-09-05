//! The router wired to the active driver's URL.

use skene_router::{Route, Router, StaticRoutes};

/// Creates the app's router from the driver's current URL, wires navigation both ways,
/// and provides it to the current scope.
///
/// In the browser the initial route parses from `location.pathname` (or `fallback` when
/// nothing matches), [`navigate`](Router::navigate) pushes a history entry, and
/// back/forward navigation flows back into the route. On a server render the route
/// parses from the requested path and navigation stays in memory.
///
/// Pair the returned router with a [`derived`](crate::derived) subtree or a
/// [`Switch`](crate::Switch) keyed on [`route`](Router::route); read it anywhere
/// beneath with [`Router::get`].
#[cfg(target_os = "unknown")]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  let router = Router::new(fallback);
  let _ = router.apply_path(&skene_web::current_path());

  router.set_sink(skene_web::push_path);

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
#[cfg(target_os = "wasi")]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  let router = Router::new(fallback);
  let _ = router.apply_path(&skene_web::current_path());

  router.provide();

  router
}

/// [`install_router`], for a route type whose statically-renderable routes are
/// enumerable: the enumeration also reaches the build tool, so `skene build web
/// --static` renders every route without being told them.
#[cfg(target_os = "unknown")]
#[inline(always)]
pub fn install_static_router<R: StaticRoutes>(fallback: R) -> Router<R> {
  install_router(fallback)
}

/// [`install_router`], recording the enumerable routes for the build tool: `skene
/// build web --static` renders every route without being told them.
#[cfg(target_os = "wasi")]
pub fn install_static_router<R: StaticRoutes>(fallback: R) -> Router<R> {
  skene_web::record_static_routes(R::all().iter().map(Route::to_path).collect());

  install_router(fallback)
}
