//! A box the app draws: a vello scene on the CPU, in device pixels, redrawn whenever what its
//! closure read changes or the box is laid out at another size.

use std::rc::Rc;

use skene_foundation::{
  Color, Dimension, Signal,
  seam::{
    access::Role,
    content::{Pixels, contents},
    node::{NodeKind, NodeSpec},
    record::Record,
    report::Size,
    tree,
  },
};
/// The geometry a [`Canvas`] draws with (vello's `kurbo`).
pub use vello_cpu::kurbo as shapes;
/// The paints a [`Canvas`] draws with (vello's `peniko`).
pub use vello_cpu::peniko as paints;
use vello_cpu::{Pixmap, RenderContext, Resources, kurbo};

use crate::{
  Active,
  access::Access,
  view::{Cx, Node, NodeHandle, View, finish, handle, properties},
};

/// What a [`Painted`] box's closure draws on: a vello scene the size of the box, in pixels
/// (scaled to the device's pixels), its origin the box's top leading corner.
pub struct Canvas<'a> {
  context: &'a mut RenderContext,
  width: f32,
  height: f32,
  scale: f64,
}

impl Canvas<'_> {
  /// The box's width, in pixels.
  #[must_use]
  pub const fn width(&self) -> f32 {
    self.width
  }

  /// The box's height, in pixels.
  #[must_use]
  pub const fn height(&self) -> f32 {
    self.height
  }

  /// Device pixels per pixel.
  #[must_use]
  pub const fn scale(&self) -> f64 {
    self.scale
  }

  /// Fills `shape` with `color`.
  pub fn fill(&mut self, shape: &impl kurbo::Shape, color: Color) {
    self.context.set_paint(paint(color));
    self.context.fill_path(&shape.to_path(0.1));
  }

  /// Strokes `shape`'s outline `width` pixels wide with `color`.
  pub fn stroke(&mut self, shape: &impl kurbo::Shape, width: f64, color: Color) {
    self.context.set_paint(paint(color));
    self.context.set_stroke(kurbo::Stroke::new(width));
    self.context.stroke_path(&shape.to_path(0.1));
  }

  /// The whole of vello's CPU renderer, drawing in pixels: layers, gradients, images.
  pub const fn context(&mut self) -> &mut RenderContext {
    self.context
  }
}

const fn paint(color: Color) -> vello_cpu::color::AlphaColor<vello_cpu::color::Srgb> {
  vello_cpu::color::AlphaColor::from_rgba8(color.red, color.green, color.blue, color.alpha)
}

/// Renders `paint` at `size` into device pixels.
fn render(size: Size, paint: &dyn Fn(&mut Canvas<'_>)) -> Pixels {
  let device = |length: f32| {
    (f64::from(length) * size.scale)
      .ceil()
      .clamp(0., f64::from(u16::MAX)) as u16
  };
  let (width, height) = (device(size.width), device(size.height));

  if width == 0 || height == 0 {
    return Pixels {
      width: 0,
      height: 0,
      scale: size.scale,
      data: Rc::from([].as_slice()),
    };
  }

  let mut context = RenderContext::new(width, height);

  context.set_transform(kurbo::Affine::scale(size.scale));

  paint(&mut Canvas {
    context: &mut context,
    width: size.width,
    height: size.height,
    scale: size.scale,
  });

  context.flush();

  let mut pixmap = Pixmap::new(width, height);

  context.render(&mut pixmap, &mut Resources::new());

  Pixels {
    width: u32::from(width),
    height: u32::from(height),
    scale: size.scale,
    data: Rc::from(pixmap.data_as_u8_slice()),
  }
}

/// A box the app draws on a [`Canvas`]: vello's CPU renderer, in device pixels, composited by
/// the platform into the window's frame.
///
/// The closure runs again whenever a signal it reads changes and whenever the box is laid out
/// at another size or scale. The box is as large as its sizing makes it: give it a width and a
/// height, or a weight in its layout. Without a description it is decoration a reader skips.
///
/// ```
/// use skene_gui::{painted::shapes::Circle, prelude::*};
///
/// let level = Signal::new(0.5_f32);
///
/// Painted::new(move |canvas| {
///   let radius = f64::from(canvas.height() / 2.);
///
///   canvas.fill(&Circle::new((radius, radius), radius), Color::rgb(0x34c759));
///   canvas.fill(
///     &Circle::new((radius, radius), radius * f64::from(level.get())),
///     Color::WHITE,
///   );
/// })
/// .set_width(48)
/// .set_height(48)
/// .set_description("Level");
/// ```
#[must_use]
pub struct Painted {
  paint: Rc<dyn Fn(&mut Canvas<'_>)>,
  record: Record,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Painted {
  /// A box drawn by `paint`.
  pub fn new(paint: impl Fn(&mut Canvas<'_>) + 'static) -> Self {
    Self {
      paint: Rc::new(paint),
      record: Record::new(),
      handle: None,
      access: Access::default(),
    }
  }

  properties! {
    /// The width.
    set_width, derive_width: WIDTH, Dimension;
    /// The height.
    set_height, derive_height: HEIGHT, Dimension;
    /// The least width.
    set_min_width, derive_min_width: MIN_WIDTH, Dimension;
    /// The least height.
    set_min_height, derive_min_height: MIN_HEIGHT, Dimension;
    /// The greatest width.
    set_max_width, derive_max_width: MAX_WIDTH, Dimension;
    /// The greatest height.
    set_max_height, derive_max_height: MAX_HEIGHT, Dimension;
  }

  handle!();

  /// Describes the picture for a reader.
  pub fn set_description(self, description: impl Into<Rc<str>>) -> Self {
    let description = description.into();

    self.derive_description(move || Rc::clone(&description))
  }

  /// Describes the picture for a reader, kept current.
  pub fn derive_description<S: Into<Rc<str>>>(
    mut self,
    description: impl Fn() -> S + 'static,
  ) -> Self {
    self.access.name = Some(Rc::new(move || description().into()));

    self
  }
}

impl View for Painted {
  fn build(self, cx: &mut Cx) -> Node {
    let described = self.access.name.is_some();
    let mut spec = NodeSpec::new(NodeKind::Painted);

    if !described {
      spec = spec.with_role(Role::Presentation);
    }

    let id = tree::create::<Active>(spec, &cx.scope);
    let size = Signal::new(None::<Size>);
    let paint = self.paint;

    finish(id, self.record, cx, self.handle.as_ref());
    tree::handle_size::<Active>(id, move |laid_out| size.set(Some(laid_out)));
    tree::bind::<Active, _>(id, contents::PIXELS, move || {
      render(size.get().unwrap_or_default(), &*paint)
    });

    self.access.apply(id);

    Node(id)
  }
}
