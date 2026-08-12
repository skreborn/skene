//! Real-DOM integration tests for the web realization of the built-in views.
//!
//! Each test installs the web platform rooted at a detached `<div>`, builds a view under
//! it, and asserts against the actual DOM the browser produced. The guard uninstalls and
//! resets the reactive runtime when it drops, isolating each test.
//!
//! Run in a headless browser:
//! `cargo test -p skene-gui --target wasm32-unknown-unknown`.
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_gui::{
  AnyView,
  Border,
  Color,
  Column,
  Deferred,
  DerivedView,
  FocusHandle,
  Grid,
  HasChildren,
  Inset,
  PointerButton,
  RawCheckbox,
  RawTextField,
  Role,
  Row,
  Stack,
  StyledBox,
  Text,
  TextStyle,
  TextTheme,
  View,
  derived,
  empty,
  head,
  reactive::{Get, Runtime, Set, Signal},
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

  mount(|| StyledBox::new().build());

  assert_eq!(only_child(&root).tag_name(), "DIV");
}

#[wasm_bindgen_test]
fn role_realizes_its_semantic_element() {
  let (_guard, root) = fresh_root();

  mount(|| StyledBox::new().set_role(Role::Navigation).build());

  assert_eq!(only_child(&root).tag_name(), "NAV");
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
    Row::new()
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
    StyledBox::new()
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
    StyledBox::new()
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

  mount(|| Stack::new().set_child(Text::new("only")).build());

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
      StyledBox::new()
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

/// A delegated pointer-down handler receives the event with its position and button.
#[wasm_bindgen_test]
fn pointer_down_handler_receives_the_event() {
  let (_guard, root) = fresh_root();
  let seen = Rc::new(Cell::new(None));

  mount({
    let seen = Rc::clone(&seen);

    move || {
      StyledBox::new()
        .handle_pointer_down(move |event| seen.set(Some((event.x, event.y, event.button))))
        .build()
    }
  });

  let target = only_child(&root);
  let init = web_sys::PointerEventInit::new();

  init.set_bubbles(true);
  init.set_client_x(11);
  init.set_client_y(7);
  init.set_button(0);

  let event =
    web_sys::PointerEvent::new_with_event_init_dict("pointerdown", &init).expect("a pointer event");

  let _ = target.dispatch_event(&event);

  assert_eq!(seen.get(), Some((11., 7., Some(PointerButton::Primary))));
}

/// A change handler receives the value, in real time, when an input event fires.
#[wasm_bindgen_test]
fn change_handler_receives_the_committed_value() {
  let (_guard, root) = fresh_root();
  let seen = Rc::new(RefCell::new(String::new()));

  mount({
    let seen = Rc::clone(&seen);

    move || {
      RawTextField::new()
        .set_value("committed")
        .handle_change(move |value| value.clone_into(&mut seen.borrow_mut()))
        .build()
    }
  });

  let target = only_child(&root);
  let init = web_sys::EventInit::new();

  init.set_bubbles(true);

  let event = web_sys::Event::new_with_event_init_dict("input", &init).expect("an event");
  let _ = target.dispatch_event(&event);

  assert_eq!(&*seen.borrow(), "committed");
}

/// A checkbox bound to a signal reflects it and writes toggles back through a real
/// input event.
#[wasm_bindgen_test]
fn checkbox_binding_round_trips() {
  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);

  mount(move || RawCheckbox::new().bind_checked(checked).build());

  let target = only_child(&root)
    .dyn_into::<web_sys::HtmlInputElement>()
    .expect("an input element");

  assert_eq!(target.get_attribute("type").as_deref(), Some("checkbox"));
  assert!(!target.checked());

  checked.set(true);

  assert!(target.checked());

  target.set_checked(false);

  let init = web_sys::EventInit::new();

  init.set_bubbles(true);

  let event = web_sys::Event::new_with_event_init_dict("input", &init).expect("an event");
  let _ = target.dispatch_event(&event);

  assert!(!checked.get());
}

/// Head registrations set the live document title and add each element once.
#[wasm_bindgen_test]
fn head_registrations_apply_once() {
  let (_guard, _root) = fresh_root();

  head::set_title("Live title");
  head::add_meta("live-test", "value");
  head::add_meta("live-test", "value");

  assert_eq!(document().title(), "Live title");

  let head = document().head().expect("a head");
  let mut matches = 0;

  for index in 0..head.children().length() {
    let element = head.children().item(index).expect("an element");

    if element.get_attribute("name").as_deref() == Some("live-test") {
      matches += 1;

      element.remove();
    }
  }

  assert_eq!(matches, 1);
}

