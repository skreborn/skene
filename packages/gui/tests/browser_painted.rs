//! The CPU path behind a painted box, in a real browser.
//!
//! The web leaf asks the page for WebGPU once, when the first box paints, and keeps the
//! answer for the page's life. This binary hides `navigator.gpu` before any box paints,
//! so every painted box here renders through vello's CPU renderer and is blitted into
//! its canvas; `browser_dom` covers the GPU path with WebGPU switched on.
//!
//! Run in a headless browser:
//! `cargo test-web --test browser_painted`.
#![cfg(all(skene_platform = "web", not(skene_server)))]

use skene_foundation::{ColumnBase, Dimension, PaintedBase};
use skene_gui::{
  Color,
  HasChildren,
  Painted,
  View,
  kurbo::Rect,
  platform::web::mount,
  testing::{frames_until, fresh_root, next_frame},
  wasm_bindgen::{JsCast, JsValue},
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

wasm_bindgen_test_configure!(run_in_browser);

/// The canvas a painted box at the start of `root` draws into, once it holds one.
fn canvas_of(root: &web_sys::Element) -> Option<HtmlCanvasElement> {
  root
    .first_element_child()?
    .first_element_child()?
    .dyn_into()
    .ok()
}

/// Whether the canvas at the start of `root` is `width` CSS pixels wide in device
/// pixels, to within one, as a painted box sizes the canvas it paints.
fn sized(root: &web_sys::Element, width: f64) -> bool {
  let ratio = web_sys::window().expect("a window").device_pixel_ratio();

  canvas_of(root).is_some_and(|canvas| (f64::from(canvas.width()) - width * ratio).abs() < 1.)
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
    .set_width(40)
    .set_height(20)
    .build()
  });

  assert!(frames_until(|| sized(&root, 40.)).await, "the box painted");

  let canvas = canvas_of(&root).expect("the canvas");
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
    .set_height(20)
    .build()
  });

  assert!(frames_until(|| sized(&root, 40.)).await, "the box painted");

  width.set(80.);

  assert!(
    frames_until(|| sized(&root, 80.)).await,
    "the canvas grew with the box"
  );

  let canvas = canvas_of(&root).expect("the canvas");

  let context: CanvasRenderingContext2d = canvas
    .get_context("2d")
    .expect("a context")
    .expect("the 2d context the blit made")
    .dyn_into()
    .expect("a 2d context");

  assert_eq!(
    &center_pixel(&canvas, &context)[0..4],
    &[255, 0, 0, 255],
    "the center of the grown canvas is the fill"
  );
}

/// A canvas whose 2d context the browser loses is replaced by a fresh one, which the box
/// repaints.
#[wasm_bindgen_test]
async fn a_lost_2d_context_repaints_in_a_fresh_canvas() {
  hide_webgpu();

  let (_installed, root) = fresh_root();
  let _ = mount(|| {
    Painted::new(|canvas, size| {
      canvas.fill(
        &Rect::new(0., 0., f64::from(size.width), f64::from(size.height)),
        Color::srgb(0., 1., 0.),
      );
    })
    .set_width(40)
    .set_height(20)
    .build()
  });

  assert!(frames_until(|| sized(&root, 40.)).await, "the box painted");

  let lost = canvas_of(&root).expect("the canvas");

  let _ = lost.dispatch_event(&web_sys::Event::new("contextlost").expect("an event"));

  let repainted = || {
    canvas_of(&root)
      .filter(|canvas| *canvas != lost)
      .and_then(|canvas| {
        let context: CanvasRenderingContext2d = canvas.get_context("2d").ok()??.dyn_into().ok()?;

        Some(center_pixel(&canvas, &context)[0..4] == [0, 255, 0, 255])
      })
      .unwrap_or(false)
  };

  assert!(
    frames_until(repainted).await,
    "a fresh canvas took the lost one's place and was repainted"
  );
  assert!(!lost.is_connected());
}

/// A painted box with no height of its own takes none from its canvas, which stands out
/// of the box's flow.
#[wasm_bindgen_test]
async fn a_canvas_never_sizes_its_box() {
  hide_webgpu();

  let (_installed, root) = fresh_root();
  let _ = mount(|| {
    skene_gui::Column::new()
      .add_child(Painted::new(|_, _| {}).set_width(40))
      .build()
  });

  next_frame().await;
  next_frame().await;

  let painted = root
    .first_element_child()
    .and_then(|column| column.first_element_child())
    .expect("the painted box");

  assert!(painted.get_bounding_client_rect().height().abs() < 0.5);
}
