//! View transitions in the browser: the app's opt-in, the wrap a same-document
//! navigation runs under, and the cross-document rule the stylesheet carries.

use std::cell::Cell;

use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use super::{end_turn, try_with_dom};
use crate::motion::VIEW_TRANSITION_RULE;

thread_local! {
  /// Whether the app opted into view transitions.
  static ENABLED: Cell<bool> = const { Cell::new(false) };
}

/// Opts the page into view transitions.
///
/// A same-document navigation committed through [`commit_navigation`] runs under
/// `document.startViewTransition` where the browser has it, and the stylesheet carries
/// `@view-transition { navigation: auto }` for cross-document navigations. Both degrade
/// to a cut.
pub fn enable_view_transitions() {
  ENABLED.with(|enabled| enabled.set(true));

  try_with_dom(|dom| dom.sheet.add_rule(VIEW_TRANSITION_RULE));
}

/// Whether the page opted into view transitions.
#[inline(always)]
#[must_use]
pub fn view_transitions_enabled() -> bool {
  ENABLED.with(Cell::get)
}

/// Runs `update`, the state change of a navigation, as one turn.
///
/// It runs under a view transition when the page opted in and the browser has
/// `document.startViewTransition`, and plainly otherwise. The page shows the new state
/// once `update` returns, so a transition captures it.
pub fn commit_navigation(update: Box<dyn FnOnce()>) {
  let update = move || {
    update();
    end_turn();
  };

  if !view_transitions_enabled() {
    update();

    return;
  }

  let Some(document) = web_sys::window().and_then(|window| window.document())
  else {
    update();

    return;
  };
  let start = js_sys::Reflect::get(&document, &JsValue::from_str("startViewTransition"))
    .ok()
    .and_then(|start| start.dyn_into::<js_sys::Function>().ok());
  let Some(start) = start
  else {
    update();

    return;
  };
  let callback = Closure::once_into_js(update);

  if start.call1(&document, &callback).is_err() {
    tracing::warn!("startViewTransition failed; navigation committed as a cut");
  }
}

/// Forgets the opt-in, for a teardown.
pub(super) fn reset() {
  ENABLED.with(|enabled| enabled.set(false));
}
