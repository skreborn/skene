//! The gallery against axe-core in a real browser: every built-in view and control,
//! mounted live, must raise no accessibility violation, and every control, link,
//! picture and dialog it shows is named, as a reader finds it.
//!
//! Run in a headless browser:
//! `cargo test -p gallery --target wasm32-unknown-unknown --test browser_axe`.
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::fmt::Write as _;

use js_sys::{Array, Function, Object, Promise, Reflect};
use skene_web::{
  platform::mount,
  prelude::*,
  testing::{document, fresh_root},
};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{HtmlElement, KeyboardEvent, KeyboardEventInit, MouseEvent, MouseEventInit};

wasm_bindgen_test_configure!(run_in_browser);

/// axe-core, vendored under `vendor/` with its license.
const AXE: &str = include_str!("vendor/axe.min.js");

/// A property of a JavaScript object as a string, or the empty string.
fn field(value: &JsValue, name: &str) -> String {
  Reflect::get(value, &JsValue::from_str(name))
    .ok()
    .and_then(|value| value.as_string())
    .unwrap_or_default()
}

/// Loads axe-core into the page, unless an earlier test has.
fn load_axe() {
  let window = web_sys::window().expect("a window");

  if Reflect::has(&window, &JsValue::from_str("axe")).expect("the window's properties") {
    return;
  }

  let script = document()
    .create_element("script")
    .expect("a script element");

  script.set_text_content(Some(AXE));
  document()
    .head()
    .expect("a head")
    .append_child(&script)
    .expect("load axe");
}

/// axe's rules requiring a name: each control, link, picture, dialog, heading, tooltip
/// and range has one.
const NAMING_RULES: &[&str] = &[
  "aria-command-name",
  "aria-dialog-name",
  "aria-input-field-name",
  "aria-meter-name",
  "aria-progressbar-name",
  "aria-toggle-field-name",
  "aria-tooltip-name",
  "aria-treeitem-name",
  "button-name",
  "empty-heading",
  "frame-title",
  "image-alt",
  "input-button-name",
  "label",
  "link-name",
  "role-img-alt",
  "select-name",
  "svg-img-alt",
];

/// The ids of the rules a result list of axe's names.
fn rule_ids(results: &JsValue, list: &str) -> Vec<String> {
  let rules: Array = Reflect::get(results, &JsValue::from_str(list))
    .unwrap_or_else(|_| panic!("the {list}"))
    .into();

  rules.iter().map(|rule| field(&rule, "id")).collect()
}

/// The options running only `rules`, or every rule where `rules` is empty.
fn run_options(rules: &[&str]) -> Object {
  let options = Object::new();

  if !rules.is_empty() {
    let run_only = Object::new();
    let values: Array = rules.iter().copied().map(JsValue::from_str).collect();

    Reflect::set(&run_only, &"type".into(), &"rule".into()).expect("the run's type");
    Reflect::set(&run_only, &"values".into(), &values).expect("the run's rules");
    Reflect::set(&options, &"runOnly".into(), &run_only).expect("the run's selection");
  }

  options
}

/// What axe finds wrong with the page in `state` under `rules` (every rule where
/// `rules` is empty): each violation with the elements it names, and each of `rules`
/// that left an element undecided or did not run.
async fn audit(state: &str, rules: &[&str]) -> Vec<String> {
  let run = Function::new_with_args("options", "return axe.run(document.body, options)");
  let promise: Promise = run
    .call1(&JsValue::NULL, &run_options(rules))
    .expect("start the run")
    .into();
  let results = JsFuture::from(promise).await.expect("the run's results");
  let violations: Array = Reflect::get(&results, &JsValue::from_str("violations"))
    .expect("the violations")
    .into();
  let mut found: Vec<String> = violations
    .iter()
    .map(|violation| {
      let mut line = format!(
        "{state}: {} ({}): {}",
        field(&violation, "id"),
        field(&violation, "impact"),
        field(&violation, "help")
      );
      let nodes: Array = Reflect::get(&violation, &JsValue::from_str("nodes"))
        .expect("the nodes")
        .into();

      for node in nodes.iter() {
        let _ = write!(line, "\n    {}", field(&node, "html"));
      }

      line
    })
    .collect();
  let mut ran = rule_ids(&results, "violations");

  ran.extend(rule_ids(&results, "passes"));
  ran.extend(rule_ids(&results, "inapplicable"));

  let undecided = rule_ids(&results, "incomplete");

  for rule in rules {
    if undecided.iter().any(|id| id == rule) {
      found.push(format!("{state}: {rule} left nodes undecided"));
    }
    else if !ran.iter().any(|id| id == rule) {
      found.push(format!("{state}: {rule} did not run"));
    }
  }

  found
}

