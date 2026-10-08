//! The scene drawn as pixels, as a browser paints the page: each box's background and
//! border, rounded by its corner radius, what a box clipping its overflow holds clipped
//! to its padding box, each run's text in the pinned face with its decoration, a text
//! entry's value, each painted box's picture, and the open overlays above the rest.
//!
//! A border is drawn solid whatever its style, and a decoration as one solid line.

use std::collections::VecDeque;

use rustc_hash::FxHashMap;
use skene_foundation::{
  Canvas,
  Color,
  IntoBrush,
  Layout,
  NativeElement,
  NodeId,
  Overflow,
  Rect,
  Renderer,
  RunPlacement,
  Scene,
  Size,
  StaticLeaf,
  TextDecoration,
  kurbo::{self, Affine, BezPath, Shape as _},
  paint,
  peniko,
  typography,
};
use skene_layout::Frames;
use skrifa::{
  GlyphId,
  MetadataProvider,
  outline::{DrawSettings, OutlinePen},
  prelude::{LocationRef, Size as FontSize},
};
use vello_cpu::{Pixmap, RenderContext, RenderSettings, Resources};

use crate::{
  Headless,
  font::Pinned,
  layout::{self, Measured},
  records::Kind,
  resolve::{Env, Records},
  with_records,
};

/// The pixels a render drew: its size, and each pixel's unpremultiplied red, green, blue
/// and alpha, rows top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct Image {
  /// The width, in pixels.
  pub width: u16,
  /// The height, in pixels.
  pub height: u16,
  /// Four bytes a pixel, rows top to bottom.
  pub pixels: Vec<u8>,
}

impl Image {
  /// The pixel at column `x` and row `y`: its red, green, blue and alpha; `None` past
  /// the image's edges.
  #[must_use]
  pub fn pixel(&self, x: u16, y: u16) -> Option<[u8; 4]> {
    if x >= self.width || y >= self.height {
      return None;
    }

    let at = (usize::from(y) * usize::from(self.width) + usize::from(x)) * 4;

    self.pixels.get(at..at + 4)?.try_into().ok()
  }
}

/// The page's color beneath everything, as a browser's canvas is.
const PAGE: Color = Color::oklch(1., 0., 0.);

/// The ink of a run that resolves none.
const BLACK: Color = Color::oklch(0., 0., 0.);

/// Draws the scene laid out as `frames` on a white page `size` pixels large, without
/// SIMD, so every machine draws the same pixels.
#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "a test page's size is a small, non-negative number of pixels"
)]
pub(crate) fn render(frames: &Frames, size: Size<f32>) -> Image {
  let width = (size.width.ceil() as u16).max(1);
  let height = (size.height.ceil() as u16).max(1);
  let settings = RenderSettings {
    level: fearless_simd::Level::fallback(),
    num_threads: 0,
  };
  let mut drawing = Drawing {
    context: RenderContext::new_with(width, height, settings),
    frames,
    overlays: VecDeque::new(),
  };
  let page = kurbo::Rect::new(0., 0., f64::from(width), f64::from(height));

  drawing.fill(&page.to_path(0.1), PAGE);

  for root in Scene::<Headless>::read(|scene| scene.roots().collect::<Vec<_>>()) {
    drawing.node(root, false);
  }

  while let Some(overlay) = drawing.overlays.pop_front() {
    drawing.node(overlay, true);
  }

  let mut pixmap = Pixmap::new(width, height);

  drawing.context.flush();
  drawing.context.render(&mut pixmap, &mut Resources::new());

  Image {
    width,
    height,
    pixels: pixmap
      .take_unpremultiplied()
      .into_iter()
      .flat_map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
      .collect(),
  }
}

/// One render in progress: the context it draws into, the layout it draws from, and the
/// open overlays it set aside to draw above the rest.
struct Drawing<'a> {
  context: RenderContext,
  frames: &'a Frames,
  overlays: VecDeque<NodeId>,
}

/// What a record holds that its drawing is made from, read out of the records.
enum Drawn {
  Box {
    layout: Layout,
    style: Box<skene_foundation::StaticBoxStyle<Headless>>,
    overlay: bool,
    entry: Option<(String, Option<String>)>,
    painted: bool,
  },
  Run(String),
  Transparent,
  Absent,
}

