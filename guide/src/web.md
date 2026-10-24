# The web

A web app is one crate for two builds, the web's two leaves. The browser build hydrates
the page in place; the server build renders a requested path to markup. `skene` builds
each from the same library.

## Rendering on the server

A page the server renders is complete markup: its text, its styles, its links. It reads,
navigates and themes itself with JavaScript off, and the browser bundle hydrates it in
place, adopting the server's elements rather than replacing them.
`skene build web --static` renders every route into static pages, and
`skene run web --ssr` renders each request as a server would.

Data a page loads crosses the boundary in a `Resource`: the server settles it and writes
its value into the page, and the client adopts that value rather than loading it again.

```rust
use skene_web::{Resource, prelude::*};

fn greeting() -> impl View {
  let name: Resource<String> = Resource::new(|| async { "reader".to_owned() });

  derived(move || match name.get() {
    None => Text::new("Loading…").build(),
    Some(name) => Text::new(format!("Hello, {name}")).build(),
  })
}
```

`Resource::client` loads on the client alone, for data the server cannot produce.
Content behind slow data can stream: a `Deferred` boundary flushes the page around it
first, and its content when it settles, still in document order and without script.

## Two forms of one view

Whatever can work without script must. A view with a form the page can carry and a
richer one the bundle brings declares both, and `Enhance` builds the fallback until its
signal turns and the child after:

```rust
use skene_web::prelude::*;

fn clock() -> impl View {
  Enhance::new(skene_web::media::hydrated())
    .set_fallback_builder(|| Text::new("The time, once the page is live."))
    .set_child_builder(|| Text::new("A live clock"))
}
```

The server renders the fallback, the hydration adopts it, and the child replaces it once
the page is live.

## The document

The `head` module registers what the document's head carries, for as long as the view
registering it lives, so a route's description leaves with the route:

```rust
use skene_web::{head, prelude::*};

fn about() -> impl View {
  head::add_meta("description", "Who we are");

  Text::new("About")
}
```

## Routes

A route is a type that parses a path and prints one. The router reads the route in
force, and a `Link` navigates to one: a real link a page without script follows, and an
in-app navigation once the bundle runs. A route's view can load lazily, as its own
module, through `#[lazy]` and `AsyncView`. A route type that lists every route
(`StaticRoutes`) takes the static router, so `skene build web --static` renders each
without being told them:

```rust
use skene_web::{
  Link,
  install_static_router,
  prelude::*,
  router::{Route, StaticRoutes},
};

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

fn app() -> impl View {
  let router = install_static_router(Page::Home);

  Column::new().set_children((
    Link::new(Page::About).set_child(Text::new("About")),
    derived(move || match router.route() {
      Page::Home => Text::new("Home").build(),
      Page::About => Text::new("About us").build(),
    }),
  ))
}
```

## Pixels

`Painted` draws with vello into a canvas: on the GPU where the page has WebGPU, on the
CPU otherwise. The server renders it empty, and a painted view declares the fallback
the page carries until pixels are available.