/// A focus handle moves real keyboard focus to its bound view.
#[wasm_bindgen_test]
fn focus_handle_focuses_the_bound_view() {
  let (_guard, root) = fresh_root();
  let handle = FocusHandle::new();

  // A detached root cannot take focus; attach it to the live document for this test.
  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("attach the test root");

  mount({
    let handle = handle.clone();

    move || RawTextField::new().bind_focus(&handle).build()
  });

  handle.focus();

  let target = only_child(&root);

  assert_eq!(document().active_element(), Some(target));

  handle.blur();

  assert_ne!(document().active_element(), Some(only_child(&root)));

  root.remove();
}

/// A handler may re-enter the platform—the click path clones handlers out before running
/// them so building or destroying nodes from inside one does not deadlock the state borrow.
#[wasm_bindgen_test]
fn a_click_handler_may_build_nodes() {
  let (_guard, root) = fresh_root();

  mount(|| {
    StyledBox::new()
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
  let count = Signal::new(0);

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
  let size = Signal::new(12.0f32);

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
  let weight = Signal::new(400.0f32);

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

  mount(|| StyledBox::new().build());

  assert_eq!(computed_style(&root, "display"), "flow-root");
}

#[wasm_bindgen_test]
fn column_container_is_a_flex_column() {
  let (_guard, root) = fresh_root();

  mount(|| Column::new().build());

  assert_eq!(computed_style(&root, "display"), "flex");
  assert_eq!(computed_style(&root, "flex-direction"), "column");
}

#[wasm_bindgen_test]
fn grid_container_is_a_grid() {
  let (_guard, root) = fresh_root();

  mount(|| Grid::uniform(2).build());

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
  let items = Signal::new(vec![1, 2, 3]);

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
  let logged_in = Signal::new(false);

  mount(move || {
    DerivedView::new(move || {
      if logged_in.get() {
        Text::new("welcome").into_any()
      }
      else {
        StyledBox::new().set_role(Role::Navigation).into_any()
      }
    })
    .build()
  });

  // `DerivedView` anchors its subtree under a group node.
  let anchor = only_child(&root);

  assert_eq!(anchor.first_element_child().unwrap().tag_name(), "NAV");

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
  let flag = Signal::new(true);

  mount(move || {
    derived(move || web::create_element(if flag.get() { "section" } else { "article" }))
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
  let items = Signal::new(vec![1, 2]);
  let extra = Signal::new(0u32);
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
  let items = Signal::new(vec![1]);
  let size = Signal::new(12.0f32);

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

/// Selection is opt-in: a plain run computes `user-select: none` through the reset,
/// and `set_selectable(true)` restores it.
#[wasm_bindgen_test]
fn unselectable_text_computes_user_select_none() {
  let (_guard, root) = fresh_root();

  mount(|| {
    Column::new()
      .set_children((
        Text::new("content").set_selectable(true),
        Text::new("chrome"),
      ))
      .build()
  });

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect root");

  let children = only_child(&root).children();
  let computed = |index: u32| {
    let element = children
      .item(index)
      .expect("a run")
      .dyn_into::<HtmlElement>()
      .expect("an HTML element");

    web_sys::window()
      .unwrap()
      .get_computed_style(&element)
      .unwrap()
      .expect("computed style")
      .get_property_value("user-select")
      .unwrap()
  };

  // A plain run computes `auto`, which resolves to the body's `none` at use time—the
  // reset is what turns selection off; the opt-in overrides it in place.
  let body_select = web_sys::window()
    .unwrap()
    .get_computed_style(&document().body().expect("a body"))
    .unwrap()
    .expect("computed style")
    .get_property_value("user-select")
    .unwrap();

  assert_eq!(body_select, "none");
  assert_eq!(computed(0), "text");
  assert_eq!(computed(1), "auto");

  root.remove();
}

/// An unstyled link keeps the reset's defaults—the scheme's link blue and the
/// underline—while a themed base ink never colors it.
#[wasm_bindgen_test]
fn links_default_to_blue_and_underline() {
  let (_installed, root) = fresh_root();

  mount(|| {
    TextTheme::new(Text::new("docs").set_link("https://example.com"))
      .set_base(TextStyle::new().set_color(Color::srgb(0.9, 0.9, 0.9)))
      .build()
  });

  assert_eq!(computed_style(&root, "color"), "rgb(0, 102, 204)");
  assert_eq!(computed_style(&root, "text-decoration-line"), "underline");
}
