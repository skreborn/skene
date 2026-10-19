//! Painting the realized tree into a frame: each box's shadows, fill, border and ring, its
//! content (a text, a text entry, a recorded picture), its children clipped where it clips,
//! everything in the scene's order through a [`Sink`] (vello_cpu's context on the CPU path). A
//! text's glyphs are FreeType's masks under the reader's font settings, each drawn where its
//! text stands in that order, as an image tinted in its color, so a box painted later covers
//! them. An overlay's subtree is painted on its own surface.

use std::{
  cell::Cell,
  collections::HashMap,
  rc::Rc,
  sync::Arc,
  time::{Duration, Instant},
};

use skene_foundation::{
  Color, Corners, Fill, UnitPoint,
  layout::Rect,
  seam::{
    node::{LayoutKind, NodeKind},
    picture::{Paint, PaintOp, Picture, peniko},
  },
  text::Laid as _,
  value::{BorderStyle, Ring},
};
use skene_parley::{
  ParleyLaid,
  linux::{GlyphMask, MaskPixels},
  render::{DecorationShape, MaskPainter, PlacedGlyph},
};
use vello_cpu::{
  Image, ImageSource, PixelMetadata, Pixmap, RenderContext, Resources, Tint, TintMode,
  kurbo::{
    Affine, BezPath, Circle, Point, Rect as KRect, RoundedRect, RoundedRectRadii, Shape as _,
    Stroke,
  },
  peniko::{Color as PColor, Gradient, ImageAlphaType, ImageQuality, ImageSampler},
};

use crate::{node::Node, palette::Palette, text};

/// A Skene color as vello's.
pub const fn color(color: Color) -> PColor {
  PColor::from_rgba8(color.red, color.green, color.blue, color.alpha)
}

/// A rectangle as kurbo's.
pub fn krect(rect: Rect) -> KRect {
  KRect::new(
    f64::from(rect.x),
    f64::from(rect.y),
    f64::from(rect.x + rect.width),
    f64::from(rect.y + rect.height),
  )
}

/// Whether two rectangles overlap.
pub fn overlaps(a: Rect, b: Rect) -> bool {
  a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// The intersection of two rectangles, empty where they do not overlap.
pub fn intersect(a: Rect, b: Rect) -> Rect {
  let x = a.x.max(b.x);
  let y = a.y.max(b.y);
  let right = (a.x + a.width).min(b.x + b.width);
  let bottom = (a.y + a.height).min(b.y + b.height);

  Rect {
    x,
    y,
    width: (right - x).max(0.),
    height: (bottom - y).max(0.),
  }
}

/// The smallest rectangle holding both.
pub fn union(a: Rect, b: Rect) -> Rect {
  if a.width <= 0. || a.height <= 0. {
    return b;
  }

  if b.width <= 0. || b.height <= 0. {
    return a;
  }

  let x = a.x.min(b.x);
  let y = a.y.min(b.y);

  Rect {
    x,
    y,
    width: (a.x + a.width).max(b.x + b.width) - x,
    height: (a.y + a.height).max(b.y + b.height) - y,
  }
}

/// Corner radii as kurbo's, clamped to the box (leading is left: the layout is left to right).
pub fn radii(corners: Corners, rect: Rect) -> RoundedRectRadii {
  let most = f64::from(rect.width.min(rect.height) / 2.).max(0.);
  let clamp = |radius: f32| f64::from(radius).clamp(0., most);

  RoundedRectRadii::new(
    clamp(corners.top_leading),
    clamp(corners.top_trailing),
    clamp(corners.bottom_trailing),
    clamp(corners.bottom_leading),
  )
}

/// What a shape is filled or stroked with.
#[derive(Clone, Debug)]
pub enum Brush {
  /// One color.
  Solid(PColor),
  /// A gradient, laid over the shape by the transform from its own space to the device's.
  Gradient(Gradient, Affine),
}

/// What the painter draws through, in device pixels: a renderer's scene, the shapes kept in
/// the order drawn. Every call names the transform it draws under, so none depends on what
/// an earlier one left.
pub trait Sink {
  /// Fills `path`, placed by `transform`, with `brush`.
  fn fill(&mut self, path: &BezPath, transform: Affine, brush: &Brush);

  /// Fills `rect`, placed by `transform`, with `brush`.
  fn fill_rect(&mut self, rect: KRect, transform: Affine, brush: &Brush);

  /// Strokes `path`, placed by `transform`, with `brush`.
  fn stroke(&mut self, path: &BezPath, stroke: &Stroke, transform: Affine, brush: &Brush);

  /// Fills `rect` with corners `radius` round, blurred by a deviation of `deviation` (its
  /// inverse where `invert`: clear inside, `color` outside), placed by `transform`.
  fn blurred_rect(
    &mut self,
    rect: KRect,
    radius: f32,
    deviation: f32,
    invert: bool,
    transform: Affine,
    color: PColor,
  );

  /// Cuts what is drawn until the matching [`Sink::pop`] to `path`, placed by `transform`.
  fn push_clip(&mut self, path: &BezPath, transform: Affine);

  /// Fades what is drawn until the matching [`Sink::pop`] to `opacity`, as one layer.
  fn push_opacity(&mut self, opacity: f32);

  /// Ends the last clip or opacity layer pushed.
  fn pop(&mut self);

  /// Draws `glyph`'s mask at its device pixel: a coverage mask in its color, a color glyph
  /// in its own.
  fn glyph(&mut self, glyph: &PlacedGlyph);
}

/// The CPU path's glyph images: each mask the rasterizer cached, as an image vello_cpu
/// draws, with the frame that last drew it.
#[derive(Default)]
pub struct GlyphImages {
  images: HashMap<usize, (Rc<GlyphMask>, Image, u64)>,
  bytes: usize,
  frame: u64,
  hits: u64,
  misses: u64,
}

impl GlyphImages {
  /// The bytes of images the cache keeps; past them it keeps the current frame's alone.
  const BUDGET: usize = 64 << 20;

  /// Starts a frame: the images it draws are the ones the cache keeps once it is past its
  /// budget.
  pub const fn next_frame(&mut self) {
    self.frame += 1;
  }

  /// The image of `mask`: its coverage as alpha (black beneath, the tint giving the color),
  /// or a color glyph's premultiplied pixels.
  fn image(&mut self, mask: &Rc<GlyphMask>) -> Image {
    let key = Rc::as_ptr(mask) as usize;

    if let Some((_, image, used)) = self.images.get_mut(&key) {
      self.hits += 1;
      *used = self.frame;

      return image.clone();
    }

    self.misses += 1;

    // Its premultiplied pixels, four bytes each.
    let bytes = mask.width as usize * mask.height as usize * 4;

    if self.bytes + bytes > Self::BUDGET {
      let frame = self.frame;

      self.images.retain(|_, (_, _, used)| *used == frame);
      self.bytes = self
        .images
        .values()
        .map(|(mask, ..)| mask.width as usize * mask.height as usize * 4)
        .sum();
    }

    let image = Image {
      image: ImageSource::Pixmap(Arc::new(mask_pixmap(mask))),
      sampler: ImageSampler {
        quality: ImageQuality::Low,
        ..ImageSampler::default()
      },
    };

    self.bytes += bytes;
    self
      .images
      .insert(key, (Rc::clone(mask), image.clone(), self.frame));

    image
  }

  /// How many glyph draws found their image cached, and how many made it.
  #[must_use]
  pub const fn lookups(&self) -> (u64, u64) {
    (self.hits, self.misses)
  }
}

/// A mask's pixels as a premultiplied pixmap: coverage as alpha over black, or a color
/// glyph's own.
fn mask_pixmap(mask: &GlyphMask) -> Pixmap {
  let (Ok(width), Ok(height)) = (u16::try_from(mask.width), u16::try_from(mask.height)) else {
    return Pixmap::new(1, 1);
  };
  let data = match &mask.pixels {
    MaskPixels::Alpha(alpha) => alpha.iter().flat_map(|&a| [0, 0, 0, a]).collect(),
    MaskPixels::Color(rgba) => rgba.clone(),
  };

  Pixmap::from_parts(
    data,
    width,
    height,
    PixelMetadata::new(ImageAlphaType::AlphaPremultiplied, true),
  )
}

/// vello_cpu's context as a [`Sink`], with the glyph images it draws from.
pub struct CpuSink<'a> {
  /// The context, in device pixels.
  pub ctx: &'a mut RenderContext,
  /// The glyph images.
  pub glyphs: &'a mut GlyphImages,
}

