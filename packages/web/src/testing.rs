//! What a browser test shares: the platform installed on a fresh connected root, reads
//! of the page it renders, events dispatched to it, and waits on the browser's clock and
//! its microtask queue.

use std::time::Duration;

use web_sys::{Document, Element};

/// The window's document.
///
/// # Panics
///
/// Panics outside a browser window.
#[must_use]
pub fn document() -> Document {
  web_sys::window()
    .expect("a window")
    .document()
    .expect("a document")
}

/// The browser leaf installed on a root of its own: uninstalled, the root removed, when
/// dropped, so a failing test leaves the page clean for the next.
#[derive(Debug)]
#[must_use]
pub struct Installed(Element);

impl Installed {
  /// Guards the browser leaf a test installed or hydrated on `root` itself.
  pub const fn on(root: Element) -> Self {
    Self(root)
  }
}

impl Drop for Installed {
  fn drop(&mut self) {
    crate::uninstall();
    self.0.remove();
  }
}

/// A fresh `<div>` connected to the body, for a test to install or hydrate the browser
/// leaf on; computed styles resolve under it.
///
/// # Panics
///
/// Panics outside a browser window.
#[must_use]
pub fn connected_root() -> Element {
  let root = document().create_element("div").expect("a root");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("a connected root");

  root
}

/// Installs the browser leaf on a [`connected_root`], returning the guard and the root.
///
/// # Panics
///
/// Panics outside a browser window, or if the leaf is already installed.
pub fn fresh_root() -> (Installed, Element) {
  let root = connected_root();

  crate::install(root.clone());

  (Installed(root.clone()), root)
}

/// The first element child of `root`: the element a view built directly under it
/// realizes.
///
/// # Panics
///
/// Panics if `root` has no element child.
#[must_use]
pub fn only_child(root: &Element) -> Element {
  root.first_element_child().expect("a built child")
}

/// The computed value of the style `property` of `element`.
///
/// # Panics
///
/// Panics outside a browser window.
#[must_use]
pub fn computed(element: &Element, property: &str) -> String {
  web_sys::window()
    .expect("a window")
    .get_computed_style(element)
    .expect("a computed style")
    .expect("a computed style")
    .get_property_value(property)
    .expect("a property value")
}

/// Dispatches a bubbling event named `name` on `target`.
///
/// # Panics
///
/// Panics if the engine refuses to construct the event.
pub fn dispatch(target: &Element, name: &str) {
  let init = web_sys::EventInit::new();

  init.set_bubbles(true);

  let event = web_sys::Event::new_with_event_init_dict(name, &init).expect("an event");
  let _ = target.dispatch_event(&event);
}

/// Waits `duration` on the browser's clock.
///
/// # Panics
///
/// Panics outside a browser window, or for a wait longer than `i32::MAX` milliseconds.
pub async fn wait(duration: Duration) {
  let milliseconds = i32::try_from(duration.as_millis()).expect("a short wait");
  let promise = js_sys::Promise::new(&mut |resolve, _| {
    let _ = web_sys::window()
      .expect("a window")
      .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, milliseconds);
  });

  let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Waits for the browser's next rendering step, after which a box is measured and
/// painted.
pub async fn next_frame() {
  wait(Duration::from_millis(20)).await;
}

/// Waits for the browser's rendering steps until `condition` holds, for at most a hundred
/// of them; whether it held.
pub async fn frames_until(condition: impl Fn() -> bool) -> bool {
  for _ in 0..100 {
    if condition() {
      return true;
    }

    next_frame().await;
  }

  condition()
}

/// Yields one turn of the microtask queue.
///
/// # Panics
///
/// Panics if a resolved promise rejects.
pub async fn tick() {
  wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&wasm_bindgen::JsValue::UNDEFINED))
    .await
    .expect("a resolved promise");
}
