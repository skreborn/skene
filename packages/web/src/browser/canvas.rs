//! Painted boxes in the browser: a canvas placed over its whole box and out of its flow,
//! sized in device pixels as the engine reports them, painted on the GPU where the page
//! has one and on the CPU otherwise.

use std::rc::Rc;

use skene_foundation::{
  NodeId,
  Size,
  web::{Canvas, Painter, Painting, kurbo::Affine},
};
use skene_reactive::ReadSignal;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
  CanvasRenderingContext2d,
  Document,
  HtmlCanvasElement,
  ImageData,
  ResizeObserver,
  ResizeObserverBoxOptions,
  ResizeObserverEntry,
  ResizeObserverOptions,
  ResizeObserverSize,
};

use super::{Dom, Web, events, events::Listener, media, try_with_dom, with_dom};
use crate::gpu;

/// How a canvas is placed: over its whole box, out of the box's flow.
const CANVAS_STYLE: &str = "position:absolute;inset:0;width:100%;height:100%;display:block";

/// A painted box's canvas: the painter, the element and its last measured size, the GPU
/// target, and which path paints it.
pub(super) struct PaintedCanvas {
  /// The painter the box draws with.
  painter: Rc<Painter>,
  /// The `<canvas>` painted into.
  pub(super) element: HtmlCanvasElement,
  /// The size last measured; `None` before the first measure.
  size: Option<CanvasSize>,
  /// The GPU target painted into, once made.
  gpu: Option<gpu::Target>,
  /// Whether it waits on the answer to the ask for a GPU.
  awaiting_gpu: bool,
  /// Whether it paints on the CPU alone, its GPU path having failed.
  on_cpu: bool,
}

/// A canvas's size: in CSS pixels, which its painter draws in, and in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CanvasSize {
  css: Size<f32>,
  device: (u16, u16),
}

impl CanvasSize {
  /// `css` in device pixels at the window's device pixel ratio.
  fn at_ratio(css: Size<f32>) -> Self {
    #[expect(
      clippy::cast_possible_truncation,
      reason = "a device pixel ratio is precise enough in f32"
    )]
    let ratio = web_sys::window().map_or(1., |window| window.device_pixel_ratio()) as f32;
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "a box's size in device pixels is non-negative and far below a canvas's limit"
    )]
    let device = (
      (css.width * ratio).round() as u16,
      (css.height * ratio).round() as u16,
    );

    Self { css, device }
  }

  /// Whether nothing would show at this size.
  fn is_empty(&self) -> bool {
    self.css.width <= 0. || self.css.height <= 0. || self.device.0 == 0 || self.device.1 == 0
  }
}

/// The observer reporting each canvas's size, whether the engine reports it in device
/// pixels, and what follows the device pixel ratio where it does not.
pub(super) struct Pixels {
  observer: ResizeObserver,
  device: bool,
  _callback: Closure<dyn FnMut(js_sys::Array)>,
  ratio: Option<Listener>,
}

impl Pixels {
  fn new() -> Self {
    let device = reports_device_pixels();
    let callback = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
      let measured: Vec<(NodeId, CanvasSize)> = entries
        .iter()
        .filter_map(|entry| {
          let entry: ResizeObserverEntry = entry.unchecked_into();
          let sink = entry
            .target()
            .parent_element()
            .and_then(|parent| events::node_of(&parent))?;
          let rect = entry.content_rect();
          #[expect(
            clippy::cast_possible_truncation,
            reason = "a box's size in CSS pixels is precise enough in f32"
          )]
          let css = Size::new(rect.width() as f32, rect.height() as f32);
          let size = device.then(|| device_size(&entry)).flatten().map_or_else(
            || CanvasSize::at_ratio(css),
            |device| CanvasSize { css, device },
          );

          Some((sink, size))
        })
        .collect();

      for (sink, size) in measured {
        try_with_dom(|dom| {
          if let Some(canvas) = dom.canvas_mut(sink) {
            canvas.size = Some(size);
          }
        });
        repaint(sink);
      }
    });
    let observer =
      ResizeObserver::new(callback.as_ref().unchecked_ref()).expect("a resize observer");

    Self {
      observer,
      device,
      _callback: callback,
      ratio: (!device).then(watch_ratio).flatten(),
    }
  }

  /// Observes `canvas`, in device pixels where the engine reports them.
  fn observe(&self, canvas: &HtmlCanvasElement) {
    if self.device {
      let options = ResizeObserverOptions::new();

      options.set_box(ResizeObserverBoxOptions::DevicePixelContentBox);
      self.observer.observe_with_options(canvas, &options);
    }
    else {
      self.observer.observe(canvas);
    }
  }

  /// Stops observing `canvas`.
  pub(super) fn unobserve(&self, canvas: &HtmlCanvasElement) {
    self.observer.unobserve(canvas);
  }
}

