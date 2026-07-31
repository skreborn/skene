//! The router wired to the active driver's URL.

use skene_router::{Route, Router};

use crate::driver;

/// Creates the app's router from the driver's current URL, wires navigation both ways,
/// and provides it to the current scope.
///
/// In the browser the initial route parses from `location.pathname` (or `fallback` when
/// nothing matches), [`navigate`](Router::navigate) pushes a history entry, and
/// back/forward navigation flows back into the route. On a server render the route
/// parses from the requested path and navigation stays in memory.
///
/// Pair the returned router with a [`DynamicView`](crate::DynamicView) that matches on
/// [`route`](Router::route); read it anywhere beneath with [`Router::get`].
#[cfg(target_os = "unknown")]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  let initial = R::parse(&driver::current_path()).unwrap_or(fallback);
  let router = Router::new(initial);

  router.set_sink(driver::push_path);

  {
    let router = router.clone();

    driver::on_pop(move |path| {
      let _ = router.apply_path(&path);
    });
  }

  router.provide();

  router
}

/// Creates the app's router from the requested path, and provides it to the current
/// scope. Navigation stays in memory — a server render is one route.
#[cfg(target_os = "wasi")]
pub fn install_router<R: Route>(fallback: R) -> Router<R> {
  let initial = R::parse(&driver::current_path()).unwrap_or(fallback);
  let router = Router::new(initial);

  router.provide();

  router
}
