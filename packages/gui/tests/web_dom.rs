//! Real-DOM integration tests for the web realization of the built-in views.
//!
//! Each test installs the web platform rooted at a detached `<div>`, builds a view under
//! it, and asserts against the actual DOM the browser produced. The guard uninstalls and
//! resets the reactive runtime when it drops, isolating each test.
//!
//! Run in a headless browser:
//! `cargo test -p skene-gui --target wasm32-unknown-unknown`.
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::{cell::Cell, rc::Rc};

use skene_gui::{
  AnyView,
  Border,
  Color,
  Container,
  Deferred,
  DynamicView,
  HasChildren,
  Inset,
  Role,
  Text,
  TextStyle,
  TextTheme,
  View,
  dynamic,
  empty,
  reactive::{Runtime, RwSignal, SignalGet, SignalUpdate},
  reconcile,
  wasm_bindgen::JsCast,
  web::{self, mount},
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Document, Element, HtmlElement};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
  web_sys::window()
    .expect("a window")
    .document()
    .expect("a document")
}

/// Uninstalls the web platform when it drops, isolating each test.
struct Installed;

impl Drop for Installed {
  fn drop(&mut self) {
    web::uninstall();
  }
}

/// Installs the web platform rooted at a new detached `<div>`, returning the guard (drop it
/// to reset) and that root. Build a view under it with [`web::mount`], then read the root's
/// subtree.
fn fresh_root() -> (Installed, Element) {
  let root = document().create_element("div").expect("create root div");

  web::install(root.clone());

  (Installed, root)
}

/// The single view element built directly under the root.
fn only_child(root: &Element) -> Element {
  root.first_element_child().expect("root has a built child")
}

/// Connects `root` to the document, reads a computed style `property` off its only child,
/// then disconnects. Computed styles resolve only for a connected element.
fn computed_style(root: &Element, property: &str) -> String {
  document()
    .body()
    .expect("a body")
    .append_child(root)
    .expect("connect root");

  let element = only_child(root)
    .dyn_into::<HtmlElement>()
    .expect("an HTML element");
  let value = web_sys::window()
    .unwrap()
    .get_computed_style(&element)
    .unwrap()
    .expect("computed style")
    .get_property_value(property)
    .unwrap();

  root.remove();

  value
}

/// The text content of each of `parent`'s element children, in order.
fn child_texts(parent: &Element) -> Vec<String> {
  let children = parent.children();

  (0..children.length())
    .map(|index| children.item(index).unwrap().text_content().unwrap())
    .collect()
}

#[wasm_bindgen_test]
fn deferred_builds_its_content_on_the_client() {
  let (_guard, root) = fresh_root();

  // A `Deferred` is a streaming boundary only on the server; on the client it builds its
  // content directly, under a grouping `<div>`.
  mount(|| Deferred::new(|| Text::new("content")).build());

  let group = only_child(&root);

  assert_eq!(group.tag_name(), "DIV");
  assert_eq!(child_texts(&group), vec!["content".to_owned()]);
}

#[wasm_bindgen_test]
fn container_is_a_div() {
  let (_guard, root) = fresh_root();

  mount(|| Container::new().build());

  assert_eq!(only_child(&root).tag_name(), "DIV");
}

#[wasm_bindgen_test]
fn button_role_realizes_a_button_element() {
  let (_guard, root) = fresh_root();

  mount(|| Container::new().set_role(Role::Button).build());

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "BUTTON");
  assert_eq!(element.get_attribute("type").as_deref(), Some("button"));
}

#[wasm_bindgen_test]
fn text_is_a_span_with_its_content() {
  let (_guard, root) = fresh_root();

  mount(|| Text::new("hello").build());

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "SPAN");
  assert_eq!(element.text_content().as_deref(), Some("hello"));
}

#[wasm_bindgen_test]
fn linked_text_is_an_anchor_element() {
  let (_guard, root) = fresh_root();

  mount(|| Text::new("go").set_link("https://example.com/").build());

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "A");
  assert_eq!(
    element.get_attribute("href").as_deref(),
    Some("https://example.com/"),
  );
}

