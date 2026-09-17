//! The CPU path behind a painted box, in a real browser.
//!
//! The web leaf asks the page for WebGPU once, when the first box paints, and keeps the
//! answer for the page's life. This binary hides `navigator.gpu` before any box paints,
//! so every painted box here renders through vello's CPU renderer and is blitted into
//! its canvas; `web_dom` covers the GPU path with WebGPU switched on.
//!
//! Run in a headless browser:
//! `cargo test -p skene-gui --target wasm32-unknown-unknown --test web_painted`.
#![cfg(skene_leaf = "browser")]

use skene_gui::{
  Color,
  Painted,
  View,
  core::Dimension,
  kurbo::Rect,
  wasm_bindgen::{JsCast, JsValue},
  web::{self, mount},
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{CanvasRenderingContext2d, Document, Element, HtmlCanvasElement};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
  web_sys::window()
    .expect("a window")
    .document()
    .expect("a document")
}

/// Uninstalls the platform and removes the root when it drops, isolating each test.
struct Installed(Element);

impl Drop for Installed {
  fn drop(&mut self) {
    web::uninstall();
    self.0.remove();
  }
}

/// Installs the platform rooted at a fresh connected `<div>`, returning the guard (drop
/// it to reset) and that root.
fn fresh_root() -> (Installed, Element) {
  let root = document().create_element("div").expect("create root div");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect root");
  web::install(root.clone());

  (Installed(root.clone()), root)
}

/// Waits for the browser's next rendering step, after which a box is measured and
/// painted.
async fn next_frame() {
  let promise = js_sys::Promise::new(&mut |resolve, _| {
    let _ = web_sys::window()
      .expect("a window")
      .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 20);
  });

  let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Shadows `navigator.gpu` with `undefined` on this page, so the web leaf finds no
/// WebGPU and paints on the CPU.
fn hide_webgpu() {
  let navigator = web_sys::window().expect("a window").navigator();
  let descriptor = js_sys::Object::new();

  js_sys::Reflect::set(
    &descriptor,
    &JsValue::from_str("value"),
    &JsValue::UNDEFINED,
  )
  .expect("the value");
  js_sys::Reflect::set(
    &descriptor,
    &JsValue::from_str("configurable"),
    &JsValue::TRUE,
  )
  .expect("configurable");
  js_sys::Object::define_property(&navigator, &JsValue::from_str("gpu"), &descriptor);

  assert!(
    js_sys::Reflect::get(&navigator, &JsValue::from_str("gpu"))
      .expect("navigator.gpu")
      .is_undefined(),
    "navigator.gpu is hidden"
  );
}

/// The center pixel of a canvas the CPU path blitted, read through its own 2d context.
fn center_pixel(canvas: &HtmlCanvasElement, context: &CanvasRenderingContext2d) -> Vec<u8> {
  context
    .get_image_data(
      f64::from(canvas.width() / 2),
      f64::from(canvas.height() / 2),
      1.,
      1.,
    )
    .expect("image data")
    .data()
    .to_vec()
}

/// Without WebGPU a painted box renders on the CPU and is blitted into its canvas
/// through a 2d context, sized to the box, so the fill shows.
#[wasm_bindgen_test]
async fn a_painted_box_is_blitted_without_webgpu() {
  hide_webgpu();

  let (_installed, root) = fresh_root();
  let _ = mount(|| {
    Painted::new(|canvas, size| {
      canvas.fill(
        &Rect::new(0., 0., f64::from(size.width), f64::from(size.height)),
        Color::srgb(0., 0., 1.),
      );
    })
    .set_width(Dimension::Length(40.))
    .set_height(Dimension::Length(20.))
    .build()
  });
  next_frame().await;
  next_frame().await;

  let canvas: HtmlCanvasElement = root
    .first_element_child()
    .expect("the painted box")
    .first_element_child()
    .expect("the canvas")
    .dyn_into()
    .expect("a canvas element");
  let context: CanvasRenderingContext2d = canvas
    .get_context("2d")
    .expect("a context")
    .expect("the 2d context the blit made")
    .dyn_into()
    .expect("a 2d context");

  assert!(
    canvas.width() >= 40 && canvas.height() >= 20,
    "sized to the box"
  );
  assert_eq!(
    &center_pixel(&canvas, &context)[0..4],
    &[0, 0, 255, 255],
    "the center pixel is the fill"
  );
}

/// A box that resizes is repainted on the CPU at its new size.
#[wasm_bindgen_test]
async fn a_resized_box_is_blitted_again() {
  use skene_gui::reactive::{Get, Set, Signal};

  hide_webgpu();

  let (_installed, root) = fresh_root();
  let width = Signal::new(40.);
  let _ = mount(|| {
    Painted::new(|canvas, size| {
      canvas.fill(
        &Rect::new(0., 0., f64::from(size.width), f64::from(size.height)),
        Color::srgb(1., 0., 0.),
      );
    })
    .derive_width(move || Dimension::Length(width.get()))
    .set_height(Dimension::Length(20.))
    .build()
  });
  next_frame().await;
  next_frame().await;

  let canvas: HtmlCanvasElement = root
    .first_element_child()
    .expect("the painted box")
    .first_element_child()
    .expect("the canvas")
    .dyn_into()
    .expect("a canvas element");
  let before = canvas.width();

  width.set(80.);
  next_frame().await;
  next_frame().await;

  let context: CanvasRenderingContext2d = canvas
    .get_context("2d")
    .expect("a context")
    .expect("the 2d context the blit made")
    .dyn_into()
    .expect("a 2d context");

  assert!(canvas.width() > before, "the canvas grew with the box");
  assert_eq!(
    &center_pixel(&canvas, &context)[0..4],
    &[255, 0, 0, 255],
    "the center of the grown canvas is the fill"
  );
}
