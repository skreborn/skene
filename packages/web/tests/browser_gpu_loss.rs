//! A painted box whose GPU device is lost, in a real browser with WebGPU on.
//!
//! The web leaf keeps the answer to its ask for a GPU for the page's life, so this
//! binary loses the device alone: with WebGPU gone by then, the box paints on the CPU
//! in a fresh canvas, the old one holding a WebGPU context.
//!
//! Run in a headless browser:
//! `cargo test -p skene-web --target wasm32-unknown-unknown --test browser_gpu_loss`.
#![cfg(skene_web_platform = "browser")]

use skene_foundation::{Dimension, PaintedBase};
use skene_web::{
  Color,
  Painted,
  View,
  kurbo::Rect,
  platform::mount,
  testing::{frames_until, fresh_root},
  wasm_bindgen::{JsCast, JsValue},
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement};

wasm_bindgen_test_configure!(run_in_browser);

/// The canvas the painted box at the start of `root` draws into.
fn canvas_of(root: &Element) -> Option<HtmlCanvasElement> {
  root
    .first_element_child()?
    .first_element_child()?
    .dyn_into()
    .ok()
}

/// Calls the method `name` of `target` with no arguments.
fn call(target: &JsValue, name: &str) -> JsValue {
  js_sys::Reflect::get(target, &JsValue::from_str(name))
    .expect("the method")
    .unchecked_into::<js_sys::Function>()
    .call0(target)
    .expect("the call")
}

/// Shadows `navigator.gpu` with `undefined`, so an ask for a GPU finds none.
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
}

/// A painted box whose device is lost paints again, on the CPU in a fresh canvas where
/// WebGPU is gone by then.
#[wasm_bindgen_test]
async fn a_lost_device_falls_back_to_the_cpu_in_a_fresh_canvas() {
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

  let configured = || {
    canvas_of(&root)
      .and_then(|canvas| canvas.get_context("webgpu").ok().flatten())
      .map(|context| call(&context, "getConfiguration"))
      .is_some_and(|configuration| !configuration.is_null() && !configuration.is_undefined())
  };

  assert!(frames_until(configured).await, "the box painted on the GPU");

  let gpu_canvas = canvas_of(&root).expect("the canvas");
  let context = gpu_canvas
    .get_context("webgpu")
    .expect("a context")
    .expect("the WebGPU context");
  let device = js_sys::Reflect::get(
    &call(&context, "getConfiguration"),
    &JsValue::from_str("device"),
  )
  .expect("the device");

  hide_webgpu();
  call(&device, "destroy");

  let blitted = || {
    canvas_of(&root)
      .filter(|canvas| *canvas != gpu_canvas)
      .and_then(|canvas| {
        let context: CanvasRenderingContext2d = canvas.get_context("2d").ok()??.dyn_into().ok()?;
        let pixel = context
          .get_image_data(
            f64::from(canvas.width() / 2),
            f64::from(canvas.height() / 2),
            1.,
            1.,
          )
          .ok()?
          .data()
          .to_vec();

        Some(pixel[0..4] == [0, 0, 255, 255])
      })
      .unwrap_or(false)
  };

  assert!(
    frames_until(blitted).await,
    "a fresh canvas took the lost one's place and was blitted"
  );
  assert!(!gpu_canvas.is_connected());
}