impl CpuSink<'_> {
  fn brush(&mut self, transform: Affine, brush: &Brush) {
    self.ctx.set_transform(transform);

    match brush {
      Brush::Solid(color) => {
        self.ctx.set_paint(*color);
        self.ctx.reset_paint_transform();
      }
      Brush::Gradient(gradient, placed) => {
        self.ctx.set_paint(gradient.clone());
        self.ctx.set_paint_transform(transform.inverse() * *placed);
      }
    }
  }
}

impl Sink for CpuSink<'_> {
  fn fill(&mut self, path: &BezPath, transform: Affine, brush: &Brush) {
    self.brush(transform, brush);
    self.ctx.fill_path(path);
  }

  fn fill_rect(&mut self, rect: KRect, transform: Affine, brush: &Brush) {
    self.brush(transform, brush);
    self.ctx.fill_rect(&rect);
  }

  fn stroke(&mut self, path: &BezPath, stroke: &Stroke, transform: Affine, brush: &Brush) {
    self.brush(transform, brush);
    self.ctx.set_stroke(stroke.clone());
    self.ctx.stroke_path(path);
  }

  fn blurred_rect(
    &mut self,
    rect: KRect,
    radius: f32,
    deviation: f32,
    invert: bool,
    transform: Affine,
    color: PColor,
  ) {
    self.brush(transform, &Brush::Solid(color));
    self
      .ctx
      .fill_blurred_rounded_rect(&rect, radius, deviation, invert);
  }

  fn push_clip(&mut self, path: &BezPath, transform: Affine) {
    self.ctx.set_transform(transform);
    self.ctx.push_clip_layer(path);
  }

  fn push_opacity(&mut self, opacity: f32) {
    self.ctx.push_opacity_layer(opacity);
  }

  fn pop(&mut self) {
    self.ctx.pop_layer();
  }

  fn glyph(&mut self, glyph: &PlacedGlyph) {
    let image = self.glyphs.image(&glyph.mask);
    let tint = matches!(glyph.mask.pixels, MaskPixels::Alpha(_)).then(|| Tint {
      color: color(glyph.color),
      mode: TintMode::AlphaMask,
    });

    self
      .ctx
      .set_transform(Affine::translate((glyph.left as f64, glyph.top as f64)));
    self.ctx.reset_paint_transform();
    self.ctx.set_paint(image);
    self.ctx.set_tint(tint);
    self.ctx.fill_rect(&KRect::new(
      0.,
      0.,
      f64::from(glyph.mask.width),
      f64::from(glyph.mask.height),
    ));
    self.ctx.reset_tint();
  }
}

/// What one frame paints with: the sink, the glyph painter, the scale and the offset from
/// window pixels to the frame's device pixels, the clip in force.
pub struct Canvas<'a> {
  /// What the frame draws through, in device pixels.
  pub sink: &'a mut dyn Sink,
  masks: Option<&'a mut MaskPainter>,
  scale: f64,
  offset: (f64, f64),
  clip: Rect,
  glyph_time: Duration,
  /// The defaults the frame draws with.
  pub palette: Palette,
}

