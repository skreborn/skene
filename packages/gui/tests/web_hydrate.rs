//! Hydration: the client adopts a server-rendered DOM instead of rebuilding it.
//!
//! The server's markup is hand-written here with the `data-h` markers a real `render_to_string`
//! emits, so the client can match each node it builds to the server element carrying its id.
//!
//! Run in a headless browser:
//! `cargo test -p skene-gui --target wasm32-unknown-unknown`.
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_gui::{
  Container,
  HasChildren,
  Lazy,
  Resource,
  Text,
  wasm_bindgen::{JsCast, JsValue},
  web::{self, hydrate},
};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Document, Element, HtmlElement};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
  web_sys::window()
    .expect("a window")
    .document()
    .expect("a document")
}

/// A detached root holding `markup` as its server-rendered DOM.
fn server_root(markup: &str) -> Element {
  let root = document().create_element("div").expect("create root div");

  root.set_inner_html(markup);

  root
}

/// The number of descendant elements under `root`.
fn element_count(root: &Element) -> u32 {
  root
    .query_selector_all("*")
    .expect("query descendants")
    .length()
}

#[wasm_bindgen_test]
fn hydrate_adopts_the_marked_elements() {
  // The container builds as node 1, its child text as node 2—matching the markers.
  let root = server_root("<div data-h=\"1\"><span data-h=\"2\">Hi</span></div>");
  let server_container = root.first_element_child().expect("a container");
  let server_span = server_container.first_element_child().expect("a span");

  hydrate(root.clone(), "", || {
    Container::new().set_child(Text::new("Hi"))
  });

  let hydrated_container = root.first_element_child().expect("a container");

  assert!(
    server_container == hydrated_container,
    "the container was adopted, not recreated",
  );

  assert!(
    server_span == hydrated_container.first_element_child().expect("a span"),
    "the child span was adopted too",
  );

  assert_eq!(
    element_count(&root),
    2,
    "no elements were duplicated by hydration",
  );

  web::uninstall();
}

#[wasm_bindgen_test]
fn a_hydrated_handler_fires_on_the_adopted_node() {
  let root = server_root("<div data-h=\"1\"></div>");

  let clicked = Rc::new(Cell::new(false));

  hydrate(root.clone(), "", {
    let clicked = Rc::clone(&clicked);

    move || Container::new().handle_click(move || clicked.set(true))
  });

  root
    .first_element_child()
    .expect("the adopted container")
    .dyn_into::<HtmlElement>()
    .expect("an HtmlElement")
    .click();

  assert!(
    clicked.get(),
    "the hydrated handler fired on the server-rendered node",
  );

  web::uninstall();
}

#[wasm_bindgen_test]
fn a_resource_adopts_server_state_without_loading() {
  let root = server_root("<span data-h=\"1\">x</span>");

  let loaded = Rc::new(Cell::new(false));
  let captured: Rc<RefCell<Option<Resource<String>>>> = Rc::new(RefCell::new(None));

  // The server's state: one order-keyed JSON-encoded value, "Ada", and no tagged ones.
  hydrate(root.clone(), r#"[["\"Ada\""],[]]"#, {
    let loaded = Rc::clone(&loaded);
    let captured = Rc::clone(&captured);

    move || {
      let data = Resource::new({
        let loaded = Rc::clone(&loaded);

        move || {
          let loaded = Rc::clone(&loaded);

          async move {
            loaded.set(true);

            "loaded".to_owned()
          }
        }
      });

      *captured.borrow_mut() = Some(data);

      Text::new("hydrated")
    }
  });

  let data = (*captured.borrow()).expect("the resource was captured");

  assert!(
    !loaded.get(),
    "the loader did not run; the server value was adopted",
  );

  data.with(|value| {
    assert_eq!(
      value.map(String::as_str),
      Some("Ada"),
      "the adopted value is the server's, not the loader's",
    );
  });

  web::uninstall();
}

/// Yields to the event loop, letting spawned work make progress.
async fn tick() {
  JsFuture::from(js_sys::Promise::resolve(&JsValue::UNDEFINED))
    .await
    .expect("a resolved promise");
}

#[wasm_bindgen_test]
async fn a_lazy_view_keeps_server_content_until_it_loads() {
  // Client build order: container 1, lazy group 2. The server numbered the lazy
  // content later (9)—after its own synchronous build—so the claim, not the order,
  // must account for it.
  let root =
    server_root("<div data-h=\"1\"><div data-h=\"2\"><span data-h=\"9\">note</span></div></div>");
  let server_span = root.query_selector("span").expect("query").expect("a span");

  hydrate(root.clone(), "", || {
    Container::new().set_child(Lazy::new(|| async { Text::new("note") }))
  });

  // Claimed: the server content still stands, visible, while the module is pending.
  assert_eq!(element_count(&root), 3, "the server content was kept");
  assert_eq!(root.text_content().expect("text"), "note");

  for _ in 0..4 {
    tick().await;
  }

  // Swapped: the arrived content replaced the server span, nothing duplicated.
  assert_eq!(
    element_count(&root),
    3,
    "the arrived content replaced the claim"
  );
  assert_eq!(root.text_content().expect("text"), "note");

  let hydrated_span = root.query_selector("span").expect("query").expect("a span");

  assert!(
    server_span != hydrated_span,
    "the arrived content is freshly built, not the claim",
  );

  web::uninstall();
}

#[wasm_bindgen_test]
fn a_structural_mismatch_is_caught_not_silently_adopted() {
  // The server sent a <span> for marker 1, but the client builds a <div> there.
  let root = server_root("<span data-h=\"1\">x</span>");

  hydrate(root.clone(), "", Container::new);

  // The mismatched span is left in place and a fresh div is created rather than adopted, so the
  // divergence shows up as an extra node instead of a silently mis-typed one.
  assert_eq!(
    element_count(&root),
    2,
    "the mismatch produced a fresh node instead of adopting the wrong one",
  );

  web::uninstall();
}
