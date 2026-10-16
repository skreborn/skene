//! Painting the realized tree into a frame: each box's shadows, fill, border, its content (a
//! text, a control), its children clipped where it clips, through vello_cpu's paths, and every
//! text's glyphs composited afterwards as FreeType masks under the reader's font settings.

use std::rc::Rc;

use skene_foundation::{
  Color, Corners,
  layout::Rect,
  seam::node::{LayoutKind, NodeKind},
  text::Laid as _,
  value::BorderStyle,
};
use skene_parley::{ParleyLaid, render::MaskPainter};
use vello_cpu::{
  Pixmap, RenderContext, Resources,
  kurbo::{
    Affine, BezPath, Circle, Rect as KRect, RoundedRect, RoundedRectRadii, Shape as _, Stroke,
  },
  peniko::Color as PColor,
};

use crate::{node::Node, palette::Palette, text};

/// A Skene color as vello's.
pub fn color(color: Color) -> PColor {
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

/// A text queued for the mask pass: its layout, where its top left is (window pixels), the
/// rectangle it is cut to, and the opacity it is drawn at.
struct TextDraw {
  laid: ParleyLaid,
  origin: (f32, f32),
  clip: Rect,
  opacity: f32,
}

/// What one frame paints with: the vello context, the scale and the offset from window
/// pixels to the frame's device pixels, the clip and opacity in force, the texts queued.
pub struct Canvas<'a> {
  /// The vello context, in device pixels.
  pub ctx: &'a mut RenderContext,
  scale: f64,
  offset: (f64, f64),
  clip: Rect,
  opacity: f32,
  texts: Vec<TextDraw>,
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

    self.ctx.set_transform(self.to_device());
    self.ctx.set_paint(color(fill));

    if corners == Corners::all(0.) {
      self.ctx.fill_rect(&krect(rect));
    } else {
      self
        .ctx
        .fill_path(&RoundedRect::from_rect(krect(rect), radii(corners, rect)).to_path(0.1));
    }
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

    self.ctx.set_transform(self.to_device());
    self.ctx.set_paint(color(ink));
    self.ctx.set_stroke(stroke);
    self.ctx.stroke_path(&path);
  }

  /// Fills a circle centered at `center`.
  pub fn circle(&mut self, center: (f32, f32), radius: f32, fill: Color) {
    self.ctx.set_transform(self.to_device());
    self.ctx.set_paint(color(fill));
    self.ctx.fill_path(
      &Circle::new(
        (f64::from(center.0), f64::from(center.1)),
        f64::from(radius),
      )
      .to_path(0.1),
    );
  }

  /// Strokes `path` (window pixels) `width` wide.
  pub fn stroke_path(&mut self, path: &BezPath, width: f32, ink: Color) {
    self.ctx.set_transform(self.to_device());
    self.ctx.set_paint(color(ink));
    self.ctx.set_stroke(Stroke::new(f64::from(width)));
    self.ctx.stroke_path(path);
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

    self.ctx.set_transform(self.to_device());
    self.ctx.set_paint(color(ink));
    self
      .ctx
      .fill_blurred_rounded_rect(&krect(rect), radius.max(0.), (blur / 2.).max(0.01), false);
  }

  /// Draws `laid` with its top left at `origin` (window pixels): its decorations now, its
  /// glyphs in the mask pass, both cut to the clip in force.
  pub fn text(&mut self, laid: &ParleyLaid, origin: (f32, f32)) {
    let mut transform = self.to_device();

    transform *= Affine::translate((f64::from(origin.0), f64::from(origin.1)));
    self.ctx.set_transform(transform);
    skene_parley::render::paint_decorations(self.ctx, laid);
    self.texts.push(TextDraw {
      laid: laid.clone(),
      origin,
      clip: self.clip,
      opacity: self.opacity,
    });
  }

  /// Runs `body` with what it draws cut to `rect` (window pixels).
  pub fn clipped(&mut self, rect: Rect, body: impl FnOnce(&mut Self)) {
    let previous = self.push_clip(rect, Corners::all(0.));

    body(self);
    self.ctx.pop_layer();
    self.clip = previous;
  }

  fn push_clip(&mut self, rect: Rect, corners: Corners) -> Rect {
    let previous = self.clip;

    self.clip = intersect(self.clip, rect);
    self.ctx.set_transform(self.to_device());

    if corners == Corners::all(0.) {
      self.ctx.push_clip_layer(&krect(rect).to_path(0.1));
    } else {
      self
        .ctx
        .push_clip_layer(&RoundedRect::from_rect(krect(rect), radii(corners, rect)).to_path(0.1));
    }

    previous
  }
}

