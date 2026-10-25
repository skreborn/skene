//! # acme-map
//!
//! A third-party low-level view through the web's native door: a `Map` is a `<canvas>`,
//! a [`NativeView`] inside a `Container` that sizes it, so it composes with wrappers,
//! themes and events as a built-in does. The server renders the canvas's markup in the
//! sea's color as its placeholder, and the browser paints it once the element is in
//! hand: built fresh, or the served placeholder adopted in place while hydrating.

#![cfg(skene_platform = "web")]

use skene_foundation::{AnyKind, NodeId, Size, View};
use skene_web::{
  Active,
  Container,
  ContainerBase,
  DecorationBase,
  HasChild,
  platform::web::NativeView,
};

/// The sea the map paints, as CSS.
const SEA: &str = "oklch(0.72 0.1 230)";

/// A map, sized by the container around it.
#[derive(Debug)]
#[must_use]
pub struct Map {
  size: Size<f32>,
}

impl Default for Map {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Map {
  /// A map 240 by 120 logical pixels.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      size: Size::new(240., 120.),
    }
  }

  /// Sets the map's size, in logical pixels.
  #[inline(always)]
  pub const fn set_size(mut self, size: Size<f32>) -> Self {
    self.size = size;

    self
  }
}

impl AnyKind for Map {}

impl View<Active> for Map {
  fn build(self) -> NodeId {
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "a map's size is a whole, non-negative number of pixels well within u32"
    )]
    let (width, height) = (self.size.width as u32, self.size.height as u32);
    let canvas = NativeView::new("canvas")
      .add_attribute("role", "img")
      .add_attribute("aria-label", "map")
      .add_attribute("width", width.to_string())
      .add_attribute("height", height.to_string())
      .add_attribute(
        "style",
        format!("display:block;width:100%;height:100%;background-color:{SEA}"),
      );
    #[cfg(not(skene_server))]
    let canvas = canvas.handle_adopt(move |_, element| paint(element, width, height));

    Container::new()
      .set_width(self.size.width)
      .set_height(self.size.height)
      .set_child(canvas)
      .build()
  }
}

/// Paints the map into `canvas`: land over the sea, a road across it.
#[cfg(not(skene_server))]
fn paint(canvas: &web_sys::Element, width: u32, height: u32) {
  use wasm_bindgen::JsCast;

  let Some(context) = canvas
    .dyn_ref::<web_sys::HtmlCanvasElement>()
    .and_then(|canvas| canvas.get_context("2d").ok().flatten())
    .and_then(|context| context.dyn_into::<web_sys::CanvasRenderingContext2d>().ok())
  else {
    return;
  };
  let (width, height) = (f64::from(width), f64::from(height));

  context.set_fill_style_str(SEA);
  context.fill_rect(0., 0., width, height);
  context.set_fill_style_str("oklch(0.8 0.12 140)");
  context.fill_rect(width * 0.1, height * 0.2, width * 0.35, height * 0.5);
  context.fill_rect(width * 0.55, height * 0.45, width * 0.35, height * 0.4);
  context.set_stroke_style_str("oklch(0.4 0.02 60)");
  context.set_line_width(3.);
  context.begin_path();
  context.move_to(0., height * 0.75);
  context.line_to(width, height * 0.3);
  context.stroke();
}