#[wasm_bindgen_test]
fn children_mount_in_order() {
  let (_guard, root) = fresh_root();

  mount(|| {
    Container::row()
      .set_children((Text::new("a"), Text::new("b"), Text::new("c")))
      .build()
  });

  let container = only_child(&root);
  let children = container.children();

  assert_eq!(children.length(), 3);
  assert_eq!(
    children.item(0).and_then(|c| c.text_content()).as_deref(),
    Some("a"),
  );
  assert_eq!(
    children.item(2).and_then(|c| c.text_content()).as_deref(),
    Some("c"),
  );
}

#[wasm_bindgen_test]
fn styling_interns_a_generated_class() {
  let (_guard, root) = fresh_root();

  mount(|| {
    Container::new()
      .set_color(Color::srgb(1.0, 0.0, 0.0))
      .build()
  });

  let class = only_child(&root)
    .get_attribute("class")
    .expect("styled node has a class");

  assert!(
    class
      .split_whitespace()
      .any(|name| name.starts_with("skene-")),
    "expected a generated `skene-` class, got {class:?}",
  );
}

#[wasm_bindgen_test]
fn interned_class_actually_applies_its_style() {
  let (_guard, root) = fresh_root();

  mount(|| {
    Container::new()
      .set_padding(Inset::all(10.0))
      .set_border(Border::new(2.0, Color::srgb(0.0, 0.0, 0.0)))
      .build()
  });

  // Computed style resolves only for a connected element; the sheet lives in `<head>`.
  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect root");

  let element = only_child(&root)
    .dyn_into::<HtmlElement>()
    .expect("an HTML element");
  let style = web_sys::window()
    .unwrap()
    .get_computed_style(&element)
    .unwrap()
    .expect("computed style");

  assert_eq!(style.get_property_value("padding-top").unwrap(), "10px");
  assert_eq!(style.get_property_value("border-top-width").unwrap(), "2px");

  root.remove();
}

#[wasm_bindgen_test]
fn stack_children_share_one_grid_cell() {
  let (_guard, root) = fresh_root();

  mount(|| Container::stack().set_child(Text::new("only")).build());

  let child = only_child(&only_child(&root))
    .dyn_into::<HtmlElement>()
    .expect("an HTML element");

  assert_eq!(
    child.style().get_property_value("grid-area").unwrap(),
    "1 / 1"
  );
}

#[wasm_bindgen_test]
fn click_handler_runs_on_a_real_click() {
  let (_guard, root) = fresh_root();
  let clicks = Rc::new(Cell::new(0u32));

  mount({
    let clicks = Rc::clone(&clicks);

    move || {
      Container::new()
        .set_role(Role::Button)
        .handle_click(move || clicks.set(clicks.get() + 1))
        .build()
    }
  });

  let button = only_child(&root)
    .dyn_into::<HtmlElement>()
    .expect("an HTML element");

  button.click();
  button.click();

  assert_eq!(clicks.get(), 2);
}

/// A handler may re-enter the platform — the click path clones handlers out before running
/// them so building or destroying nodes from inside one does not deadlock the state borrow.
#[wasm_bindgen_test]
fn a_click_handler_may_build_nodes() {
  let (_guard, root) = fresh_root();

  mount(|| {
    Container::new()
      .set_role(Role::Button)
      .handle_click(|| {
        // Re-enter the platform from inside a handler.
        let _ = web::create_element("div");
      })
      .build()
  });

  only_child(&root)
    .dyn_into::<HtmlElement>()
    .expect("an HTML element")
    .click();
}

#[wasm_bindgen_test]
fn derived_text_updates_the_dom_on_change() {
  let (_guard, root) = fresh_root();
  let count = RwSignal::new(0);

  mount(move || Text::derive(move || count.get().to_string()).build());

  let element = only_child(&root);

  assert_eq!(element.text_content().as_deref(), Some("0"));

  count.set(5);
  Runtime::drain_pending_work();

  assert_eq!(element.text_content().as_deref(), Some("5"));
}

#[wasm_bindgen_test]
fn derived_style_overrides_inline_on_change() {
  let (_guard, root) = fresh_root();
  let size = RwSignal::new(12.0f32);

  mount(move || Text::new("t").derive_font_size(move || size.get()).build());

  let element = only_child(&root)
    .dyn_into::<HtmlElement>()
    .expect("an HTML element");

  assert_eq!(
    element.style().get_property_value("font-size").unwrap(),
    "12px"
  );

  size.set(20.0);
  Runtime::drain_pending_work();

  assert_eq!(
    element.style().get_property_value("font-size").unwrap(),
    "20px"
  );
}

