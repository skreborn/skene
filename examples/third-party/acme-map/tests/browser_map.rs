//! The map against a real DOM: a `<canvas>` of the map's size inside its container.
//!
//! Run in a headless browser:
//! `cargo test -p acme-map --target wasm32-unknown-unknown --test browser_map`.
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use acme_map::Map;
use skene_foundation::{Size, View};
use skene_web::{
  platform::{self, Web},
  testing::{computed, fresh_root},
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn the_map_paints_a_canvas_of_its_size() {
  let (_guard, root) = fresh_root();
  let _ = platform::mount(|| Map::<Web>::new().set_size(Size::new(200., 80.)).build());

  let container = root.first_element_child().expect("the container");
  let canvas = container.first_element_child().expect("the canvas");

  assert_eq!(canvas.tag_name(), "CANVAS");
  assert_eq!(canvas.get_attribute("width").as_deref(), Some("200"));
  assert_eq!(canvas.get_attribute("height").as_deref(), Some("80"));
  assert_eq!(canvas.get_attribute("role").as_deref(), Some("img"));
  assert_eq!(computed(&container, "width"), "200px");
  assert_eq!(computed(&canvas, "height"), "80px");
}