impl Canvas<'_> {
  fn to_device(&self) -> Affine {
    Affine::translate((-self.offset.0, -self.offset.1)) * Affine::scale(self.scale)
  }

  /// Fills `rect` (window pixels) with `fill`, its corners rounded by `corners`.
  pub fn fill(&mut self, rect: Rect, corners: Corners, fill: Color) {
    if fill.alpha == 0 || rect.width <= 0. || rect.height <= 0. {
      return;
    }

    let transform = self.to_device();
    let brush = Brush::Solid(color(fill));

    if corners == Corners::all(0.) {
      self.sink.fill_rect(krect(rect), transform, &brush);
    } else {
      self.sink.fill(
        &RoundedRect::from_rect(krect(rect), radii(corners, rect)).to_path(0.1),
        transform,
        &brush,
      );
    }
  }

  /// Fills `rect` (window pixels) with `fill`, a color or a gradient laid over it, its
  /// corners rounded by `corners`.
  pub fn fill_with(&mut self, rect: Rect, corners: Corners, fill: &Fill) {
    if fill.is_clear() || rect.width <= 0. || rect.height <= 0. {
      return;
    }

    let transform = self.to_device();
    let brush = self.brush(fill, rect);

    if corners == Corners::all(0.) {
      self.sink.fill_rect(krect(rect), transform, &brush);
    } else {
      self.sink.fill(
        &RoundedRect::from_rect(krect(rect), radii(corners, rect)).to_path(0.1),
        transform,
        &brush,
      );
    }
  }

  /// The brush `fill` paints the box at `rect` (window pixels) with: one color, or a
  /// gradient laid over the box as CSS lays it over the border box.
  fn brush(&self, fill: &Fill, rect: Rect) -> Brush {
    let (x, y, width, height) = (
      f64::from(rect.x),
      f64::from(rect.y),
      f64::from(rect.width),
      f64::from(rect.height),
    );
    let at = |point: UnitPoint| (f64::from(point.x) * width, f64::from(point.y) * height);
    let (gradient, placed) = match fill {
      Fill::Color(fill) => return Brush::Solid(color(*fill)),
      // The node's ink is put in before a fill is drawn (`Fill::in_ink`); one left draws
      // nothing.
      Fill::Ink(_) => return Brush::Solid(color(skene_foundation::Color::TRANSPARENT)),
      Fill::Linear(linear) => {
        let (sin, cos) = f64::from(linear.angle.in_radians()).sin_cos();
        let length = (width * sin).abs() + (height * cos).abs();
        let (dx, dy) = (sin * length / 2., -cos * length / 2.);
        let (cx, cy) = (x + width / 2., y + height / 2.);

        (
          Gradient::new_linear((cx - dx, cy - dy), (cx + dx, cy + dy)),
          Affine::IDENTITY,
        )
      }
      Fill::Radial(radial) => {
        let (cx, cy) = at(radial.center);

        match radial.radius {
          Some(radius) => (
            Gradient::new_radial((x + cx, y + cy), radius),
            Affine::IDENTITY,
          ),
          None => {
            // The ellipse through the farthest corner, shaped as the closest sides shape it.
            let (side_x, side_y) = (cx.min(width - cx), cy.min(height - cy));
            let (far_x, far_y) = (cx.max(width - cx), cy.max(height - cy));
            let (radius_x, radius_y) = if side_x > 0. && side_y > 0. {
              let grow = ((far_x / side_x).powi(2) + (far_y / side_y).powi(2)).sqrt();

              (side_x * grow, side_y * grow)
            } else {
              let reach = far_x.hypot(far_y);

              (reach, reach)
            };

            (
              Gradient::new_radial((0., 0.), 1.),
              Affine::translate((x + cx, y + cy)) * Affine::scale_non_uniform(radius_x, radius_y),
            )
          }
        }
      }
      Fill::Sweep(sweep) => {
        let (cx, cy) = at(sweep.center);
        // The turn starts up, at its start angle, and runs clockwise: the gradient's own turn
        // (from the x axis, a whole turn) rotated there about the center.
        let start = f64::from(sweep.start.in_degrees() - 90.).to_radians();

        (
          Gradient::new_sweep((0., 0.), 0., std::f32::consts::TAU),
          Affine::translate((x + cx, y + cy)) * Affine::rotate(start),
        )
      }
    };
    let mut reached = 0_f32;
    let stops: Vec<(f32, PColor)> = fill
      .stops()
      .map(|stops| {
        stops
          .0
          .iter()
          .map(|stop| {
            // A stop before the one ahead of it stands where that one does.
            reached = reached.max(stop.at.clamp(0., 1.));

            (
              reached,
              color(stop.color.fixed().unwrap_or(Color::TRANSPARENT)),
            )
          })
          .collect()
      })
      .unwrap_or_default();

    Brush::Gradient(
      gradient.with_stops(stops.as_slice()),
      self.to_device() * placed,
    )
  }

  /// Draws a line `width` wide just inside `rect`'s edges, in `style`, in `fill`: a color, or
  /// a gradient laid over the box.
  pub fn border_with(
    &mut self,
    rect: Rect,
    corners: Corners,
    width: f32,
    style: BorderStyle,
    fill: &Fill,
  ) {
    if width <= 0. || fill.is_clear() {
      return;
    }

    let brush = self.brush(fill, rect);

    self.stroke_border(rect, corners, width, style, &brush);
  }

  /// Draws a line `width` wide just inside `rect`'s edges, in `style`.
  pub fn border(
    &mut self,
    rect: Rect,
    corners: Corners,
    width: f32,
    style: BorderStyle,
    ink: Color,
  ) {
    if width <= 0. || ink.alpha == 0 {
      return;
    }

    self.stroke_border(rect, corners, width, style, &Brush::Solid(color(ink)));
  }

  fn stroke_border(
    &mut self,
    rect: Rect,
    corners: Corners,
    width: f32,
    style: BorderStyle,
    brush: &Brush,
  ) {
    let half = width / 2.;
    let inner = Rect {
      x: rect.x + half,
      y: rect.y + half,
      width: (rect.width - width).max(0.),
      height: (rect.height - width).max(0.),
    };
    let shrunk = Corners {
      top_leading: (corners.top_leading - half).max(0.),
      top_trailing: (corners.top_trailing - half).max(0.),
      bottom_trailing: (corners.bottom_trailing - half).max(0.),
      bottom_leading: (corners.bottom_leading - half).max(0.),
    };
    let path = RoundedRect::from_rect(krect(inner), radii(shrunk, inner)).to_path(0.1);
    let mut stroke = Stroke::new(f64::from(width));

    match style {
      BorderStyle::Solid | BorderStyle::Double => {}
      BorderStyle::Dashed => stroke = stroke.with_dashes(0., [f64::from(width) * 3.]),
      BorderStyle::Dotted => stroke = stroke.with_dashes(0., [f64::from(width)]),
    }

    let transform = self.to_device();

    self.sink.stroke(&path, &stroke, transform, brush);
  }

  /// Fills a circle centered at `center`.
  pub fn circle(&mut self, center: (f32, f32), radius: f32, fill: Color) {
    let transform = self.to_device();

    self.sink.fill(
      &Circle::new(
        (f64::from(center.0), f64::from(center.1)),
        f64::from(radius),
      )
      .to_path(0.1),
      transform,
      &Brush::Solid(color(fill)),
    );
  }

  /// Strokes `path` (window pixels) `width` wide.
  pub fn stroke_path(&mut self, path: &BezPath, width: f32, ink: Color) {
    let transform = self.to_device();

    self.sink.stroke(
      path,
      &Stroke::new(f64::from(width)),
      transform,
      &Brush::Solid(color(ink)),
    );
  }

  /// A shadow of the box `rect` with `corners`, blurred by `blur`, grown by `spread`, offset
  /// by `x`, `y`.
  #[expect(clippy::too_many_arguments, reason = "a shadow's every parameter")]
  pub fn shadow(
    &mut self,
    rect: Rect,
    corners: Corners,
    x: f32,
    y: f32,
    blur: f32,
    spread: f32,
    ink: Color,
  ) {
    let rect = Rect {
      x: rect.x + x - spread,
      y: rect.y + y - spread,
      width: rect.width + spread * 2.,
      height: rect.height + spread * 2.,
    };
    let radius = corners.top_leading.max(corners.top_trailing) + spread.max(0.);
    let transform = self.to_device();

    self.sink.blurred_rect(
      krect(rect),
      radius.max(0.),
      (blur / 2.).max(0.01),
      false,
      transform,
      color(ink),
    );
  }

  /// A shadow cast inside the box `rect` with `corners`, blurred by `blur`, shrunk by
  /// `spread`, offset by `x`, `y`; cut to the box by the caller.
  #[expect(clippy::too_many_arguments, reason = "a shadow's every parameter")]
  pub fn inner_shadow(
    &mut self,
    rect: Rect,
    corners: Corners,
    x: f32,
    y: f32,
    blur: f32,
    spread: f32,
    ink: Color,
  ) {
    let transform = self.to_device();

    self.sink.blurred_rect(
      krect(Rect {
        x: rect.x + x + spread,
        y: rect.y + y + spread,
        width: rect.width - spread * 2.,
        height: rect.height - spread * 2.,
      }),
      corners.top_leading,
      (blur / 2.).max(0.01),
      true,
      transform,
      color(ink),
    );
  }

  /// Draws `laid` with its top left at `origin` (window pixels): its decorations, then its
  /// glyphs, each where the text stands in the scene's order, cut to the clip in force.
  pub fn text(&mut self, laid: &ParleyLaid, origin: (f32, f32)) {
    let transform =
      self.to_device() * Affine::translate((f64::from(origin.0), f64::from(origin.1)));

    for extent in laid.run_extents() {
      for line in extent.decoration_lines() {
        let brush = Brush::Solid(color(line.color));

        match DecorationShape::of(&line) {
          DecorationShape::Fill(path) => self.sink.fill(&path, transform, &brush),
          DecorationShape::Stroke(path, stroke) => {
            self.sink.stroke(&path, &stroke, transform, &brush);
          }
        }
      }
    }

    let device = self.to_device() * Point::new(f64::from(origin.0), f64::from(origin.1));
    let scale = self.scale;
    let Some(masks) = self.masks.as_deref_mut() else {
      return;
    };
    let started = Instant::now();
    let engine = text::engine();
    let mut fallback = engine.fallback();
    let sink = &mut *self.sink;

    #[expect(clippy::cast_possible_truncation, reason = "device pixels")]
    masks.place(
      laid,
      (device.x as f32, device.y as f32),
      scale as f32,
      fallback.as_deref_mut(),
      |glyph| sink.glyph(&glyph),
    );
    self.glyph_time += started.elapsed();
  }

  /// Replays `picture`, recorded in device pixels, with its origin at `rect`'s top left
  /// (window pixels) snapped to the device's pixel grid, cut to `rect`, [`Paint::Ink`]
  /// drawing in `ink`.
  pub fn picture(&mut self, picture: &Picture, rect: Rect, ink: Color) {
    if picture.is_empty() || rect.width <= 0. || rect.height <= 0. {
      return;
    }

    let previous = self.push_clip(rect, Corners::all(0.));
    let origin = self.to_device() * Point::new(f64::from(rect.x), f64::from(rect.y));
    let base = Affine::translate((origin.x.round(), origin.y.round()));
    let mut transform = base;
    let mut depth = 0_u32;
    let brush = |paint: &Paint, transform: Affine| -> Option<Brush> {
      Some(match paint {
        Paint::Ink => Brush::Solid(color(ink)),
        Paint::Color(fill) => Brush::Solid(color(*fill)),
        Paint::Brush(peniko::Brush::Solid(fill)) => Brush::Solid(*fill),
        Paint::Brush(peniko::Brush::Gradient(gradient)) => {
          Brush::Gradient(gradient.clone(), transform)
        }
        Paint::Brush(peniko::Brush::Image(_)) => return None,
      })
    };

    for op in picture.ops() {
      match op {
        PaintOp::Transform(op) => transform = base * *op,
        PaintOp::Fill { path, paint } => {
          if let Some(brush) = brush(paint, transform) {
            self.sink.fill(path, transform, &brush);
          }
        }
        PaintOp::Stroke {
          path,
          stroke,
          paint,
        } => {
          if let Some(brush) = brush(paint, transform) {
            self.sink.stroke(path, stroke, transform, &brush);
          }
        }
        PaintOp::PushClip(path) => {
          self.sink.push_clip(path, transform);
          depth += 1;
        }
        PaintOp::PushOpacity(opacity) => {
          self.sink.push_opacity(*opacity);
          depth += 1;
        }
        PaintOp::Pop if depth > 0 => {
          self.sink.pop();
          depth -= 1;
        }
        PaintOp::Pop => {}
      }
    }

    for _ in 0..depth {
      self.sink.pop();
    }

    self.pop_clip(previous);
  }

  /// Draws `ring` around the box at `rect` with `corners`, in `ink`.
  pub fn ring(&mut self, rect: Rect, corners: Corners, ring: Ring, ink: Color) {
    if !ring.is_visible() {
      return;
    }

    let grow = ring.offset + ring.width;
    let grown = Rect {
      x: rect.x - grow,
      y: rect.y - grow,
      width: rect.width + grow * 2.,
      height: rect.height + grow * 2.,
    };
    let round = |radius: f32| {
      if radius > 0. {
        (radius + grow).max(0.)
      } else {
        0.
      }
    };
    let corners = Corners {
      top_leading: round(corners.top_leading),
      top_trailing: round(corners.top_trailing),
      bottom_trailing: round(corners.bottom_trailing),
      bottom_leading: round(corners.bottom_leading),
    };

    self.border(grown, corners, ring.width, BorderStyle::Solid, ink);
  }

  /// Runs `body` with what it draws cut to `rect` (window pixels).
  pub fn clipped(&mut self, rect: Rect, body: impl FnOnce(&mut Self)) {
    let previous = self.push_clip(rect, Corners::all(0.));

    body(self);
    self.pop_clip(previous);
  }

  /// Runs `body` with what it draws faded to `opacity`, as one layer.
  pub fn faded(&mut self, opacity: f32, body: impl FnOnce(&mut Self)) {
    self.sink.push_opacity(opacity);
    body(self);
    self.sink.pop();
  }

  fn push_clip(&mut self, rect: Rect, corners: Corners) -> Rect {
    let previous = self.clip;
    let transform = self.to_device();

    self.clip = intersect(self.clip, rect);

    if corners == Corners::all(0.) {
      self.sink.push_clip(&krect(rect).to_path(0.1), transform);
    } else {
      self.sink.push_clip(
        &RoundedRect::from_rect(krect(rect), radii(corners, rect)).to_path(0.1),
        transform,
      );
    }

    previous
  }

  fn pop_clip(&mut self, previous: Rect) {
    self.sink.pop();
    self.clip = previous;
  }
}

