//! Hydration: the client adopts a server-rendered DOM instead of rebuilding it.
//!
//! The server's markup is hand-written here with the `data-h` markers and anchor
//! comments a real `render_to_string` emits, so the client can match each node it
//! builds to the server node carrying its marker.
//!
//! Run in a headless browser:
//! `cargo test -p skene-gui --target wasm32-unknown-unknown --test web_hydrate`.
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

/// The single view element built directly under `root`.
fn only_child(root: &Element) -> Element {
  root.first_element_child().expect("a built child")
}

/// A container's static child sits between the anchors of a segment of its own: the
/// container builds as element 1, its text as element 2, around the first segment.
#[wasm_bindgen_test]
fn hydrate_adopts_the_marked_elements() {
  let root =
    server_root("<div data-h=\"1\"><!--s:0--><span data-h=\"2\">Hi</span><!--e:0--></div>");
  let server_container = only_child(&root);
  let server_span = only_child(&server_container);

  hydrate(root.clone(), "", || {
    Container::new().set_child(Text::new("Hi"))
  });

  let hydrated_container = only_child(&root);

  assert_eq!(web::hydration_mismatches(), 0);
  assert!(
    server_container == hydrated_container,
    "the container was adopted, not recreated",
  );

  assert!(
    server_span == only_child(&hydrated_container),
    "the child span was adopted too",
  );

  assert_eq!(
    element_count(&root),
    2,
    "no elements were duplicated by hydration",
  );
  assert_eq!(
    hydrated_container.child_nodes().length(),
    3,
    "the anchors were adopted rather than added",
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

  only_child(&root)
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
  // Client build order: container 1, then the lazy segment inside the container's
  // child segment. The server numbered the lazy content later (9)—after its own
  // synchronous build—so the claim, not the order, must account for it.
  let root = server_root(
    "<div data-h=\"1\"><!--s:0--><!--s:1--><span data-h=\"9\">note</span><!--e:1--><!--e:0--></div>",
  );
  let server_span = root.query_selector("span").expect("query").expect("a span");

  hydrate(root.clone(), "", || {
    Container::new().set_child(Lazy::new(|| async { Text::new("note") }))
  });

  // Claimed: the server content still stands, visible, while the module is pending.
  assert_eq!(web::hydration_mismatches(), 0);
  assert_eq!(element_count(&root), 2, "the server content was kept");
  assert_eq!(root.text_content().expect("text"), "note");

  for _ in 0..4 {
    tick().await;
  }

  // Swapped: the arrived content replaced the server span, nothing duplicated.
  assert_eq!(
    element_count(&root),
    2,
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

  // The mismatched span is left in place and a fresh div is created rather than
  // adopted, so the divergence shows up as an extra node instead of a silently
  // mis-typed one, and is counted.
  assert_eq!(
    element_count(&root),
    2,
    "the mismatch produced a fresh node instead of adopting the wrong one",
  );
  assert!(web::hydration_mismatches() > 0);

  web::uninstall();
}

/// A streamed page leaves its style deltas in the body; hydration adopts them with the
/// head's block, so no rule is added twice and the delta stays the one copy.
#[wasm_bindgen_test]
fn a_streamed_delta_is_adopted_with_the_stylesheet() {
  use skene_gui::Color;

  let head = document().head().expect("a head");
  let block = document().create_element("style").expect("a style");

  let _ = block.set_attribute("id", "skene-styles");
  block.set_text_content(Some(""));
  let _ = head.append_child(&block);

  // The markup a streamed render leaves: the delta right before the content it styles.
  let app = || {
    Container::new()
      .set_color(Color::srgb(0.1, 0.2, 0.3))
      .set_child(Text::new("late"))
  };
  let (markup, rule) = {
    let staging = document().create_element("div").expect("a staging root");

    web::install(staging.clone());

    let _ = web::mount(|| skene_gui::View::build(app()));

    let container = only_child(&staging);
    let class = container.get_attribute("class").expect("a class");
    let rule = document()
      .query_selector_all("style[data-skene-style]")
      .expect("the styles")
      .item(1)
      .and_then(|style| style.text_content())
      .expect("the class's rule");

    let _ = container.set_attribute("data-h", "1");
    let _ = only_child(&container).set_attribute("data-h", "2");

    let markup = format!(
      "<style data-skene-style>{rule}</style>{}",
      staging.inner_html()
    );

    web::uninstall();

    assert!(rule.contains(&class));

    (markup, rule)
  };
  let root = server_root(&markup);

  hydrate(root.clone(), "", app);

  let styles = document()
    .query_selector_all("style[data-skene-style]")
    .expect("the styles");
  let copies = (0..styles.length())
    .filter_map(|index| styles.item(index))
    .filter_map(|style| style.text_content())
    .filter(|text| text.contains(&rule))
    .count();

  assert_eq!(web::hydration_mismatches(), 0);
  assert_eq!(copies, 1, "the delta is the one copy of its rule");

  web::uninstall();

  assert!(
    !block.is_connected(),
    "the adopted block goes with the driver"
  );
  assert!(
    document()
      .query_selector("style[data-skene-style]")
      .expect("query")
      .is_none(),
    "and so does the delta",
  );
}