/// How much farther than its box a node may paint: its outer shadows and its focus ring.
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

  shadows.max(4.)
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

/// The node with focus that shows a ring, and the node the pointer presses: what the painter
/// draws differently from the node's own states.
#[derive(Clone, Default)]
pub struct Marks {
  /// The focused node, where focus shows.
  pub focus: Option<Node>,
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
  let rect = Rect {
    x: origin.0 + frame.x,
    y: origin.1 + frame.y,
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
    let (sx, sy) = node.scroll.get();

    paint_children(node, canvas, (rect.x - sx, rect.y - sy), region, marks);

    return;
  }

  let look = node.look.borrow().clone();

  let opacity_layer = look.opacity < 1.;
  let previous_opacity = canvas.opacity;

  if opacity_layer {
    canvas.ctx.push_opacity_layer(look.opacity);
    canvas.opacity *= look.opacity;
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

  if let Some(background) = look.background {
    canvas.fill(rect, look.corners, background);
  }

  if let Some(shadows) = &look.shadows
    && shadows.0.iter().any(|shadow| shadow.inset)
  {
    let previous = canvas.push_clip(rect, look.corners);

    for shadow in shadows.0.iter().filter(|shadow| shadow.inset) {
      canvas.ctx.set_transform(canvas.to_device());
      canvas.ctx.set_paint(color(shadow.color));
      canvas.ctx.fill_blurred_rounded_rect(
        &krect(Rect {
          x: rect.x + shadow.x + shadow.spread,
          y: rect.y + shadow.y + shadow.spread,
          width: rect.width - shadow.spread * 2.,
          height: rect.height - shadow.spread * 2.,
        }),
        look.corners.top_leading,
        (shadow.blur / 2.).max(0.01),
        true,
      );
    }

    canvas.ctx.pop_layer();
    canvas.clip = previous;
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
          for run in laid.range_rects(link.clone()) {
            let run = Rect {
              x: rect.x + run.x,
              y: rect.y + run.y,
              ..run
            };

            focus_ring(canvas, run, Corners::all(2.));
          }
        }
      }
    }
    NodeKind::Control(kind) => node.paint_control(kind, canvas, rect, marks),
    _ => {}
  }

  let clips = look.clip || scrolls;
  let previous_clip = clips.then(|| canvas.push_clip(rect, look.corners));
  let (sx, sy) = node.scroll.get();

  paint_children(node, canvas, (rect.x - sx, rect.y - sy), region, marks);

  if scrolls {
    node.paint_scroll_bar(canvas, rect);
  }

  if let Some(previous) = previous_clip {
    canvas.ctx.pop_layer();
    canvas.clip = previous;
  }

  if let Some(border) = look.border {
    let ink = look
      .border_color
      .or_else(|| node.run.borrow().color)
      .unwrap_or(canvas.palette.ink);

    canvas.border(rect, look.corners, border.width, border.style, ink);
  }

  if marks
    .focus
    .as_ref()
    .is_some_and(|focused| Rc::ptr_eq(focused, node))
    && !matches!(node.spec.kind, NodeKind::Control(_) | NodeKind::Text)
    && !node
      .interest
      .get()
      .states
      .contains(skene_foundation::State::FocusVisible)
  {
    focus_ring(canvas, rect, look.corners);
  }

  if opacity_layer {
    canvas.ctx.pop_layer();
    canvas.opacity = previous_opacity;
  }
}

/// The ring a focused node shows where focus shows and its style does not answer it.
pub fn focus_ring(canvas: &mut Canvas<'_>, rect: Rect, corners: Corners) {
  let grown = Rect {
    x: rect.x - 2.,
    y: rect.y - 2.,
    width: rect.width + 4.,
    height: rect.height + 4.,
  };
  let corners = Corners {
    top_leading: corners.top_leading + 2.,
    top_trailing: corners.top_trailing + 2.,
    bottom_trailing: corners.bottom_trailing + 2.,
    bottom_leading: corners.bottom_leading + 2.,
  };
  let accent = canvas.palette.accent;

  canvas.border(grown, corners, 2., BorderStyle::Solid, accent);
}