/// How much farther than its box a node may paint: its outer shadows, its ring, and the ring
/// the realization draws around a focused link run or control.
fn overhang(node: &Node) -> f32 {
  let look = node.look.borrow();
  let shadows = look.shadows.as_ref().map_or(0., |shadows| {
    shadows
      .0
      .iter()
      .filter(|shadow| !shadow.inset)
      .map(|shadow| shadow.blur * 1.5 + shadow.spread.max(0.) + shadow.x.abs().max(shadow.y.abs()))
      .fold(0., f32::max)
  });

  shadows.max(look.ring.reach()).max(4.)
}

/// What a node paints beyond its box, at its window rectangle `rect`.
pub fn painted_bounds(node: &Node, rect: Rect) -> Rect {
  let more = overhang(node);

  Rect {
    x: rect.x - more,
    y: rect.y - more,
    width: rect.width + more * 2.,
    height: rect.height + more * 2.,
  }
}

/// What the painter draws differently from the nodes' own styles: where focus is, and the
/// focused link run.
#[derive(Clone, Default)]
pub struct Marks {
  /// The focused node, where focus shows.
  pub focus: Option<Node>,
  /// The focused node, whether focus shows or not: a text entry draws its caret.
  pub focused: Option<Node>,
  /// The focused link run of the focused text, by its bytes.
  pub link: Option<std::ops::Range<usize>>,
}