#[wasm_bindgen_test]
fn theme_link_style_inherits_and_updates_on_a_link() {
  let (_guard, root) = fresh_root();
  let weight = RwSignal::new(400.0f32);

  mount(move || {
    TextTheme::new(Text::new("go").set_link("https://example.com/"))
      .set_link(TextStyle::new().derive_font_weight(move || weight.get()))
      .build()
  });

  let link = only_child(&root)
    .dyn_into::<HtmlElement>()
    .expect("an HTML element");

  assert_eq!(link.tag_name(), "A");
  assert_eq!(
    link.style().get_property_value("font-weight").unwrap(),
    "400"
  );

  weight.set(700.0);
  Runtime::drain_pending_work();

  assert_eq!(
    link.style().get_property_value("font-weight").unwrap(),
    "700"
  );
}

#[wasm_bindgen_test]
fn theme_link_style_skips_non_link_text() {
  let (_guard, root) = fresh_root();

  mount(|| {
    TextTheme::new(Text::new("plain"))
      .set_link(TextStyle::new().set_font_weight(700.0))
      .build()
  });

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect root");

  let text = only_child(&root)
    .dyn_into::<HtmlElement>()
    .expect("an HTML element");

  assert_eq!(text.tag_name(), "SPAN");

  // Computed weight is the default, not the theme's link weight: the link style reached
  // the non-link node through neither the interned class nor an inline override.
  let weight = web_sys::window()
    .unwrap()
    .get_computed_style(&text)
    .unwrap()
    .expect("computed style")
    .get_property_value("font-weight")
    .unwrap();

  assert_eq!(weight, "400");

  root.remove();
}

#[wasm_bindgen_test]
fn block_container_computes_as_flow_root() {
  let (_guard, root) = fresh_root();

  mount(|| Container::new().build());

  assert_eq!(computed_style(&root, "display"), "flow-root");
}

#[wasm_bindgen_test]
fn column_container_is_a_flex_column() {
  let (_guard, root) = fresh_root();

  mount(|| Container::column().build());

  assert_eq!(computed_style(&root, "display"), "flex");
  assert_eq!(computed_style(&root, "flex-direction"), "column");
}

#[wasm_bindgen_test]
fn grid_container_is_a_grid() {
  let (_guard, root) = fresh_root();

  mount(|| Container::grid(2).build());

  assert_eq!(computed_style(&root, "display"), "grid");
}

#[wasm_bindgen_test]
fn empty_renders_an_element_with_no_children() {
  let (_guard, root) = fresh_root();

  mount(|| empty().build());

  assert_eq!(only_child(&root).children().length(), 0);
}

#[wasm_bindgen_test]
fn a_low_level_view_reaches_the_platform_directly() {
  let (_guard, root) = fresh_root();

  // The escape hatch: a view that calls the platform's own functions, no trait involved.
  mount(|| {
    AnyView::new(|| {
      let node = web::create_element("section");

      web::set_text(node, "raw");

      node
    })
    .build()
  });

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "SECTION");
  assert_eq!(element.text_content().as_deref(), Some("raw"));
}

#[wasm_bindgen_test]
fn reconcile_reorders_the_real_dom_preserving_nodes() {
  let (_guard, root) = fresh_root();
  let items = RwSignal::new(vec![1, 2, 3]);

  mount(move || {
    let parent = web::create_element("div");

    reconcile(
      parent,
      move || items.get(),
      |item| *item,
      |item| {
        let node = web::create_element("span");

        web::set_text(node, &item.to_string());

        node
      },
    );

    parent
  });

  let parent = only_child(&root);

  assert_eq!(child_texts(&parent), ["1", "2", "3"]);

  // Mark the "2" node so a rebuild (rather than a move) would drop the marker.
  parent
    .children()
    .item(1)
    .unwrap()
    .set_attribute("data-keep", "yes")
    .unwrap();

  items.set(vec![3, 1, 2]);
  Runtime::drain_pending_work();

  assert_eq!(child_texts(&parent), ["3", "1", "2"]);

  let moved = parent.children().item(2).unwrap();

  assert_eq!(moved.text_content().as_deref(), Some("2"));
  assert_eq!(moved.get_attribute("data-keep").as_deref(), Some("yes"));
}

