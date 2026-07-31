//! Routing on the server leaf: the requested path resolves the route the render shows.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene::{
  DynamicView,
  Text,
  View,
  install_router,
  render_to_string_at,
  router::{Route, Router},
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

fn app() -> impl skene::IntoView {
  let router = install_router(Page::Home);

  DynamicView::new(move || match router.route() {
    Page::Home => Text::new("home page").into_any(),
    Page::About => Text::new("about page").into_any(),
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

    DynamicView::new(|| {
      let router = Router::<Page>::get().expect("a provided router");

      Text::new(router.route().to_path()).into_any()
    })
  });

  assert!(rendered.body().contains("/about"));
}