fn paint_node(
  node: &Node,
  canvas: &mut Canvas<'_>,
  origin: (f32, f32),
  region: Rect,
  marks: &Marks,
) {
  if node.is_hidden() {
    return;
  }

  let frame = node.frame.get();
  let (dx, dy) = node.offset();
  let rect = Rect {
    x: origin.0 + frame.x + dx,
    y: origin.1 + frame.y + dy,
    width: frame.width,
    height: frame.height,
  };
  let reach = node.reach.get();

  if !overlaps(
    Rect {
      x: rect.x + reach.x,
      y: rect.y + reach.y,
      ..reach
    },
    region,
  ) {
    return;
  }

  let scrolls = matches!(node.layout_kind(), Some(LayoutKind::Scroll { .. }));

  if !overlaps(painted_bounds(node, rect), region) {
    // Only a child overhanging the box reaches the region.
    paint_children(node, canvas, (rect.x, rect.y), region, marks);

    return;
  }

  let look = node.look.borrow().clone();

  let opacity_layer = look.opacity < 1.;

  if opacity_layer {
    canvas.sink.push_opacity(look.opacity);
  }

  if let Some(shadows) = &look.shadows {
    for shadow in shadows.0.iter().filter(|shadow| !shadow.inset) {
      canvas.shadow(
        rect,
        look.corners,
        shadow.x,
        shadow.y,
        shadow.blur,
        shadow.spread,
        shadow.color,
      );
    }
  }

  if let Some(background) = &look.background {
    canvas.fill_with(rect, look.corners, &background.in_ink(node.ink.get()));
  }

  if let Some(shadows) = &look.shadows
    && shadows.0.iter().any(|shadow| shadow.inset)
  {
    let previous = canvas.push_clip(rect, look.corners);

    for shadow in shadows.0.iter().filter(|shadow| shadow.inset) {
      canvas.inner_shadow(
        rect,
        look.corners,
        shadow.x,
        shadow.y,
        shadow.blur,
        shadow.spread,
        shadow.color,
      );
    }

    canvas.pop_clip(previous);
  }

  if let Some(border) = look.border {
    let fill = look
      .border_color
      .clone()
      .unwrap_or_else(|| Fill::Color(node.ink.get()));

    canvas.border_with(
      rect,
      look.corners,
      border.width,
      border.style,
      &fill.in_ink(node.ink.get()),
    );
  }

  match node.spec.kind {
    NodeKind::Text => {
      if let Some(cell) = node.text.borrow().as_ref()
        && let Some(laid) = cell.laid(Some(node.laid_width.get()))
      {
        canvas.text(&laid, (rect.x, rect.y));

        if let (Some(focused), Some(link)) = (&marks.focus, &marks.link)
          && Rc::ptr_eq(focused, node)
        {
          // A text's ring is its focused link run's: 2 pixels wide in the accent unless its
          // style says otherwise.
          let ring = if look.ring.width > 0. {
            look.ring
          } else {
            Ring::new(2)
          };
          let color = look.ring_color.unwrap_or(canvas.palette.accent);

          for run in laid.range_rects(link.clone()) {
            let run = Rect {
              x: rect.x + run.x,
              y: rect.y + run.y,
              ..run
            };

            canvas.ring(run, Corners::all(2.), ring, color);
          }
        }
      }
    }
    NodeKind::Editable { .. } => {
      let focused = marks
        .focused
        .as_ref()
        .is_some_and(|focused| Rc::ptr_eq(focused, node));

      node.paint_entry(canvas, rect, focused);
    }
    NodeKind::Painted => {
      if let Some(picture) = node.contents.borrow().picture.clone() {
        canvas.picture(&picture, rect, node.ink.get());
      }
    }
    _ => {}
  }

  let clips = look.clip || scrolls;
  let previous_clip = clips.then(|| {
    let (inside, corners) = padding_box(rect, look.corners, look.border.map(|border| border.width));

    canvas.push_clip(inside, corners)
  });
  paint_children(node, canvas, (rect.x, rect.y), region, marks);

  if let Some(previous) = previous_clip {
    canvas.pop_clip(previous);
  }

  if node.spec.kind != NodeKind::Text {
    canvas.ring(
      rect,
      look.corners,
      look.ring,
      look.ring_color.unwrap_or_else(|| node.ink.get()),
    );
  }

  if opacity_layer {
    canvas.sink.pop();
  }
}

