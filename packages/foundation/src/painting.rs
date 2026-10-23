//! The portable low-level path for pixels: a paint callback on a box, drawing through
//! vello, realized by every platform that can show pixels.

use skene_reactive::ReadSignal;
pub use vello_cpu::{PaintType, kurbo, peniko};
use vello_cpu::{Pixmap, RenderContext, Resources};

use crate::{Color, NodeId, Realization, Size};

/// The surface a paint callback draws on: a vello scene the size of the box, in device
/// pixels, recorded once and replayed into the platform's renderer once the callback
/// returns.
///
/// Geometry is [`kurbo`]'s and paint is [`peniko`]'s; the framework's [`Color`] paints
/// solid through [`IntoBrush`]. Coordinates are device pixels from the box's top-left corner.
#[must_use]
pub struct Canvas {
  commands: Vec<Command>,
  width: u16,
  height: u16,
}

impl std::fmt::Debug for Canvas {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Canvas").finish_non_exhaustive()
  }
}

/// One recorded drawing step.
enum Command {
  Transform(kurbo::Affine),
  Fill(kurbo::BezPath, peniko::Brush),
  Stroke(kurbo::BezPath, kurbo::Stroke, peniko::Brush),
  PushClip(kurbo::BezPath),
  Pop,
}

impl Canvas {
  /// A blank canvas of `width` by `height` device pixels.
  pub const fn new(width: u16, height: u16) -> Self {
    Self {
      commands: Vec::new(),
      width: if width == 0 { 1 } else { width },
      height: if height == 0 { 1 } else { height },
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
    self.commands.push(Command::Transform(transform));
  }

  /// Fills `shape` with `brush`.
  pub fn fill(&mut self, shape: &impl kurbo::Shape, brush: impl IntoBrush) {
    self
      .commands
      .push(Command::Fill(shape.to_path(0.1), brush.into_brush()));
  }

  /// Strokes `shape` with `brush` along `stroke`.
  pub fn stroke(
    &mut self,
    shape: &impl kurbo::Shape,
    stroke: &kurbo::Stroke,
    brush: impl IntoBrush,
  ) {
    self.commands.push(Command::Stroke(
      shape.to_path(0.1),
      stroke.clone(),
      brush.into_brush(),
    ));
  }

  /// Clips every later shape to `shape` until the matching [`pop`](Self::pop).
  pub fn push_clip(&mut self, shape: &impl kurbo::Shape) {
    self.commands.push(Command::PushClip(shape.to_path(0.1)));
  }

  /// Ends the innermost [`push_clip`](Self::push_clip).
  #[inline(always)]
  pub fn pop(&mut self) {
    self.commands.push(Command::Pop);
  }

  /// Replays the recorded scene into `renderer`, in order.
  pub fn replay(&self, renderer: &mut impl Renderer) {
    for command in &self.commands {
      match command {
        Command::Transform(transform) => renderer.set_transform(*transform),
        Command::Fill(path, brush) => renderer.fill(path, brush),
        Command::Stroke(path, stroke, brush) => renderer.stroke(path, stroke, brush),
        Command::PushClip(path) => renderer.push_clip(path),
        Command::Pop => renderer.pop(),
      }
    }
  }

  /// Renders the scene on the CPU: the pixels as unpremultiplied RGBA8, rows top to
  /// bottom.
  #[must_use]
  pub fn pixels(&self) -> Vec<u8> {
    let mut context = RenderContext::new(self.width, self.height);
    let mut resources = Resources::new();
    let mut pixmap = Pixmap::new(self.width, self.height);

    self.replay(&mut context);
    context.flush();
    context.render(&mut pixmap, &mut resources);

    pixmap.take_rgba8(peniko::ImageAlphaType::Alpha)
  }
}

/// A vello scene a [`Canvas`] replays into: the drawing vocabulary every renderer
/// takes, so one recording renders on the CPU, on the GPU, or on a platform layer.
///
/// [`vello_cpu`]'s context implements it here; a platform implements it for the renderer
/// it owns, mapping each brush through [`paint`].
pub trait Renderer {
  /// Sets the transform every later shape is drawn under.
  fn set_transform(&mut self, transform: kurbo::Affine);
  /// Fills `path` with `brush`.
  fn fill(&mut self, path: &kurbo::BezPath, brush: &peniko::Brush);
  /// Strokes `path` with `brush` along `stroke`.
  fn stroke(&mut self, path: &kurbo::BezPath, stroke: &kurbo::Stroke, brush: &peniko::Brush);
  /// Clips every later shape to `path` until the matching [`pop`](Self::pop).
  fn push_clip(&mut self, path: &kurbo::BezPath);
  /// Ends the innermost [`push_clip`](Self::push_clip).
  fn pop(&mut self);
}

impl Renderer for RenderContext {
  #[inline(always)]
  fn set_transform(&mut self, transform: kurbo::Affine) {
    Self::set_transform(self, transform);
  }