/// Whether the engine reports a box's size in device pixels.
fn reports_device_pixels() -> bool {
  let global = js_sys::global();

  js_sys::Reflect::get(&global, &JsValue::from_str("ResizeObserverEntry"))
    .and_then(|entry| js_sys::Reflect::get(&entry, &JsValue::from_str("prototype")))
    .and_then(|prototype| {
      js_sys::Reflect::has(&prototype, &JsValue::from_str("devicePixelContentBoxSize"))
    })
    .unwrap_or(false)
}

/// The device-pixel size `entry` reports, when it reports one.
fn device_size(entry: &ResizeObserverEntry) -> Option<(u16, u16)> {
  let size: ResizeObserverSize = entry
    .device_pixel_content_box_size()
    .get(0)
    .dyn_into()
    .ok()?;

  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a device-pixel size is non-negative and far below a canvas's limit"
  )]
  Some((
    size.inline_size().round() as u16,
    size.block_size().round() as u16,
  ))
}

/// Repaints every painted box when the device pixel ratio changes, for an engine that
/// reports no device pixels; `None` where the window answers no media query.
fn watch_ratio() -> Option<Listener> {
  let window = web_sys::window()?;
  let query = window
    .match_media(&format!(
      "(resolution: {}dppx)",
      window.device_pixel_ratio()
    ))
    .ok()??;

  Some(Listener::new(query.into(), "change", false, |_| {
    let painted = painted_sinks(|canvas| {
      canvas.size = canvas.size.map(|size| CanvasSize::at_ratio(size.css));

      true
    });

    for sink in painted {
      repaint(sink);
    }

    // The query stands for the ratio it was made at; the next one is made once this
    // listener has returned.
    wasm_bindgen_futures::spawn_local(async {
      let ratio = watch_ratio();

      try_with_dom(|dom| {
        if let Some(pixels) = &mut dom.pixels {
          pixels.ratio = ratio;
        }
      });
    });
  }))
}

/// A canvas placed over its box.
fn new_canvas(document: &Document) -> Option<HtmlCanvasElement> {
  let canvas: HtmlCanvasElement = document.create_element("canvas").ok()?.dyn_into().ok()?;

  let _ = canvas.set_attribute("style", CANVAS_STYLE);

  Some(canvas)
}

impl Dom {
  /// The observer reporting each canvas's size, made on first use.
  fn pixels(&mut self) -> &Pixels {
    self.pixels.get_or_insert_with(Pixels::new)
  }

  /// The canvas of the painted box `sink`.
  fn canvas_mut(&mut self, sink: NodeId) -> Option<&mut PaintedCanvas> {
    self.node_mut(sink)?.extra.canvas.as_deref_mut()
  }

  /// Gives the box `sink` a canvas over it painted by `painter`, observed, or hands its
  /// canvas `painter`; the box contains the canvas's layout, so the canvas never sizes
  /// it.
  fn place_canvas(&mut self, sink: NodeId, painter: Rc<Painter>) {
    if let Some(canvas) = self.canvas_mut(sink) {
      canvas.painter = painter;

      return;
    }

    let Some(element) = new_canvas(self.document())
    else {
      return;
    };

    self.pixels().observe(&element);

    let Some(node) = self.node_mut(sink)
    else {
      return;
    };
    let Some(html) = node.html()
    else {
      return;
    };

    let _ = html.style().set_property("contain", "layout");
    let _ = html.append_child(&element);

    let rect = element.get_bounding_client_rect();
    #[expect(
      clippy::cast_possible_truncation,
      reason = "a box's size in CSS pixels is precise enough in f32"
    )]
    let css = Size::new(rect.width() as f32, rect.height() as f32);

    node.extra.canvas = Some(Box::new(PaintedCanvas {
      painter,
      element,
      size: Some(CanvasSize::at_ratio(css)),
      gpu: None,
      awaiting_gpu: false,
      on_cpu: false,
    }));
  }

  /// Puts a fresh canvas element in place of the box `sink`'s, whose context is spoken
  /// for, and forgets the GPU target the old one held.
  fn replace_canvas(&mut self, sink: NodeId) {
    let Some(fresh) = new_canvas(self.document())
    else {
      return;
    };

    self.pixels().observe(&fresh);

    let Some(canvas) = self.canvas_mut(sink)
    else {
      return;
    };
    let old = std::mem::replace(&mut canvas.element, fresh.clone());

    canvas.gpu = None;

    let _ = old.replace_with_with_node_1(&fresh);

    if let Some(pixels) = &self.pixels {
      pixels.unobserve(&old);
    }
  }
}

