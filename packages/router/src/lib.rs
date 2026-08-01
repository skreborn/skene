//! Routing as reactive state: a typed route in a signal, shared by every platform.
//!
//! An app defines its route type and implements [`Route`]; the [`Router`] holds the
//! current route in a signal, so any view that reads it re-renders on navigation. The
//! router knows nothing of URLs' owners: a platform binds it thinly—the browser to the
//! History API, a server render to the requested path, a native shell to nothing at all.

use std::{cell::RefCell, rc::Rc};

use skene_reactive::{Context, RwSignal, SignalGet, SignalUpdate};

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
type Sink = Rc<RefCell<Option<Box<dyn Fn(&str)>>>>;

/// The current route as reactive state.
///
/// Cheap to clone and thread through the reactive [`Context`]
/// ([`provide`](Self::provide) / [`get`](Self::get)), so any view can read or navigate.
#[must_use]
pub struct Router<R: Route> {
  current: RwSignal<R>,
  sink: Sink,
}

impl<R: Route> Clone for Router<R> {
  fn clone(&self) -> Self {
    Self {
      current: self.current,
      sink: Rc::clone(&self.sink),
    }
  }
}

impl<R: Route> Router<R> {
  /// Creates a router at `initial`.
  pub fn new(initial: R) -> Self {
    Self {
      current: RwSignal::new(initial),
      sink: Rc::new(RefCell::new(None)),
    }
  }

  /// The current route. Tracks, so a reading view re-runs on navigation.
  #[must_use]
  pub fn route(&self) -> R {
    self.current.get()
  }

  /// Navigates to `route`: reports the new path to the platform's sink (the browser
  /// pushes a history entry), then updates the state.
  ///
  /// The sink runs first so anything reacting to the state—a derived document title
  /// above all—applies to the new history entry, never retroactively to the departed
  /// one.
  pub fn navigate(&self, route: R) {
    let path = route.to_path();

    if let Some(sink) = self.sink.borrow().as_ref() {
      sink(&path);
    }

    self.current.set(route);
  }

  /// Applies an externally-changed path (back/forward navigation, the initial URL)
  /// without reporting it back to the sink. Returns whether the path parsed.
  #[must_use = "an unmatched path leaves the route unchanged"]
  pub fn apply_path(&self, path: &str) -> bool {
    match R::parse(path) {
      Some(route) => {
        self.current.set(route);

        true
      }
      None => false,
    }
  }

  /// Installs the platform's URL sink, called with the new path on each
  /// [`navigate`](Self::navigate).
  pub fn set_sink(&self, sink: impl Fn(&str) + 'static) {
    *self.sink.borrow_mut() = Some(Box::new(sink));
  }

  /// Stores this router in the current reactive scope, for [`get`](Self::get) anywhere
  /// beneath it.
  pub fn provide(&self) {
    Context::provide(self.clone());
  }

  /// The router provided by an ancestor scope, if any.
  #[must_use]
  pub fn get() -> Option<Self> {
    Context::get::<Self>()
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
      let router = router.clone();
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
