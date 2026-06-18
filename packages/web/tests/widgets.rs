//! End-to-end widget tests against the real [`WebBackend`] and a live DOM.
//!
//! These run only under `wasm32` via `wasm-bindgen-test` in a browser, not on the
//! host: the backend drives `web_sys` directly.

#![cfg(target_arch = "wasm32")]
#![allow(missing_docs)]

use std::{cell::Cell, rc::Rc};

use skene_core::{BackendGuard, Color, Length, install_test_backend};
use skene_reactive::{RwSignal, SignalGet, SignalUpdate};
use skene_web::{Container, HasChildren, RichText, Span, Text, WebBackend, mount};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Element, HtmlElement, window};

wasm_bindgen_test_configure!(run_in_browser);

/// Installs a fresh backend rooted at a detached element, returning that root so a test
/// can inspect the resulting DOM alongside the guard that keeps the backend installed.
fn fresh_root() -> (Element, TestBackend<WebBackend>) {
  let document = window().unwrap().document().unwrap();
  let root = document.create_element("div").unwrap();

  let backend = install_test_backend(WebBackend::new(root.clone()));

  (root, backend)
}

/// A static label renders its text into the DOM.
#[wasm_bindgen_test]
fn text_renders_its_content() {
  let (root, _backend) = fresh_root();

  mount(|| Text::new("hello"));

  assert_eq!(root.text_content().unwrap(), "hello");
}

/// A derived label re-renders when its signal changes.
#[wasm_bindgen_test]
fn derived_text_updates_on_change() {
  let (root, _backend) = fresh_root();
  let count = RwSignal::new(0i32);

  mount(move || Text::derive(move || format!("Count: {}", count.get())));

  assert_eq!(root.text_content().unwrap(), "Count: 0");

  count.set(1);

  assert_eq!(root.text_content().unwrap(), "Count: 1");
}

/// A button's press handler runs on a real DOM click.
#[wasm_bindgen_test]
fn button_click_runs_handler() {
  let (root, _backend) = fresh_root();
  let clicks = Rc::new(Cell::new(0u32));

  mount({
    let clicks = Rc::clone(&clicks);

    move || {
      Container::new()
        .set_children([Text::new("+")])
        .on_press(move || clicks.set(clicks.get() + 1))
    }
  });

  let button = root
    .first_element_child()
    .unwrap()
    .dyn_into::<HtmlElement>()
    .unwrap();

  button.click();
  button.click();

  assert_eq!(clicks.get(), 2);
}

/// Rich text renders styled spans and a styled group of nested spans, exercising the
/// span build and group-inheritance paths.
#[wasm_bindgen_test]
fn rich_text_renders_styled_spans() {
  let (root, _backend) = fresh_root();

  mount(|| {
    RichText::new([
      Span::text("a").set_color(Color::BLACK),
      Span::group([Span::text("b"), Span::text("c")]).set_font_weight(700),
    ])
  });

  assert_eq!(root.text_content().unwrap(), "abc");
}

/// A derived text style is applied and updated as an inline style on the element,
/// overriding the interned class rather than re-interning it.
#[wasm_bindgen_test]
fn derived_text_style_updates_inline() {
  let (root, _backend) = fresh_root();
  let size = RwSignal::new(10u32);

  mount(move || Text::new("x").derive_font_size(move || Length::from(f64::from(size.get()))));

  let element = root
    .first_element_child()
    .unwrap()
    .dyn_into::<HtmlElement>()
    .unwrap();

  assert_eq!(
    element.style().get_property_value("font-size").unwrap(),
    "10px"
  );

  size.set(20);

  assert_eq!(
    element.style().get_property_value("font-size").unwrap(),
    "20px"
  );
}

/// A keyed list reconciles to the new order without dropping surviving items.
#[wasm_bindgen_test]
fn keyed_list_reorders() {
  let (root, _backend) = fresh_root();
  let items = RwSignal::new(vec![1u32, 2, 3]);

  mount(move || {
    Container::column().derive_children(
      move || items.get(),
      |item| *item,
      |item: u32| Text::new(item.to_string()),
    )
  });

  let list = root.first_element_child().unwrap();

  assert_eq!(list.text_content().unwrap(), "123");

  items.set(vec![3, 1, 2]);

  assert_eq!(list.text_content().unwrap(), "312");
}

/// A keyed list removes and tears down the nodes of items that leave.
#[wasm_bindgen_test]
fn keyed_list_removes_and_destroys() {
  let (root, _backend) = fresh_root();
  let items = RwSignal::new(vec![1u32, 2, 3]);

  mount(move || {
    Container::column().derive_children(
      move || items.get(),
      |item| *item,
      |item: u32| Text::new(item.to_string()),
    )
  });

  let list = root.first_element_child().unwrap();

  assert_eq!(list.child_element_count(), 3);

  items.set(vec![1, 3]);

  assert_eq!(list.text_content().unwrap(), "13");
  assert_eq!(list.child_element_count(), 2);
}