/// The primitive a library user reaches for: swap between two whole trees on a state change.
#[wasm_bindgen_test]
fn dynamic_view_swaps_between_trees_on_state_change() {
  let (_guard, root) = fresh_root();
  let logged_in = RwSignal::new(false);

  mount(move || {
    DynamicView::new(move || {
      if logged_in.get() {
        Text::new("welcome").into_any()
      }
      else {
        Container::new().set_role(Role::Button).into_any()
      }
    })
    .build()
  });

  // `DynamicView` anchors its subtree under a group node.
  let anchor = only_child(&root);

  assert_eq!(anchor.first_element_child().unwrap().tag_name(), "BUTTON");

  logged_in.set(true);
  Runtime::drain_pending_work();

  let swapped = anchor.first_element_child().unwrap();

  assert_eq!(anchor.children().length(), 1, "the old tree is torn down");
  assert_eq!(swapped.tag_name(), "SPAN");
  assert_eq!(swapped.text_content().as_deref(), Some("welcome"));
}

#[wasm_bindgen_test]
fn dynamic_swaps_the_real_dom() {
  let (_guard, root) = fresh_root();
  let flag = RwSignal::new(true);

  mount(move || {
    dynamic(move || web::create_element(if flag.get() { "section" } else { "article" }))
  });

  let anchor = only_child(&root);

  assert_eq!(anchor.first_element_child().unwrap().tag_name(), "SECTION");

  flag.set(false);
  Runtime::drain_pending_work();

  assert_eq!(anchor.first_element_child().unwrap().tag_name(), "ARTICLE");
}

/// Only `items` decides when the list is diffed: a signal an item happens to read while
/// building must not subscribe the reconcile effect and re-run the whole diff.
#[wasm_bindgen_test]
fn an_items_own_signal_does_not_re_run_the_list_diff() {
  let (_guard, root) = fresh_root();
  let items = RwSignal::new(vec![1, 2]);
  let extra = RwSignal::new(0u32);
  let runs = Rc::new(Cell::new(0u32));

  mount({
    let runs = Rc::clone(&runs);

    move || {
      let parent = web::create_element("div");

      reconcile(
        parent,
        {
          let runs = Rc::clone(&runs);

          move || {
            runs.set(runs.get() + 1);

            items.get()
          }
        },
        |item| *item,
        move |item| {
          let node = web::create_element("span");

          // Read a signal directly while building this item.
          web::set_text(node, &format!("{item}-{}", extra.get()));

          node
        },
      );

      parent
    }
  });

  assert_eq!(runs.get(), 1, "one diff on mount");

  extra.set(9);
  Runtime::drain_pending_work();

  assert_eq!(
    runs.get(),
    1,
    "an item's own signal must not re-run the list diff"
  );

  // The list itself still drives the diff.
  items.set(vec![1, 2, 3]);
  Runtime::drain_pending_work();

  assert_eq!(runs.get(), 2, "changing the list re-runs the diff");
  assert_eq!(child_texts(&only_child(&root)).len(), 3);
}

/// The flip side: untracking an item's build must not muzzle the item's *own* effects. A
/// `derive_` property inside a reconciled item still tracks and updates.
#[wasm_bindgen_test]
fn a_derived_property_inside_an_item_still_updates() {
  let (_guard, root) = fresh_root();
  let items = RwSignal::new(vec![1]);
  let size = RwSignal::new(12.0f32);

  mount(move || {
    let parent = web::create_element("div");

    reconcile(
      parent,
      move || items.get(),
      |item| *item,
      move |item| {
        Text::new(item.to_string())
          .derive_font_size(move || size.get())
          .build()
      },
    );

    parent
  });

  let item = only_child(&only_child(&root))
    .dyn_into::<HtmlElement>()
    .expect("an HTML element");

  assert_eq!(
    item.style().get_property_value("font-size").unwrap(),
    "12px"
  );

  size.set(24.0);
  Runtime::drain_pending_work();

  assert_eq!(
    item.style().get_property_value("font-size").unwrap(),
    "24px"
  );
}