  fn fill(&mut self, path: &kurbo::BezPath, brush: &peniko::Brush) {
    self.set_paint(paint(brush));
    self.fill_path(path);
  }

  fn stroke(&mut self, path: &kurbo::BezPath, stroke: &kurbo::Stroke, brush: &peniko::Brush) {
    self.set_stroke(stroke.clone());
    self.set_paint(paint(brush));
    self.stroke_path(path);
  }

  #[inline(always)]
  fn push_clip(&mut self, path: &kurbo::BezPath) {
    self.push_clip_layer(path);
  }

  #[inline(always)]
  fn pop(&mut self) {
    self.pop_layer();
  }
}

/// A peniko brush as the paint a vello renderer takes; an image brush paints nothing.
#[must_use]
pub fn paint(brush: &peniko::Brush) -> PaintType {
  match brush {
    peniko::Brush::Solid(color) => PaintType::Solid(*color),
    peniko::Brush::Gradient(gradient) => PaintType::Gradient(gradient.clone()),
    peniko::Brush::Image(_) => PaintType::Solid(peniko::Color::TRANSPARENT),
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

/// A platform that paints a callback into a box.
///
/// The bound a portable low-level view states: the view builds an ordinary box for
/// its sizing and hands the platform a painter for it, which it runs whenever the
/// box repaints. The browser leaf renders into a canvas, the headless platform to pixels
/// a test reads, the terminal through its graphics protocol where the terminal proved
/// one, and the server leaf as nothing, so a page renders the box empty.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not paint",
  label = "a painted view is written against a platform implementing `Painting`"
)]
pub trait Painting: Realization {
  /// Paints `painter` into the box `node` at its laid-out size, now and whenever the
  /// box repaints; a later call replaces the painter.
  fn paint(node: NodeId, painter: Box<Painter>);

  /// Whether a painted box shows as pixels, as a signal: false on the server leaf,
  /// on the browser leaf until it is live and in the terminal until its graphics are
  /// proven, true once they are; what
  /// [`Painted::set_fallback_builder`](crate::Painted::set_fallback_builder) waits on.
  fn available() -> ReadSignal<bool>;
}

#[cfg(test)]
mod tests {
  use super::{Canvas, Renderer, kurbo, peniko};
  use crate::Color;

  /// A renderer that logs each step it takes.
  #[derive(Default)]
  struct Log(Vec<String>);

  impl Renderer for Log {
    fn set_transform(&mut self, transform: kurbo::Affine) {
      self
        .0
        .push(format!("transform {}", transform.as_coeffs()[0]));
    }

    fn fill(&mut self, path: &kurbo::BezPath, brush: &peniko::Brush) {
      self.0.push(format!(
        "fill {} {}",
        path.elements().len(),
        matches!(brush, peniko::Brush::Solid(_))
      ));
    }

    fn stroke(&mut self, _: &kurbo::BezPath, stroke: &kurbo::Stroke, _: &peniko::Brush) {
      self.0.push(format!("stroke {}", stroke.width));
    }

    fn push_clip(&mut self, _: &kurbo::BezPath) {
      self.0.push("clip".into());
    }

    fn pop(&mut self) {
      self.0.push("pop".into());
    }
  }

  /// A platform's renderer takes the canvas's steps in the order they were drawn.
  #[test]
  fn a_canvas_replays_its_scene_into_a_renderer() {
    let mut canvas = Canvas::new(4, 4);
    let rect = kurbo::Rect::new(0., 0., 4., 4.);

    canvas.set_transform(kurbo::Affine::scale(2.));
    canvas.push_clip(&rect);
    canvas.fill(&rect, Color::srgb(1., 0., 0.));
    canvas.stroke(&rect, &kurbo::Stroke::new(3.), Color::srgb(0., 1., 0.));
    canvas.pop();

    let mut log = Log::default();

    canvas.replay(&mut log);

    assert_eq!(
      log.0,
      ["transform 2", "clip", "fill 5 true", "stroke 3", "pop"]
    );
  }
}
