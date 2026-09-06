//! The portable low-level path for pixel media: a paint callback on a box, drawing
//! through vello.

use skene_core::{Color, Driver, Size};
use vello_cpu::{Pixmap, RenderContext, Resources};
pub use vello_cpu::{kurbo, peniko};

/// The surface a paint callback draws on: a vello scene the size of the box, in device
/// pixels, rendered by the driver once the callback returns.
///
/// Geometry is [`kurbo`]'s and paint is [`peniko`]'s; the framework's [`Color`] paints
/// solid through [`IntoBrush`]. Coordinates are device pixels from the box's top-left corner.
#[must_use]
pub struct Canvas {
  context: RenderContext,
  resources: Resources,
  width: u16,
  height: u16,
}

impl Canvas {
  /// A blank canvas of `width` by `height` device pixels.
  pub fn new(width: u16, height: u16) -> Self {
    Self {
      context: RenderContext::new(width.max(1), height.max(1)),
      resources: Resources::new(),
      width: width.max(1),
      height: height.max(1),
    }
  }

  /// The canvas size, in device pixels.
  #[inline(always)]
  #[must_use]
  pub const fn size(&self) -> (u16, u16) {
    (self.width, self.height)
  }

  /// Sets the transform every later shape is drawn under.
  #[inline(always)]
  pub fn set_transform(&mut self, transform: kurbo::Affine) {
    self.context.set_transform(transform);
  }

  /// Fills `shape` with `brush`.
  pub fn fill(&mut self, shape: &impl kurbo::Shape, brush: impl IntoBrush) {
    self.context.set_paint(brush_paint(brush.into_brush()));
    self.context.fill_path(&shape.to_path(0.1));
  }

  /// Strokes `shape` with `brush` along `stroke`.
  pub fn stroke(
    &mut self,
    shape: &impl kurbo::Shape,
    stroke: &kurbo::Stroke,
    brush: impl IntoBrush,
  ) {
    self.context.set_stroke(stroke.clone());
    self.context.set_paint(brush_paint(brush.into_brush()));
    self.context.stroke_path(&shape.to_path(0.1));
  }

  /// Clips every later shape to `shape` until the matching [`pop`](Self::pop).
  pub fn push_clip(&mut self, shape: &impl kurbo::Shape) {
    self.context.push_clip_layer(&shape.to_path(0.1));
  }

  /// Ends the innermost [`push_clip`](Self::push_clip).
  #[inline(always)]
  pub fn pop(&mut self) {
    self.context.pop_layer();
  }

  /// Renders the scene: the pixels as unpremultiplied RGBA8, rows top to bottom.
  #[must_use]
  pub fn pixels(&mut self) -> Vec<u8> {
    let mut pixmap = Pixmap::new(self.width, self.height);

    self.context.flush();
    self.context.render(&mut pixmap, &mut self.resources);

    pixmap
      .take_unpremultiplied()
      .into_iter()
      .flat_map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
      .collect()
  }
}

/// A peniko brush as the paint `vello_cpu` takes.
fn brush_paint(brush: peniko::Brush) -> vello_cpu::PaintType {
  match brush {
    peniko::Brush::Solid(color) => vello_cpu::PaintType::Solid(color),
    peniko::Brush::Gradient(gradient) => vello_cpu::PaintType::Gradient(gradient),
    peniko::Brush::Image(_) => vello_cpu::PaintType::Solid(peniko::Color::TRANSPARENT),
  }
}

/// What a shape is painted with: a brush, or anything that is one.
///
/// The framework's [`Color`] paints solid; peniko's brush, color and gradient pass
/// through.
pub trait IntoBrush {
  /// The brush.
  fn into_brush(self) -> peniko::Brush;
}

impl IntoBrush for peniko::Brush {
  #[inline(always)]
  fn into_brush(self) -> peniko::Brush {
    self
  }
}

impl IntoBrush for peniko::Color {
  #[inline(always)]
  fn into_brush(self) -> peniko::Brush {
    peniko::Brush::Solid(self)
  }
}

impl IntoBrush for peniko::Gradient {
  #[inline(always)]
  fn into_brush(self) -> peniko::Brush {
    peniko::Brush::Gradient(self)
  }
}

impl IntoBrush for Color {
  #[inline(always)]
  fn into_brush(self) -> peniko::Brush {
    peniko::Brush::Solid(self.to_alpha_color())
  }
}

/// A paint callback: draws into the canvas at the node's laid-out size, in the
/// medium's logical units; the canvas is scaled so one unit is one logical pixel.
pub type Painter = dyn Fn(&mut Canvas, Size<f32>);

/// A driver that paints a callback into a box.
///
/// The bound a portable low-level view states for pixel media: the view builds an
/// ordinary box for its sizing and hands the driver a painter for it, which the driver
/// runs whenever the box repaints. The terminal does not implement it, so the view does
/// not exist there; the server leaf implements it as nothing, so a page renders the box
/// empty.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not paint",
  label = "a painted view is written against a pixel driver implementing `Painting`; the terminal does not"
)]
pub trait Painting: Driver {
  /// Paints `painter` into the box `sink` at its laid-out size, now and whenever the
  /// box repaints; a later call replaces the painter.
  fn paint(&self, sink: &Self::Sink, painter: Box<Painter>);
}