impl Drawing<'_> {
  /// Draws `node` with everything beneath it; an open overlay only as an overlay's own
  /// drawing, `overlay`, sets it aside for that otherwise.
  fn node(&mut self, node: NodeId, overlay: bool) {
    match Self::shape(node) {
      Drawn::Absent => {}
      Drawn::Transparent => self.children(node),
      Drawn::Run(text) => {
        if let Some(rect) = self.frames.rect(node) {
          self.text(&layout::lone(node, text), rect);
        }
      }
      Drawn::Box { overlay: true, .. } if !overlay => self.overlays.push_back(node),
      Drawn::Box {
        layout,
        style,
        entry,
        painted,
        ..
      } => self.decorated(node, layout, &style, entry, painted),
    }
  }

  /// What `node` is to the drawing.
  fn shape(node: NodeId) -> Drawn {
    let graphics = with_records(|records| records.graphics.unwrap_or(true));

    with_records(|records| {
      let Some(record) = records.record(node)
      else {
        return Drawn::Absent;
      };
      let entry = |props: &skene_foundation::ControlProps<Headless>| {
        matches!(
          props.native,
          Some(NativeElement::TextField | NativeElement::TextEditor)
        )
        .then(|| (record.value.clone(), props.placeholder.clone()))
      };

      match &record.kind {
        Kind::Box(props) => Drawn::Box {
          layout: props.layout,
          style: Box::new(props.style.clone()),
          overlay: false,
          entry: None,
          painted: graphics && record.painter.is_some(),
        },
        Kind::Control(props) => Drawn::Box {
          layout: props.box_props.layout,
          style: Box::new(props.box_props.style.clone()),
          overlay: false,
          entry: entry(props),
          painted: false,
        },
        Kind::Overlay(props) if record.open => Drawn::Box {
          layout: props.box_props.layout,
          style: Box::new(props.box_props.style.clone()),
          overlay: true,
          entry: None,
          painted: false,
        },
        Kind::Overlay(_) => Drawn::Absent,
        Kind::Run(props) if props.placement == RunPlacement::Embedded => Drawn::Transparent,
        Kind::Run(_) => Drawn::Run(record.text.clone()),
        Kind::Anchor | Kind::Scope => Drawn::Transparent,
      }
    })
  }

  /// Draws every child `node` places.
  fn children(&mut self, node: NodeId) {
    for child in layout::placed_children(node) {
      self.node(child, false);
    }
  }

  /// Draws the box `node`: its background and border, then its picture, its entry's
  /// text, its paragraph or its children, clipped where it clips its overflow.
  fn decorated(
    &mut self,
    node: NodeId,
    layout: Layout,
    style: &skene_foundation::StaticBoxStyle<Headless>,
    entry: Option<(String, Option<String>)>,
    painted: bool,
  ) {
    let (Some(rect), Some(env)) = (self.frames.rect(node), Env::of(&Records, node))
    else {
      return;
    };
    let style = layout::presented(node, &env, style);

    if style.hidden == Some(true) {
      return;
    }

    let border = style.border_width.unwrap_or_default();
    let padding = style.padding.unwrap_or_default();
    let radius = f64::from(style.ext.corner_radius.map_or(0., |radius| radius.0));
    let outer = rounded(rect, radius);
    let inner = rounded(
      inset(rect, border),
      (radius - f64::from(widest(border))).max(0.),
    );

    if let Some(background) = style.background {
      self.fill(&outer, background);
    }

    if let Some(color) = style.border_color
      && border != skene_foundation::Inset::default()
    {
      let mut ring = outer.clone();

      ring.extend(inner.iter());
      self.context.set_fill_rule(peniko::Fill::EvenOdd);
      self.fill(&ring, color);
      self.context.set_fill_rule(peniko::Fill::NonZero);
    }

    if painted {
      self.picture(node, rect);
    }

    let content = inset(inset(rect, border), padding);

    if let Some((value, placeholder)) = entry {
      self.entry(&env, content, &value, placeholder.as_deref());
    }

    let clips = style.overflow.is_some_and(|overflow| {
      overflow.horizontal != Overflow::Visible || overflow.vertical != Overflow::Visible
    });

    if clips {
      self.context.push_clip_layer(&inner);
    }

    match (layout == Layout::Document)
      .then(|| layout::paragraph(node))
      .flatten()
    {
      Some(paragraph) => self.text(&paragraph, content),
      None => self.children(node),
    }

    if clips {
      self.context.pop_layer();
    }
  }

  /// Draws the picture the painted box `node` paints at `rect`.
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a test box's size is a small, non-negative number of pixels"
  )]
  fn picture(&mut self, node: NodeId, rect: Rect<f32>) {
    let Some(painter) = with_records(|records| records.record(node)?.painter.clone())
    else {
      return;
    };
    let mut canvas = Canvas::new(rect.size.width as u16, rect.size.height as u16);

    painter(&mut canvas, rect.size);

    let origin = Affine::translate((f64::from(rect.origin.x), f64::from(rect.origin.y)));

    self.context.push_clip_layer(&rounded(rect, 0.));
    self.context.set_transform(origin);
    canvas.replay(&mut Placed {
      context: &mut self.context,
      origin,
    });
    self.context.reset_transform();
    self.context.pop_layer();
  }

  /// Draws a text entry's `value` in its content box at `content`, or its `placeholder`
  /// where it holds none, a line to each of its lines.
  fn entry(&mut self, env: &Env, content: Rect<f32>, value: &str, placeholder: Option<&str>) {
    let (size, line_height) = layout::typography(env);
    let ink = env.with_resolver(|resolver| {
      resolver
        .keyframe_run_value(&StaticLeaf::from(typography::INK))
        .unwrap_or(BLACK)
    });
    let (text, color) = match placeholder {
      // Chromium's placeholder gray, #757575.
      Some(placeholder) if value.is_empty() => (placeholder, Color::srgb(0.459, 0.459, 0.459)),
      _ => (value, ink),
    };
    let height = size * line_height;
    let pinned = Pinned::get();

    for (index, line) in text.split('\n').enumerate() {
      #[expect(
        clippy::cast_precision_loss,
        reason = "an entry's line count stays far below the integers f32 holds exactly"
      )]
      let top = content.origin.y + height * index as f32;

      self.glyphs(
        line,
        content.origin.x,
        top + pinned.baseline(height, size),
        size,
        color,
      );
    }
  }

  /// Draws `paragraph` laid out in a content box at `content`: each cluster's glyphs
  /// in its run's ink and size, each run's decoration, and each inline box.
  fn text(&mut self, paragraph: &Measured, content: Rect<f32>) {
    let mut inks: FxHashMap<NodeId, Ink> = FxHashMap::default();
    let pinned = Pinned::get();

    for placed in paragraph.clusters(content) {
      if placed.text.is_empty() {
        self.node(placed.node, false);

        continue;
      }

      if matches!(placed.text, "\n" | "\r\n") {
        continue;
      }

      let ink = *inks
        .entry(placed.node)
        .or_insert_with(|| Ink::of(placed.node));
      let rect = placed.rect;
      let baseline = rect.origin.y + pinned.baseline(rect.size.height, ink.size);

      self.glyphs(placed.text, rect.origin.x, baseline, ink.size, ink.color);

      let line = match ink.decoration {
        TextDecoration::None => continue,
        TextDecoration::Underline => pinned.underline(ink.size),
        TextDecoration::LineThrough => pinned.strikeout(ink.size),
        TextDecoration::Overline => {
          let underline = pinned.underline(ink.size);

          crate::font::Line {
            offset: pinned.ascent(ink.size),
            thickness: underline.thickness,
          }
        }
      };
      let top = f64::from(baseline - line.offset);
      let stroke = kurbo::Rect::new(
        f64::from(rect.origin.x),
        top,
        f64::from(rect.origin.x + rect.size.width),
        top + f64::from(line.thickness.max(1.)),
      );

      self.fill(&stroke.to_path(0.1), ink.decoration_color);
    }
  }

  /// Draws `text`'s glyphs from `x` along the baseline `baseline` at `size` in `color`.
  fn glyphs(&mut self, text: &str, x: f32, baseline: f32, size: f32, color: Color) {
    let font = Pinned::font();
    let charmap = font.charmap();
    let outlines = font.outline_glyphs();
    let pinned = Pinned::get();
    let mut pen = Pen {
      path: BezPath::new(),
      x,
      y: baseline,
    };

    for character in text.chars() {
      let glyph = charmap.map(character).unwrap_or(GlyphId::NOTDEF);

      if let Some(outline) = outlines.get(glyph) {
        let _ = outline.draw(
          DrawSettings::unhinted(FontSize::new(size), LocationRef::default()),
          &mut pen,
        );
      }

      pen.x += pinned.advance_of(character, size);
    }

    if !pen.path.elements().is_empty() {
      self.fill(&pen.path, color);
    }
  }

  /// Fills `path` with `color`.
  fn fill(&mut self, path: &BezPath, color: Color) {
    self.context.set_paint(paint(&color.into_brush()));
    self.context.fill_path(path);
  }
}

