//! # The conformance suite's pages, as one web app
//!
//! Every page of the suite ([`skene_conformance::pages`]) served from one app: the path
//! a browser opens names the page, `/button` the page named `button`, and the app builds
//! it as its whole view, in the pinned font the app serves (`public/fonts`). Once built,
//! the page is reachable from the harness (`src/bin/harness`) as the global
//! `skeneConformance`, whose `probe(name)` reads what the page publishes as its probe
//! `name` ([`probe`]).

#![cfg(skene_platform = "web")]

use std::cell::RefCell;

use skene_conformance::Probes;
use skene_web::{Active, AnyView, head, main, platform::web::current_path};

thread_local! {
  /// What the page the app built publishes, once it is built.
  static PROBES: RefCell<Option<Probes>> = const { RefCell::new(None) };
}

/// Builds the page the path names in the pinned font, then publishes the global the
/// harness reads it by.
///
/// # Panics
///
/// Panics where the path names no page of the suite.
#[main]
fn app() -> AnyView {
  let path = current_path();
  let name = path.trim_start_matches('/');
  let page = skene_conformance::page::<Active>(name)
    .unwrap_or_else(|| panic!("no page of the suite is named {name:?}"));
  let mut probes = Probes::default();

  head::add_stylesheet("/fonts/pinned.css");

  let view = (page.build)(&mut probes);

  PROBES.set(Some(probes));

  #[cfg(not(skene_server))]
  publish();

  view
}

/// What the built page publishes as its probe `name`; `None` where it publishes no such
/// probe or is not built yet.
#[must_use]
pub fn probe(name: &str) -> Option<String> {
  PROBES.with_borrow(|probes| probes.as_ref()?.read(name))
}

/// Publishes the global `skeneConformance`, its `probe` calling [`probe`].
#[cfg(not(skene_server))]
fn publish() {
  use wasm_bindgen::{JsValue, closure::Closure};

  let read = Closure::<dyn Fn(String) -> Option<String>>::new(|name: String| probe(&name));
  let global = js_sys::Object::new();

  js_sys::Reflect::set(&global, &JsValue::from_str("probe"), read.as_ref())
    .expect("the probe is set");
  js_sys::Reflect::set(
    &js_sys::global(),
    &JsValue::from_str("skeneConformance"),
    &global,
  )
  .expect("the global is set");

  read.forget();
}
