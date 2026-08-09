//! A routed app: two pages behind real links, the heavy one loading lazily.
//!
//! The about page's body—and everything only it uses—splits into its own wasm module,
//! fetched on the first navigation there. `skene build web --static` renders every
//! route as full markup, so both pages read and navigate with JavaScript disabled; a
//! running client upgrades the links to instant in-app navigation and fetches the
//! split module in the background.

#![cfg(target_arch = "wasm32")]

use skene_gui::{
  AnyView,
  DerivedView,
  Lazy,
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
fn app() -> impl IntoView {
  let router = install_static_router(Page::Home);

  Column::new()
    .set_spacing(32.)
    .set_padding(Inset::all(48.))
    .set_color(Color::srgb(0.1, 0.1, 0.1))
    .set_children((
      Row::new().set_spacing(16.).set_children((
        Link::new(Page::Home).set_child(|| navigation("Home")),
        Link::new(Page::About).set_child(|| navigation("About")),
      )),
      DerivedView::new(move || match router.route() {
        Page::Home => home().into_view().into_any(),
        Page::About => Lazy::new(about).into_any(),
      }),
    ))
}

fn home() -> impl IntoView {
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
  .into_view()
  .into_any()
}

fn page(title: &'static str, body: &'static str) -> impl IntoView {
  Column::new()
    .set_spacing(12.)
    .set_max_width(420.)
    .set_padding(Inset::symmetric(32., 28.))
    .set_color(Color::srgb(0.15, 0.15, 0.15))
    .set_border(Border::new(1., Color::srgb(0.2, 0.2, 0.2)))
    .set_border_radius(8.)
    .set_children((
      Text::new(title)
        .set_font_size(13.)
        .set_font_weight(500.)
        .set_color(Color::srgb(0.55, 0.57, 0.63)),
      Text::new(body)
        .set_font_size(16.)
        .set_color(Color::srgb(0.78, 0.87, 0.8)),
    ))
}

fn navigation(label: &'static str) -> impl IntoView {
  Text::new(label)
    .set_font_size(14.)
    .set_font_weight(600.)
    .set_color(Color::srgb(0.78, 0.87, 0.8))
}
