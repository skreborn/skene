//! The browser's URL primitives: read the path, push history entries, observe
//! back/forward navigation.

use std::cell::RefCell;

use wasm_bindgen::{JsCast, JsValue, closure::Closure};

/// A live popstate listener callback.
type PopListener = Closure<dyn FnMut(web_sys::Event)>;

thread_local! {
  /// The live popstate listeners, kept alive for the page's lifetime.
  static POP_LISTENERS: RefCell<Vec<PopListener>> = const { RefCell::new(Vec::new()) };
}

/// The window's current path (`location.pathname`).
///
/// # Panics
///
/// Panics if there is no `window` (i.e. not running in a browser).
#[must_use]
pub fn current_path() -> String {
  web_sys::window()
    .expect("a window")
    .location()
    .pathname()
    .unwrap_or_else(|_| "/".to_owned())
}

/// Pushes a history entry for `path` without reloading.
pub fn push_path(path: &str) {
  if let Some(window) = web_sys::window()
    && let Ok(history) = window.history()
  {
    let _ = history.push_state_with_url(&JsValue::NULL, "", Some(path));
  }
}

/// Runs `on_pop` with the new path on every back/forward navigation, for the page's
/// lifetime.
pub fn on_pop(on_pop: impl Fn(String) + 'static) {
  let Some(window) = web_sys::window()
  else {
    return;
  };

  let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
    on_pop(current_path());
  }) as Box<dyn FnMut(web_sys::Event)>);

  let _ = window.add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref());

  POP_LISTENERS.with(|listeners| listeners.borrow_mut().push(closure));
}
