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
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Document, Element};

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

/// Every violation axe reports for the page, each with the elements it names.
async fn violations() -> Vec<String> {
  let script = document()
    .create_element("script")
    .expect("a script element");

  script.set_text_content(Some(AXE));
  document()
    .head()
    .expect("a head")
    .append_child(&script)
    .expect("load axe");

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
        "{} ({}): {}",
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
  let violations = violations().await;

  assert!(
    violations.is_empty(),
    "axe found {} violation(s):\n{}",
    violations.len(),
    violations.join("\n")
  );
}
