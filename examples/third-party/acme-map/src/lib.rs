//! # acme-map
//!
//! A third-party low-level view covering every platform of the web medium through a
//! library-local [`MapHost`] trait: on the web a `Map` paints into a `<canvas>` it
//! creates through `WebDom` once the page runs script, a box of the sea's color
//! standing in its place through `WebMarkup` before then and on the server. Every form
//! realizes for a node `LowLevel` mints inside a `Container` that sizes it, so it
//! composes with wrappers, themes and events as a built-in does, and an app names no
//! platform.

use std::marker::PhantomData;

use skene_foundation::{
  AnyKind,
  Container,
  DecorationBase,
  HasChild,
  IntoChildOf,
  NodeId,
  PlainChild,
  Realization,
  Size,
  View,
};

/// The sea the map paints, as CSS.
const SEA: &str = "oklch(0.72 0.1 230)";

/// A map, sized by the container around it.
#[derive(Debug)]
#[must_use]
pub struct Map<P: Realization> {
  size: Size<f32>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization> Default for Map<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Map<P> {
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

  /// The container of the map's size holding `node`.
  fn sized(&self, node: impl IntoChildOf<P, PlainChild>) -> NodeId
  where
    P: Realization<Scalar = f32>,
  {
    Container::<P>::new()
      .set_width(self.size.width)
      .set_height(self.size.height)
      .set_child(node)
      .build()
  }
}

/// A platform a [`Map`] realizes on: every platform of the web medium.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is no platform a map realizes on",
  label = "a map covers the web medium's platforms"
)]
pub trait MapHost: Realization<Scalar = f32> {
  /// Realizes `map`, returning its node.
  fn build_map(map: Map<Self>) -> NodeId;
}

impl<P: Realization> AnyKind for Map<P> {}

impl<P: MapHost> View<P> for Map<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    P::build_map(self)
  }
}

impl MapHost for skene_web::platform::Web {
  /// The canvas once the page runs script, the sea's box before then and on the
  /// server.
  fn build_map(map: Map<Self>) -> NodeId {
    use skene_foundation::Enhance;

    let size = map.size;
    let live = Self::hydration().expect("the web answers whether the page runs script");

    Enhance::<Self>::new(live)
      .set_fallback_builder(move || Map::<Self>::new().set_size(size).surface("div", |_| {}))
      .set_child_builder(move || Map::<Self>::new().set_size(size).canvas())
      .build()
  }
}

impl Map<skene_web::platform::Web> {
  /// A node realized as an element of `tag` dressed as the map's surface, then handed
  /// to `draw`, inside a container of the map's size.
  fn surface(self, tag: &'static str, draw: impl FnOnce(NodeId)) -> NodeId {
    use skene_foundation::{LowLevel, NodeKind};
    use skene_web::platform::{Declarations, Web, WebMarkup};

    let node = LowLevel::<Web>::realize(NodeKind::Box, |node| {
      Web::create_element(node, tag);
      Web::set_attribute(node, "role", "img");
      Web::set_attribute(node, "aria-label", "map");
      Web::set_declarations(
        node,
        &Declarations::new()
          .set("display", "block")
          .set("width", "100%")
          .set("height", "100%")
          .set("background-color", SEA),
      );
      draw(node);
    });

    self.sized(skene_foundation::AnyView::<Web>::node(node))
  }

  /// The canvas the map paints.
  #[cfg(all(skene_platform = "web", not(skene_server)))]
  fn canvas(self) -> NodeId {
    use skene_web::platform::{Web, WebDom, WebMarkup};
    use wasm_bindgen::JsCast;

    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "a map's size is a whole, non-negative number of pixels well within u32"
    )]
    let (width, height) = (self.size.width as u32, self.size.height as u32);

    self.surface("canvas", |node| {
      Web::set_attribute(node, "width", &width.to_string());
      Web::set_attribute(node, "height", &height.to_string());

      if let Some(canvas) =
        Web::element(node).and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
      {
        paint(&canvas, width, height);
      }
    })
  }

  /// The sea's box: a server render never builds the canvas.
  #[cfg(all(skene_platform = "web", skene_server))]
  fn canvas(self) -> NodeId {
    self.surface("div", |_| {})
  }
}

/// Paints the map: land over the sea, a road across it.
#[cfg(all(skene_platform = "web", not(skene_server)))]
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
