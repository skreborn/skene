//! Routing as reactive state: a typed route in a signal, shared by every platform.
//!
//! An app defines its route type and implements [`Route`]; the [`Router`] holds the
//! current route in a signal, so any view that reads it re-renders on navigation. The
//! router knows nothing of URLs' owners: a platform binds it thinly—the browser to the
//! History API, a server render to the requested path, a native shell to nothing at all.

use std::rc::Rc;

use skene_reactive::{Get, Set, Signal, provide_context, use_context};

/// A type that names every place in the app, convertible to and from a URL path.
pub trait Route: Clone + PartialEq + 'static {
  /// Parses a path (`"/users/42"`) into a route, or `None` when nothing matches.
  fn parse(path: &str) -> Option<Self>;

  /// Renders the route back to its path.
  fn to_path(&self) -> String;
}

/// A route type whose instances can be enumerated—what static site generation renders.
pub trait StaticRoutes: Route {
  /// Every statically-renderable route.
  fn all() -> Vec<Self>;
}

/// The URL sink a platform installs: called with the new path on programmatic
/// navigation.
type Sink = Option<Rc<dyn Fn(&str)>>;

/// The commit a platform installs: runs a navigation's state change, wrapping it in a
/// view transition where the platform has one.
type Commit = Option<Rc<dyn Fn(Box<dyn FnOnce()>)>>;

/// The current route as reactive state.
///
/// `Copy`, and threaded through the reactive context ([`provide`](Self::provide) /
/// [`get`](Self::get)), so any view can read or navigate. It lives as long as the owner
/// it was made under.
#[must_use]
pub struct Router<R: Route> {
  current: Signal<R>,
  sink: Signal<Sink>,
  commit: Signal<Commit>,
}

impl<R: Route> std::fmt::Debug for Router<R> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Router").finish_non_exhaustive()
  }
}

impl<R: Route> Copy for Router<R> {}

#[expect(
  clippy::expl_impl_clone_on_copy,
  reason = "a derive would bound the route on Copy"
)]
impl<R: Route> Clone for Router<R> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<R: Route> Router<R> {
  /// Creates a router at `initial`.
  pub fn new(initial: R) -> Self {
    Self {
      current: Signal::opaque(initial),
      sink: Signal::opaque(None),
      commit: Signal::opaque(None),
    }
  }

  /// The current route. Tracks, so a reading view re-runs on navigation.
  #[must_use]
  pub fn route(self) -> R {
    self.current.get()
  }

  /// Navigates to `route`: reports the new path to the platform's sink (the browser
  /// pushes a history entry), then updates the state through the platform's commit.
  ///
  /// The sink runs first so anything reacting to the state—a derived document title
  /// above all—applies to the new history entry, never retroactively to the departed
  /// one.
  pub fn navigate(self, route: R) {
    let path = route.to_path();

    if let Some(sink) = self.sink.get_untracked() {
      sink(&path);
    }

    self.commit(route);
  }

  /// Writes `route` into the state, through the platform's commit when one is
  /// installed.
  fn commit(self, route: R) {
    let current = self.current;

    match self.commit.get_untracked() {
      Some(commit) => commit(Box::new(move || current.set(route))),
      None => current.set(route),
    }
  }

  /// Applies an externally-changed path (back/forward navigation, the initial URL)
  /// without reporting it back to the sink. Returns whether the path parsed.
  ///
  /// Trailing slashes are trimmed before parsing (the root stays `/`), so the
  /// directory URLs a statically-hosted route serves under match their route.
  #[must_use = "an unmatched path leaves the route unchanged"]
  pub fn apply_path(self, path: &str) -> bool {
    let trimmed = path.trim_end_matches('/');
    let path = if trimmed.is_empty() { "/" } else { trimmed };

    match R::parse(path) {
      Some(route) => {
        self.commit(route);

        true
      }
      None => false,
    }
  }

  /// Installs the platform's URL sink, called with the new path on each
  /// [`navigate`](Self::navigate).
  pub fn set_sink(self, sink: impl Fn(&str) + 'static) {
    self.sink.set(Some(Rc::new(sink)));
  }

  /// Installs the platform's commit: every state change of a navigation runs through
  /// it, so a platform with view transitions wraps the change in one.
  pub fn set_commit(self, commit: impl Fn(Box<dyn FnOnce()>) + 'static) {
    self.commit.set(Some(Rc::new(commit)));
  }

  /// Stores this router in the current reactive scope, for [`get`](Self::get) anywhere
  /// beneath it.
  pub fn provide(self) {
    provide_context(self);
  }

  /// The router provided by an ancestor scope, if any.
  #[must_use]
  pub fn get() -> Option<Self> {
    use_context::<Self>()
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use skene_reactive::Effect;

  use super::*;

  #[derive(Clone, PartialEq, Debug)]
  enum Page {
    Home,
    User(u32),
  }

  impl Route for Page {
    fn parse(path: &str) -> Option<Self> {
      if path == "/" {
        return Some(Self::Home);
      }

      path
        .strip_prefix("/users/")
        .and_then(|id| id.parse().ok())
        .map(Self::User)
    }

    fn to_path(&self) -> String {
      match self {
        Self::Home => "/".to_owned(),
        Self::User(id) => format!("/users/{id}"),
      }
    }
  }

  /// Navigation updates the route and reports the path to the sink.
  #[test]
  fn navigate_updates_state_and_sink() {
    let router = Router::new(Page::Home);
    let pushed = Rc::new(RefCell::new(Vec::new()));

    {
      let pushed = Rc::clone(&pushed);

      router.set_sink(move |path| pushed.borrow_mut().push(path.to_owned()));
    }

    router.navigate(Page::User(42));

    assert_eq!(router.route(), Page::User(42));
    assert_eq!(*pushed.borrow(), ["/users/42"]);
  }

  /// An externally-applied path updates the state without echoing to the sink.
  #[test]
  fn apply_path_does_not_echo() {
    let router = Router::new(Page::Home);
    let pushed = Rc::new(RefCell::new(0));

    {
      let pushed = Rc::clone(&pushed);

      router.set_sink(move |_| *pushed.borrow_mut() += 1);
    }

    assert!(router.apply_path("/users/7"));
    assert_eq!(router.route(), Page::User(7));
    assert_eq!(*pushed.borrow(), 0);
    assert!(!router.apply_path("/nope"));
    assert_eq!(router.route(), Page::User(7));
  }

  /// A view reading the route re-runs on navigation.
  #[test]
  fn route_reads_track() {
    let router = Router::new(Page::Home);
    let runs = Rc::new(RefCell::new(0));

    {
      let runs = Rc::clone(&runs);

      Effect::new(move |_| {
        let _ = router.route();

        *runs.borrow_mut() += 1;
      });
    }

    assert_eq!(*runs.borrow(), 1);

    router.navigate(Page::User(1));

    assert_eq!(*runs.borrow(), 2);
  }
}