/// The box inside a border `width` wide (none where `None`) around `rect` with `corners`, and
/// its corners: where a box that clips cuts what it holds, as CSS clips overflow.
fn padding_box(rect: Rect, corners: Corners, width: Option<f32>) -> (Rect, Corners) {
  let Some(width) = width.filter(|width| *width > 0.) else {
    return (rect, corners);
  };
  let inside = Rect {
    x: rect.x + width,
    y: rect.y + width,
    width: (rect.width - 2. * width).max(0.),
    height: (rect.height - 2. * width).max(0.),
  };
  let inner = |radius: f32| (radius - width).max(0.);

  (
    inside,
    Corners {
      top_leading: inner(corners.top_leading),
      top_trailing: inner(corners.top_trailing),
      bottom_trailing: inner(corners.bottom_trailing),
      bottom_leading: inner(corners.bottom_leading),
    },
  )
}

/// Paints `node`'s children in the scene's order, `node` drawn at `origin`: its scrolling
/// content moved by its scroll offset, a scroll bar's track unmoved and over the content.
fn paint_children(
  node: &Node,
  canvas: &mut Canvas<'_>,
  origin: (f32, f32),
  region: Rect,
  marks: &Marks,
) {
  for child in node.children_ref().iter() {
    if !child.is_overlay() {
      let (sx, sy) = node.scroll_for(child);

      paint_node(child, canvas, (origin.0 - sx, origin.1 - sy), region, marks);
    }
  }
}

/// Which path a window asks for when it opens: what `SKENE_RENDERER` names (`cpu` the CPU path,
/// `gpu` any adapter, a software one included; unset a hardware adapter where one presents,
/// the CPU otherwise), unless this thread chose ([`PathChoice::set_current`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathChoice {
  /// The CPU path.
  Cpu,
  /// A hardware adapter where one presents to the window.
  Hardware,
  /// Any adapter that presents to the window, a software one included.
  Any,
}