/// How a run's text is drawn: its size, its ink, and its decoration with that line's
/// color.
#[derive(Clone, Copy)]
struct Ink {
  size: f32,
  color: Color,
  decoration: TextDecoration,
  decoration_color: Color,
}

impl Ink {
  /// How the run `node` draws its text, its ink the typography's where it sets none and
  /// a link's underlined where its decoration says nothing.
  fn of(node: NodeId) -> Self {
    let size = layout::metrics(node).size;
    let props = with_records(|records| match &records.record(node)?.kind {
      Kind::Run(props) => Some((props.style.clone(), props.link.is_some())),
      _ => None,
    });
    let resolved = Env::of(&Records, node)
      .zip(props)
      .map(|(env, (style, link))| {
        env.with_resolver(|resolver| {
          let resolved = resolver.run_style(&style);
          let fallback = if link {
            typography::LINK_INK
          }
          else {
            typography::INK
          };
          let color = resolved
            .color
            .or_else(|| resolver.keyframe_run_value(&StaticLeaf::from(fallback)))
            .unwrap_or(BLACK);
          let decoration = resolved.decoration.unwrap_or(if link {
            TextDecoration::Underline
          }
          else {
            TextDecoration::None
          });

          (
            color,
            decoration,
            resolved.decoration_color.unwrap_or(color),
          )
        })
      });
    let (color, decoration, decoration_color) =
      resolved.unwrap_or((BLACK, TextDecoration::None, BLACK));

    Self {
      size,
      color,
      decoration,
      decoration_color,
    }
  }
}

