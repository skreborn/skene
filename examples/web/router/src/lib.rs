//! A routed app: two pages behind real links, the heavy one loading lazily.
//!
//! The about page's body—and everything only it uses—splits into its own wasm module,
//! fetched on the first navigation there. `skene build web --static` renders every
//! route as full markup, so both pages read and navigate with JavaScript disabled; a
//! running client upgrades the links to instant in-app navigation and fetches the
//! split module in the background.
//!
//! Gated to `wasm32`: links and lazy modules are the web's, so the host has no platform
//! for them.

#![cfg(any(target_arch = "wasm32", skene_platform = "server"))]

use skene_simple_web::prelude::*;
use skene_web::{
  AnyView,
  AsyncView,
  Link,
  install_static_router,
  prelude::*,
  router::{Route, StaticRoutes},
};

/// Every page in the app, one URL each.
#[derive(Clone, Copy, PartialEq)]
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

#[main]
fn app() -> impl View {
  skene_web::view_transitions::enable();

  let router = install_static_router(Page::Home);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_background(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_spacing(32.)
          .set_padding(Inset::all(48.))
          .set_children((
            Row::new().set_spacing(16.).set_children((
              Link::new(Page::Home).add_child(navigation("Home")),
              Link::new(Page::About).add_child(navigation("About")),
            )),
            derived(move || match router.route() {
              Page::Home => home().build(),
              Page::About => AsyncView::new(about).build(),
            }),
          )),
      )
  })
}

fn home() -> impl View {
  page(
    "HOME",
    "This page ships in the main module; the about page arrives on demand.",
  )
}

/// The about page: its body—and everything only it uses—loads as its own wasm module,
/// fetched on the first navigation here.
#[lazy]
async fn about() -> AnyView {
  page(
    "ABOUT",
    "This page's code lives in a split module, fetched when you first navigated here.",
  )
  .into_any()
}

fn page(title: &'static str, body: &'static str) -> impl View {
  Container::new().set_max_width(420.).set_child(
    SimpleCard::new().set_spacing(12.).set_children((
      Text::new(title)
        .set_font_size(13.)
        .set_font_weight(500.)
        .set_color(SimplePalette::MUTED_FOREGROUND),
      Text::new(body)
        .set_font_size(16.)
        .set_color(SimplePalette::FOREGROUND),
    )),
  )
}

fn navigation(label: &'static str) -> impl View {
  Text::new(label)
    .set_font_size(14.)
    .set_font_weight(600.)
    .set_color(SimplePalette::FOREGROUND)
}
