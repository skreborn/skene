//! The gallery against axe-core in a real browser: every built-in view and control,
//! mounted live, must raise no accessibility violation.
//!
//! Run in a headless browser:
//! `cargo test -p gallery --target wasm32-unknown-unknown --test web_axe`.
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::fmt::Write as _;

use js_sys::{Array, Function, Promise, Reflect};
use skene_gui::{
  prelude::*,
  web::{self, mount},
};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{
  Document,
  Element,
  HtmlElement,
  KeyboardEvent,
  KeyboardEventInit,
  MouseEvent,
  MouseEventInit,
};

wasm_bindgen_test_configure!(run_in_browser);

/// axe-core, vendored under `vendor/` with its license.
const AXE: &str = include_str!("vendor/axe.min.js");

fn document() -> Document {
  web_sys::window()
    .expect("a window")
    .document()
    .expect("a document")
}

/// Uninstalls the platform and removes the root when it drops.
struct Installed(Element);

impl Drop for Installed {
  fn drop(&mut self) {
    web::uninstall();
    self.0.remove();
  }
}

/// A property of a JavaScript object as a string, or the empty string.
fn field(value: &JsValue, name: &str) -> String {
  Reflect::get(value, &JsValue::from_str(name))
    .ok()
    .and_then(|value| value.as_string())
    .unwrap_or_default()
}

/// Loads axe-core into the page.
fn load_axe() {
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

/// Every violation axe reports for the page in `state`, each with the elements it names.
async fn violations(state: &str) -> Vec<String> {
  let run = Function::new_no_args("return axe.run(document.body)");
  let promise: Promise = run.call0(&JsValue::NULL).expect("start the run").into();
  let results = JsFuture::from(promise).await.expect("the run's results");
  let violations: Array = Reflect::get(&results, &JsValue::from_str("violations"))
    .expect("the violations")
    .into();

  violations
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
    .collect()
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

/// The gallery raises no violation as drawn, nor with any of its overlays open: a
/// tooltip, the picker's popup and a context menu.
#[wasm_bindgen_test]
async fn the_gallery_raises_no_accessibility_violation() {
  let root = document().create_element("div").expect("a root");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");
  web::install(root.clone());

  let _guard = Installed(root.clone());
  let _ = mount(|| gallery::app().into_view().build());

  load_axe();
  settle().await;

  let mut found = violations("the page").await;
  let save = element_with_text("button", "Save");

  save.focus().expect("focus the described button");
  settle().await;

  assert!(overlay_open(), "focus opened no tooltip");

  found.extend(violations("a tooltip open").await);
  save.blur().expect("blur the described button");
  settle().await;

  let face = element_with_text("[role=combobox]", "");

  face.click();
  settle().await;

  assert!(
    overlay_open(),
    "a click on the picker's face opened no popup"
  );

  found.extend(violations("the picker's popup open").await);
  press_escape();
  settle().await;

  open_context_menu(&element_with_text("[tabindex]", "Right-click me"));
  settle().await;

  assert!(overlay_open(), "the menu gesture opened no context menu");

  found.extend(violations("a context menu open").await);
  press_escape();

  assert!(
    found.is_empty(),
    "axe found {} violation(s):\n{}",
    found.len(),
    found.join("\n")
  );
}
