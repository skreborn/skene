//! # acme-map
//!
//! A third-party low-level view written against the web platform's escape traits: a
//! `Map` paints into a `<canvas>` it creates through `WebDom`, so it exists on the
//! browser leaf alone, and its `Placeholder` renders a box through `WebMarkup`, so
//! a server-rendered page carries something in its place. Either registers its element
//! through `LowLevel` and sits inside a `Container` that sizes it, so it composes
//! with wrappers, themes and events as a built-in does.
//!
//! Web-only: on a host target the crate is empty.

#![cfg(any(target_arch = "wasm32", skene_platform = "server"))]

use std::marker::PhantomData;

use skene_common::{Container, LowLevel};
use skene_core::{AnyKind, HasChildren, NodeId, NodeKind, Platform, Size, View};
#[cfg(target_os = "unknown")]
use skene_web::WebDom;
use skene_web::{Declarations, WebMarkup};

/// The sea the map paints, as CSS.
const SEA: &str = "oklch(0.72 0.1 230)";

/// A map: a `<canvas>` the view paints, sized by the container around it.
#[must_use]
pub struct Map<P: Platform> {
  size: Size<f32>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> Default for Map<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Map<P> {
  /// A map 240 by 120 logical pixels.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      size: Size::new(240., 120.),
      platform: PhantomData,
    }
  }

  /// Sets the map's size, in logical pixels.
  #[inline(always)]
  pub const fn set_size(mut self, size: Size<f32>) -> Self {
    self.size = size;

    self
  }

  /// The map as the server leaf renders it: a box of the sea's color where the canvas
  /// will be, carrying the same size.
  #[inline(always)]
  pub const fn placeholder(self) -> Placeholder<P> {
    Placeholder { map: self }
  }

  /// The element `sink` dressed as the map's surface, inside a container of the map's
  /// size.
  fn realize(self, sink: P::Sink) -> NodeId
  where
    P: WebMarkup<Scalar = f32>,
  {
    let platform = P::default();

    platform.set_attribute(&sink, "role", "img");
    platform.set_attribute(&sink, "aria-label", "map");
    platform.set_declarations(
      &sink,
      &Declarations::new()
        .set("display", "block")
        .set("width", "100%")
        .set("height", "100%")
        .set("background-color", SEA),
    );

    let node = LowLevel::<P>::realize(sink, NodeKind::Box);

    Container::<P>::new()
      .set_width(self.size.width)
      .set_height(self.size.height)
      .set_child(node)
      .build()
  }
}

impl<P: Platform> AnyKind for Map<P> {}

#[cfg(target_os = "unknown")]
impl<P: Platform<Scalar = f32> + WebDom> View<P> for Map<P> {
  fn build(self) -> NodeId {
    use wasm_bindgen::JsCast;

    let platform = P::default();
    let sink = platform.create_element("canvas");

    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (width, height) = (self.size.width as u32, self.size.height as u32);

    platform.set_attribute(&sink, "width", &width.to_string());
    platform.set_attribute(&sink, "height", &height.to_string());

    if let Some(canvas) = platform
      .element(&sink)
      .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
    {
      paint(&canvas, width, height);
    }

    self.realize(sink)
  }
}

/// Paints the map: land over the sea, a road across it.
#[cfg(target_os = "unknown")]
fn paint(canvas: &web_sys::HtmlCanvasElement, width: u32, height: u32) {
  use wasm_bindgen::JsCast;

  let Some(context) = canvas
    .get_context("2d")
    .ok()
    .flatten()
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

/// The map as the server leaf renders it: a box in the sea's color, of the map's size.
#[must_use]
pub struct Placeholder<P: Platform> {
  map: Map<P>,
}

impl<P: Platform> AnyKind for Placeholder<P> {}

impl<P: Platform<Scalar = f32> + WebMarkup> View<P> for Placeholder<P> {
  fn build(self) -> NodeId {
    let platform = P::default();
    let sink = platform.create_element("div");

    self.map.realize(sink)
  }
}