thread_local! {
  static CHOSEN: Cell<Option<PathChoice>> = const { Cell::new(None) };
}

impl PathChoice {
  /// The choice a window opened on this thread now makes.
  #[must_use]
  pub fn current() -> Self {
    CHOSEN
      .get()
      .unwrap_or_else(|| match std::env::var("SKENE_RENDERER").as_deref() {
        Ok("cpu") => Self::Cpu,
        Ok("gpu") => Self::Any,
        _ => Self::Hardware,
      })
  }

  /// Makes `choice` the one windows opened on this thread from now on make, in place of
  /// `SKENE_RENDERER`'s; `None` returns to it.
  pub fn set_current(choice: Option<Self>) {
    CHOSEN.set(choice);
  }
}

/// The path a window paints with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintPath {
  /// vello_cpu, into shared memory.
  Cpu,
  /// vello's hybrid renderer over wgpu, presented through the platform's swapchain.
  Gpu,
}

/// The frame's renderer: its vello context and resources, the glyph painter and its images,
/// the frame's pixels, and the GPU path where the window paints on the GPU.
pub struct Renderer {
  ctx: Option<RenderContext>,
  resources: Resources,
  masks: Option<MaskPainter>,
  glyphs: GlyphImages,
  #[cfg(feature = "gpu")]
  gpu: Option<crate::gpu::Gpu>,
  stale: bool,
  /// The frame: premultiplied RGBA, in device pixels. On the GPU path, the last frame read
  /// back when asked for ([`Renderer::last_frame`]).
  pub pixmap: Pixmap,
}

/// What one frame cost, in microseconds, and how its glyphs were found.
#[derive(Clone, Copy, Default, Debug)]
pub struct Cost {
  /// Recording the scene, the glyphs placed included.
  pub record: u64,
  /// Rasterizing it.
  pub raster: u64,
  /// Placing the glyphs (their masks found or drawn), within the recording.
  pub glyphs: u64,
  /// The glyph masks the rasterizer's cache answered, and those FreeType drew, so far.
  pub masks: (u64, u64),
  /// The glyph images found cached, and those made, so far.
  pub images: (u64, u64),
  /// Whether the frame went to the screen through the GPU's swapchain, the whole of it.
  pub presented: bool,
}

impl Default for Renderer {
  fn default() -> Self {
    Self::new()
  }
}

impl Renderer {
  /// The side, in device pixels, of the tiles vello_cpu computes coverage on, counted from
  /// the top left of what it draws into: a band of a frame from its left edge, its top and
  /// bottom on multiples of it, comes out pixel for pixel as the whole frame drawn at once
  /// does.
  const TILE: f64 = 4.;

  /// A renderer with no frame yet.
  pub fn new() -> Self {
    Self {
      ctx: None,
      resources: Resources::new(),
      masks: MaskPainter::new(),
      glyphs: GlyphImages::default(),
      #[cfg(feature = "gpu")]
      gpu: None,
      stale: false,
      pixmap: Pixmap::new(1, 1),
    }
  }

  /// Paints on the GPU from now on, through `gpu`.
  #[cfg(feature = "gpu")]
  pub fn use_gpu(&mut self, gpu: crate::gpu::Gpu) {
    self.gpu = Some(gpu);
  }

  /// The path frames are painted with now.
  #[must_use]
  pub const fn path(&self) -> PaintPath {
    #[cfg(feature = "gpu")]
    if self.gpu.is_some() {
      return PaintPath::Gpu;
    }

    PaintPath::Cpu
  }

  /// Loses the GPU's device, as a driver reset or a removed GPU would; whether the window
  /// painted on the GPU. The next frame paints on the CPU.
  pub fn lose_device(&mut self) -> bool {
    #[cfg(feature = "gpu")]
    if let Some(gpu) = &self.gpu {
      gpu.lose();

      return true;
    }

    false
  }

  /// The last frame painted: on the GPU path, read back from the GPU first.
  pub fn last_frame(&mut self) -> &Pixmap {
    #[cfg(feature = "gpu")]
    if self.stale
      && let Some((width, height, pixels)) = self.gpu.as_ref().and_then(crate::gpu::Gpu::read_back)
    {
      let data = pixels.into_iter().flatten().collect();

      self.pixmap = Pixmap::from_parts(
        data,
        u16::try_from(width).unwrap_or(1),
        u16::try_from(height).unwrap_or(1),
        PixelMetadata::new(ImageAlphaType::AlphaPremultiplied, true),
      );
    }

    self.stale = false;

    &self.pixmap
  }

