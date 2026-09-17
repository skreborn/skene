//! The map against a real DOM: a `<canvas>` of the map's size inside its container.
//!
//! Run in a headless browser:
//! `cargo test -p acme-map --target wasm32-unknown-unknown --test web_map`.
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use acme_map::Map;
use skene_core::{Size, View};
use skene_web::Web;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Document, Element};

wasm_bindgen_test_configure!(run_in_browser);

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
    skene_web::uninstall();
    self.0.remove();
  }
}

fn computed(element: &Element, property: &str) -> String {
  web_sys::window()
    .expect("a window")
    .get_computed_style(element)
    .expect("computed style")
    .expect("a computed style")
    .get_property_value(property)
    .expect("a property value")
}

#[wasm_bindgen_test]
fn the_map_paints_a_canvas_of_its_size() {
  let root = document().create_element("div").expect("a root");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");
  skene_web::install(root.clone());

  let _guard = Installed(root.clone());
  let _ = skene_web::mount(|| Map::<Web>::new().set_size(Size::new(200., 80.)).build());

  let container = root.first_element_child().expect("the container");
  let canvas = container.first_element_child().expect("the canvas");

  assert_eq!(canvas.tag_name(), "CANVAS");
  assert_eq!(canvas.get_attribute("width").as_deref(), Some("200"));
  assert_eq!(canvas.get_attribute("height").as_deref(), Some("80"));
  assert_eq!(canvas.get_attribute("role").as_deref(), Some("img"));
  assert_eq!(computed(&container, "width"), "200px");
  assert_eq!(computed(&canvas, "height"), "80px");
}