/// A recorded picture replayed at a box's origin: each transform it sets composed after
/// the move there.
struct Placed<'a> {
  context: &'a mut RenderContext,
  origin: Affine,
}

impl Renderer for Placed<'_> {
  fn set_transform(&mut self, transform: Affine) {
    self.context.set_transform(self.origin * transform);
  }

  fn fill(&mut self, path: &BezPath, brush: &peniko::Brush) {
    Renderer::fill(self.context, path, brush);
  }

  fn stroke(&mut self, path: &BezPath, stroke: &kurbo::Stroke, brush: &peniko::Brush) {
    Renderer::stroke(self.context, path, stroke, brush);
  }

  fn push_clip(&mut self, path: &BezPath) {
    Renderer::push_clip(self.context, path);
  }

  fn pop(&mut self) {
    Renderer::pop(self.context);
  }
}

/// A glyph's outline collected as a path: the face's coordinates, up from the baseline,
/// turned to the page's, down from its top, from the pen's origin.
struct Pen {
  path: BezPath,
  x: f32,
  y: f32,
}

impl Pen {
  /// The page's point for the face's (`x`, `y`).
  fn point(&self, x: f32, y: f32) -> kurbo::Point {
    kurbo::Point::new(f64::from(self.x + x), f64::from(self.y - y))
  }
}

impl OutlinePen for Pen {
  fn move_to(&mut self, x: f32, y: f32) {
    let point = self.point(x, y);

    self.path.move_to(point);
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let point = self.point(x, y);

    self.path.line_to(point);
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    let (control, point) = (self.point(cx0, cy0), self.point(x, y));

    self.path.quad_to(control, point);
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    let (first, second, point) = (self.point(cx0, cy0), self.point(cx1, cy1), self.point(x, y));

    self.path.curve_to(first, second, point);
  }

  fn close(&mut self) {
    self.path.close_path();
  }
}

/// `rect` as a path, its corners rounded by `radius`.
fn rounded(rect: Rect<f32>, radius: f64) -> BezPath {
  kurbo::RoundedRect::new(
    f64::from(rect.origin.x),
    f64::from(rect.origin.y),
    f64::from(rect.origin.x + rect.size.width),
    f64::from(rect.origin.y + rect.size.height),
    radius,
  )
  .to_path(0.1)
}

/// `rect` with `inset` taken off each edge, empty where nothing is left.
fn inset(rect: Rect<f32>, inset: skene_foundation::Inset<f32>) -> Rect<f32> {
  Rect {
    origin: skene_foundation::Point::new(rect.origin.x + inset.start, rect.origin.y + inset.top),
    size: Size::new(
      (rect.size.width - inset.start - inset.end).max(0.),
      (rect.size.height - inset.top - inset.bottom).max(0.),
    ),
  }
}

/// The widest edge of `inset`.
const fn widest(inset: skene_foundation::Inset<f32>) -> f32 {
  inset.top.max(inset.end).max(inset.bottom).max(inset.start)
}