/// Waits for what an event opened or closed to settle.
async fn settle() {
  let promise = Promise::new(&mut |resolve, _| {
    web_sys::window()
      .expect("a window")
      .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 100)
      .expect("a timeout");
  });

  JsFuture::from(promise).await.expect("the timeout");
}

/// The first element matching `selector` whose text contains `text`.
fn element_with_text(selector: &str, text: &str) -> HtmlElement {
  let found = document().query_selector_all(selector).expect("a selector");

  (0..found.length())
    .filter_map(|index| found.item(index))
    .filter_map(|node| node.dyn_into::<HtmlElement>().ok())
    .find(|element| {
      element
        .text_content()
        .is_some_and(|content| content.contains(text))
    })
    .unwrap_or_else(|| panic!("no {selector} reading {text:?}"))
}

/// Whether an overlay is open: a popover in the top layer.
fn overlay_open() -> bool {
  document()
    .query_selector(":popover-open")
    .expect("a selector")
    .is_some()
}

/// Presses `Escape` where focus is.
fn press_escape() {
  let init = KeyboardEventInit::new();

  init.set_key("Escape");
  init.set_bubbles(true);
  init.set_cancelable(true);

  let event = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).expect("a key");
  let target = document()
    .active_element()
    .or_else(|| document().body().map(Into::into))
    .expect("a target");

  let _ = target.dispatch_event(&event);
}

/// Presses the secondary button on `element`, as the menu gesture does.
#[expect(
  clippy::cast_possible_truncation,
  reason = "an event's client position is whole pixels"
)]
fn open_context_menu(element: &HtmlElement) {
  let rect = element.get_bounding_client_rect();
  let init = MouseEventInit::new();

  init.set_bubbles(true);
  init.set_cancelable(true);
  init.set_button(2);
  init.set_client_x((rect.left() + rect.width() / 2.) as i32);
  init.set_client_y((rect.top() + rect.height() / 2.) as i32);

  let event = MouseEvent::new_with_mouse_event_init_dict("contextmenu", &init).expect("a press");

  let _ = element.dispatch_event(&event);
}

/// What axe finds wrong under `rules` (every rule where `rules` is empty) with the
/// gallery mounted live, as drawn and with each of its overlays open: a tooltip, the
/// picker's popup and a context menu.
async fn audit_the_gallery(rules: &[&str]) -> Vec<String> {
  let (_guard, _root) = fresh_root();
  let _ = mount(|| gallery::app().build());

  load_axe();
  settle().await;

  let mut found = audit("the page", rules).await;
  let save = element_with_text("button", "Save");

  save.focus().expect("focus the described button");
  settle().await;

  assert!(overlay_open(), "focus opened no tooltip");

  found.extend(audit("a tooltip open", rules).await);
  save.blur().expect("blur the described button");
  settle().await;

  let face = element_with_text("[role=combobox]", "");

  face.click();
  settle().await;

  assert!(
    overlay_open(),
    "a click on the picker's face opened no popup"
  );

  found.extend(audit("the picker's popup open", rules).await);
  press_escape();
  settle().await;

  open_context_menu(&element_with_text("[tabindex]", "Right-click me"));
  settle().await;

  assert!(overlay_open(), "the menu gesture opened no context menu");

  found.extend(audit("a context menu open", rules).await);
  press_escape();
  settle().await;

  found
}

/// The gallery raises no violation as drawn, nor with any of its overlays open.
#[wasm_bindgen_test]
async fn the_gallery_raises_no_accessibility_violation() {
  let found = audit_the_gallery(&[]).await;

  assert!(
    found.is_empty(),
    "axe found {} violation(s):\n{}",
    found.len(),
    found.join("\n")
  );
}

/// Every control, link, picture and dialog the gallery shows is named, as drawn and with
/// each of its overlays open, each naming rule proven to have run.
#[wasm_bindgen_test]
async fn every_control_the_gallery_shows_is_named() {
  let found = audit_the_gallery(NAMING_RULES).await;

  assert!(
    found.is_empty(),
    "axe found {} naming failure(s):\n{}",
    found.len(),
    found.join("\n")
  );
}