impl Painting for Web {
  fn available() -> ReadSignal<bool> {
    media::live()
  }

  fn paint(sink: NodeId, painter: Box<Painter>) {
    with_dom(|dom| dom.place_canvas(sink, Rc::from(painter)));
    repaint(sink);
  }
}

/// Runs the painter of `sink` at its canvas's size: on the GPU where the page has one
/// and the canvas has not fallen back, else on the CPU.
fn repaint(sink: NodeId) {
  let Some((painter, size, on_cpu)) = try_with_dom(|dom| {
    let canvas = dom.canvas_mut(sink)?;

    Some((Rc::clone(&canvas.painter), canvas.size?, canvas.on_cpu))
  })
  .flatten()
  else {
    return;
  };

  if size.is_empty() {
    return;
  }

  let (width, height) = size.device;
  let mut surface = Canvas::new(width, height);

  surface.set_transform(Affine::scale_non_uniform(
    f64::from(width) / f64::from(size.css.width),
    f64::from(height) / f64::from(size.css.height),
  ));
  painter(&mut surface, size.css);

  if !on_cpu {
    match gpu::renderer(repaint_awaiting, lost) {
      gpu::Availability::Pending => {
        with_dom(|dom| {
          if let Some(canvas) = dom.canvas_mut(sink) {
            canvas.awaiting_gpu = true;
          }
        });

        return;
      }
      gpu::Availability::Ready(context) => {
        let rendered = with_dom(|dom| {
          let canvas = dom.canvas_mut(sink)?;

          if canvas.gpu.is_none() {
            canvas.gpu = Some(context.target(&canvas.element, width, height)?);
          }

          Some(context.render(canvas.gpu.as_mut()?, &surface))
        });

        if rendered == Some(true) {
          return;
        }

        with_dom(|dom| {
          if let Some(canvas) = dom.canvas_mut(sink) {
            canvas.on_cpu = true;
          }

          dom.replace_canvas(sink);
        });
      }
      gpu::Availability::Unavailable => {}
    }
  }

  blit(sink, &surface);
}

/// Copies `surface` into the canvas of `sink` through its 2D context, in a fresh canvas
/// where a context of another kind holds the old one.
fn blit(sink: NodeId, surface: &Canvas) {
  let (width, height) = surface.size();

  for _ in 0..2 {
    let Some(canvas) = with_dom(|dom| dom.canvas_mut(sink).map(|canvas| canvas.element.clone()))
    else {
      return;
    };

    canvas.set_width(u32::from(width));
    canvas.set_height(u32::from(height));

    if let Ok(Some(context)) = canvas.get_context("2d")
      && let Ok(context) = context.dyn_into::<CanvasRenderingContext2d>()
    {
      let pixels = surface.pixels();

      if let Ok(image) = ImageData::new_with_u8_clamped_array_and_sh(
        wasm_bindgen::Clamped(&pixels),
        u32::from(width),
        u32::from(height),
      ) {
        let _ = context.put_image_data(&image, 0., 0.);
      }

      return;
    }

    with_dom(|dom| dom.replace_canvas(sink));
  }
}

/// Repaints every box that waited on the GPU's answer.
fn repaint_awaiting() {
  let awaiting = painted_sinks(|canvas| std::mem::take(&mut canvas.awaiting_gpu));

  for sink in awaiting {
    repaint(sink);
  }
}

/// Forgets every canvas's GPU target once the device is lost, and repaints every box
/// through the renderer asked for afresh.
fn lost() {
  let painted = painted_sinks(|canvas| {
    canvas.gpu = None;

    true
  });

  for sink in painted {
    repaint(sink);
  }
}

/// The painted boxes whose canvas `select` picks, given each canvas to act on.
fn painted_sinks(mut select: impl FnMut(&mut PaintedCanvas) -> bool) -> Vec<NodeId> {
  try_with_dom(|dom| {
    dom
      .realizer
      .records
      .iter_mut()
      .filter_map(|(sink, node)| {
        let canvas = node.extra.canvas.as_deref_mut()?;

        select(canvas).then_some(sink)
      })
      .collect()
  })
  .unwrap_or_default()
}