  /// Paints the whole frame of `size` device pixels on the GPU and presents it, as
  /// [`Renderer::paint`] paints on the CPU; `None` where the device is lost, which hands the
  /// window to the CPU path.
  #[cfg(feature = "gpu")]
  #[expect(clippy::too_many_arguments, reason = "one frame's every input")]
  fn paint_gpu(
    &mut self,
    root: Option<&Node>,
    size: (u16, u16),
    scale: f64,
    region: Rect,
    origin: (f32, f32),
    marks: &Marks,
    chrome: &mut dyn FnMut(&mut Canvas<'_>),
  ) -> Option<Cost> {
    let gpu = self.gpu.as_mut()?;
    let masks = &mut self.masks;
    let mut glyph_time = Duration::ZERO;
    let result = gpu.paint((u32::from(size.0), u32::from(size.1)), &mut |sink| {
      let mut canvas = Canvas {
        sink,
        masks: masks.as_mut(),
        scale,
        offset: (-f64::from(origin.0) * scale, -f64::from(origin.1) * scale),
        clip: region,
        glyph_time: Duration::ZERO,
        palette: Palette::now(),
      };

      chrome(&mut canvas);

      if let Some(root) = root {
        paint_node(root, &mut canvas, (0., 0.), region, marks);
      }

      glyph_time = canvas.glyph_time;
    });

    match result {
      Ok(cost) => {
        self.stale = true;

        if cost.short {
          crate::shell::invalidate_all();
        }

        #[expect(clippy::cast_possible_truncation, reason = "microseconds")]
        Some(Cost {
          record: cost.record,
          raster: cost.render,
          glyphs: glyph_time.as_micros() as u64,
          masks: self
            .masks
            .as_ref()
            .map_or((0, 0), |masks| masks.rasterizer().lookups()),
          images: self.glyphs.lookups(),
          presented: true,
        })
      }
      Err(crate::gpu::Lost) => {
        crate::trace::log(|| "[gpu] lost: the window paints on the CPU from now on".to_owned());
        self.gpu = None;

        None
      }
    }
  }

  /// Paints the part `region` (window pixels) of `root`'s tree, where there is one, into the
  /// frame of `size` device pixels, the window's top left at `origin` (surface pixels, the
  /// CSD shadow's margin), at `scale`; `chrome` first paints what lies behind and around the
  /// content in the same canvas (the shadow, the window's fill, the title bar), or the whole
  /// of a surface with no tree (a popup's list).
  #[expect(clippy::too_many_arguments, reason = "one frame's every input")]
  pub fn paint(
    &mut self,
    root: Option<&Node>,
    size: (u16, u16),
    scale: f64,
    region: Rect,
    origin: (f32, f32),
    marks: &Marks,
    chrome: &mut dyn FnMut(&mut Canvas<'_>),
  ) -> Cost {
    let started = Instant::now();
    #[cfg(feature = "gpu")]
    let mut region = region;

    self.glyphs.next_frame();

    if let Some(masks) = self.masks.as_mut() {
      masks.rasterizer_mut().next_frame();
    }

    #[cfg(feature = "gpu")]
    if self.gpu.is_some() {
      #[expect(clippy::cast_possible_truncation, reason = "window pixels")]
      let whole = Rect {
        x: -origin.0,
        y: -origin.1,
        width: (f64::from(size.0) / scale) as f32,
        height: (f64::from(size.1) / scale) as f32,
      };

      if let Some(cost) = self.paint_gpu(root, size, scale, whole, origin, marks, chrome) {
        return cost;
      }

      // The device was lost: this frame and every one after paint on the CPU, this one whole.
      region = whole;
      self.pixmap = Pixmap::new(1, 1);
    }

    if self.pixmap.width() != size.0 || self.pixmap.height() != size.1 {
      self.pixmap = Pixmap::new(size.0, size.1);
    }

    // The band of the frame the damage lies in, in device pixels: from the frame's left edge,
    // its other edges on whole tiles around the damage.
    let device = |value: f32| f64::from(value) * scale;
    let before = |value: f64| (value / Self::TILE).floor() * Self::TILE;
    let after = |value: f64| (value / Self::TILE).ceil() * Self::TILE;
    let left = 0.;
    let top = before(device(region.y + origin.1)).max(0.);
    let right = after(device(region.x + origin.0 + region.width)).min(f64::from(size.0));
    let bottom = after(device(region.y + origin.1 + region.height)).min(f64::from(size.1));

    if right <= left || bottom <= top {
      return Cost::default();
    }

    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "pixels"
    )]
    let (dx, dy, dw, dh) = (
      left as u16,
      top as u16,
      (right - left) as u16,
      (bottom - top) as u16,
    );
    let ctx = self.ctx.get_or_insert_with(|| RenderContext::new(dw, dh));

    if ctx.width() != dw || ctx.height() != dh {
      ctx.reset_and_resize(dw, dh);
    } else {
      ctx.reset();
    }

    let window_region = Rect {
      x: (left / scale) as f32 - origin.0,
      y: (top / scale) as f32 - origin.1,
      width: ((right - left) / scale) as f32,
      height: ((bottom - top) / scale) as f32,
    };
    let mut sink = CpuSink {
      ctx,
      glyphs: &mut self.glyphs,
    };
    let mut canvas = Canvas {
      sink: &mut sink,
      masks: self.masks.as_mut(),
      scale,
      offset: (left - device(origin.0), top - device(origin.1)),
      clip: window_region,
      glyph_time: Duration::ZERO,
      palette: Palette::now(),
    };

    chrome(&mut canvas);

    if let Some(root) = root {
      paint_node(root, &mut canvas, (0., 0.), window_region, marks);
    }

    let glyph_time = canvas.glyph_time;
    let recorded = started.elapsed();
    let ctx = self.ctx.as_mut().expect("made above");

    ctx.flush();

    // Clear the region, then composite the scene over it.
    {
      let width = usize::from(self.pixmap.width());
      let data = self.pixmap.data_as_u8_slice_mut();

      for row in usize::from(dy)..usize::from(dy) + usize::from(dh) {
        let start = (row * width + usize::from(dx)) * 4;

        data[start..start + usize::from(dw) * 4].fill(0);
      }
    }

    let settings = vello_cpu::RasterizerSettings {
      target_init: vello_cpu::TargetInit::SrcOver,
      offset: (dx, dy),
      ..vello_cpu::RasterizerSettings::default()
    };

    ctx.render_with(&mut self.pixmap, &mut self.resources, settings);

    let rastered = started.elapsed();

    #[expect(clippy::cast_possible_truncation, reason = "microseconds")]
    Cost {
      record: recorded.as_micros() as u64,
      raster: (rastered - recorded).as_micros() as u64,
      glyphs: glyph_time.as_micros() as u64,
      masks: self
        .masks
        .as_ref()
        .map_or((0, 0), |masks| masks.rasterizer().lookups()),
      images: self.glyphs.lookups(),
      presented: false,
    }
  }
}
