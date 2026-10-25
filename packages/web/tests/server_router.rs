//! Routing on the server leaf: the requested path resolves the route the render shows.

#![cfg(all(skene_platform = "web", skene_server))]

use skene_web::{
  Link,
  LinkBase,
  Text,
  TextBase,
  View,
  derived,
  install_router,
  install_static_router,
  render_to_string_at,
  router::{Route, Router, StaticRoutes},
};

#[derive(Clone, PartialEq)]
enum Page {
  Home,
  About,
}

impl Route for Page {
  fn parse(path: &str) -> Option<Self> {
    match path {
      "/" => Some(Self::Home),
      "/about" => Some(Self::About),
      _ => None,
    }
  }

  fn to_path(&self) -> String {
    match self {
      Self::Home => "/".to_owned(),
      Self::About => "/about".to_owned(),
    }
  }
}

impl StaticRoutes for Page {
  fn all() -> Vec<Self> {
    vec![Self::Home, Self::About]
  }
}

fn app() -> impl skene_web::View {
  let router = install_router(Page::Home);

  derived(move || match router.route() {
    Page::Home => Text::new("home page").build(),
    Page::About => Text::new("about page").build(),
  })
}

/// The requested path picks the rendered route; an unknown path falls back.
#[test]
fn requested_path_resolves_the_route() {
  assert!(render_to_string_at("/", app).body().contains("home page"));
  assert!(
    render_to_string_at("/about", app)
      .body()
      .contains("about page")
  );

  assert!(
    render_to_string_at("/nope", app)
      .body()
      .contains("home page")
  );
}

/// The provided router is reachable beneath the install point.
#[test]
fn router_flows_through_context() {
  let rendered = render_to_string_at("/about", || {
    let _ = install_router(Page::Home);

    derived(|| {
      let router = Router::<Page>::current().expect("a provided router");

      Text::new(router.route().to_path()).build()
    })
  });

  assert!(rendered.body().contains("/about"));
}

/// A link renders as a real link to its route's path around the view it holds, under a
/// router and without one alike, so a page without script follows it.
#[test]
fn a_link_renders_its_destination() {
  let link = || Link::new(Page::About).set_child(Text::new("about us"));
  let routed = render_to_string_at("/", move || {
    let _ = install_router(Page::Home);

    link()
  });
  let unrouted = render_to_string_at("/", link);

  for page in [routed, unrouted] {
    let body = page.body();
    let anchor = &body[body.find("<a ").expect("an anchor")..];
    let anchor = &anchor[..anchor.find("</a>").expect("the anchor's end")];

    assert!(anchor.contains(" href=\"/about\""), "{body}");
    assert!(anchor.contains(">about us</span>"), "{body}");
  }
}

/// An enumerable router records its routes for the build tool; a plain one records
/// nothing.
#[test]
fn static_router_records_its_routes() {
  let _ = render_to_string_at("/", || {
    let router = install_static_router(Page::Home);

    derived(move || match router.route() {
      Page::Home => Text::new("home page").build(),
      Page::About => Text::new("about page").build(),
    })
  });

  assert_eq!(
    skene_web::platform::web::recorded_static_routes(),
    Some(vec!["/".to_owned(), "/about".to_owned()])
  );

  let _ = render_to_string_at("/", app);

  assert_eq!(skene_web::platform::web::recorded_static_routes(), None);
}
