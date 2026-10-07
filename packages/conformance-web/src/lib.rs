//! # The conformance suite's pages, as one web app
//!
//! Every page of the suite ([`skene_conformance::pages`]) served from one app: the path
//! a browser opens names the page, `/button` the page named `button`, and the app builds
//! it as its whole view. Once built, the page is reachable from the harness
//! (`src/bin/harness`) as the global `skeneConformance`, whose `probe(name)` reads what
//! the page publishes as its probe `name` ([`probe`]).

use std::cell::RefCell;

use skene_conformance::Probes;
use skene_web::{ActivePlatform, AnyView, Platform, main};

thread_local! {
  /// What the page the app built publishes, once it is built.
  static PROBES: RefCell<Option<Probes>> = const { RefCell::new(None) };
}

/// Builds the page the path names, then publishes the global the harness reads it by.
///
/// # Panics
///
/// Panics where the path names no page of the suite.
#[main]
fn app() -> AnyView {
  let path = ActivePlatform::current_path().unwrap_or_default();
  let name = path.trim_start_matches('/');
  let page = skene_conformance::page::<ActivePlatform>(name)
    .unwrap_or_else(|| panic!("no page of the suite is named {name:?}"));
  let mut probes = Probes::default();
  let view = (page.build)(&mut probes);

  PROBES.set(Some(probes));

  #[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