fn paint_children(
  node: &Node,
  canvas: &mut Canvas<'_>,
  origin: (f32, f32),
  region: Rect,
  marks: &Marks,
) {
  for child in node.children() {
    paint_node(&child, canvas, origin, region, marks);
  }
}

/// The frame's renderer: its vello context and resources, the mask painter, the frame's
/// pixels.
pub struct Renderer {
  ctx: Option<RenderContext>,
  resources: Resources,
  masks: Option<MaskPainter>,
  /// The frame: premultiplied RGBA, in device pixels.
  pub pixmap: Pixmap,
}

/// What one frame cost, in microseconds.
#[derive(Clone, Copy, Default, Debug)]
pub struct Cost {
  /// Recording the paths.
  pub record: u64,
  /// Rasterizing them.
  pub raster: u64,
  /// Compositing the glyph masks.
  pub glyphs: u64,
}

impl Default for Renderer {
  fn default() -> Self {
    Self::new()
  }
}

impl Renderer {
  /// A renderer with no frame yet.
  pub fn new() -> Self {
    Self {
      ctx: None,
      resources: Resources::new(),
      masks: MaskPainter::new(),
      pixmap: Pixmap::new(1, 1),
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
    let started = std::time::Instant::now();

    if self.pixmap.width() != size.0 || self.pixmap.height() != size.1 {
      self.pixmap = Pixmap::new(size.0, size.1);
    }

    // The damage in device pixels, whole pixels around it.
    let device = |value: f32| f64::from(value) * scale;
    let left = device(region.x + origin.0).floor().max(0.);
    let top = device(region.y + origin.1).floor().max(0.);
    let right = device(region.x + origin.0 + region.width)
      .ceil()
      .min(f64::from(size.0));
    let bottom = device(region.y + origin.1 + region.height)
      .ceil()
      .min(f64::from(size.1));

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
    let mut canvas = Canvas {
      ctx,
      scale,
      offset: (left - device(origin.0), top - device(origin.1)),
      clip: window_region,
      opacity: 1.,
      texts: Vec::new(),
      palette: Palette::now(),
    };

    chrome(&mut canvas);

    if let Some(root) = root {
      paint_node(root, &mut canvas, (0., 0.), window_region, marks);
    }

    let texts = std::mem::take(&mut canvas.texts);
    let recorded = started.elapsed();

    canvas.ctx.flush();

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

    ctx_render(
      self.ctx.as_ref().expect("made above"),
      &mut self.pixmap,
      &mut self.resources,
      settings,
    );

    let rastered = started.elapsed();

    if let Some(masks) = &mut self.masks {
      let engine = text::engine();
      let mut fallback = engine.fallback();

      for draw in texts {
        let clip = intersect(draw.clip, window_region);

        if clip.width <= 0. || clip.height <= 0. {
          continue;
        }

        #[expect(clippy::cast_possible_truncation, reason = "pixels")]
        let device_clip = (
          (f64::from(clip.x + origin.0) * scale).floor() as i64,
          (f64::from(clip.y + origin.1) * scale).floor() as i64,
          (f64::from(clip.x + origin.0 + clip.width) * scale).ceil() as i64,
          (f64::from(clip.y + origin.1 + clip.height) * scale).ceil() as i64,
        );

        masks.paint_with(
          &mut self.pixmap,
          &draw.laid,
          (
            ((f64::from(draw.origin.0 + origin.0)) * scale) as f32,
            ((f64::from(draw.origin.1 + origin.1)) * scale) as f32,
          ),
          skene_parley::render::MaskOptions {
            scale: scale as f32,
            clip: Some(device_clip),
            opacity: draw.opacity,
          },
          fallback.as_deref_mut(),
        );
      }
    }

    let done = started.elapsed();

    #[expect(clippy::cast_possible_truncation, reason = "microseconds")]
    Cost {
      record: recorded.as_micros() as u64,
      raster: (rastered - recorded).as_micros() as u64,
      glyphs: (done - rastered).as_micros() as u64,
    }
  }
}

fn ctx_render(
  ctx: &RenderContext,
  pixmap: &mut Pixmap,
  resources: &mut Resources,
  settings: vello_cpu::RasterizerSettings,
) {
  ctx.render_with(pixmap, resources, settings);
}
