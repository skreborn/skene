//! Drawing a laid paragraph through vello: its glyphs as outlines, each in its paint's color,
//! and its decorations as the shared painter's lines ([`DecorationLine`]).

use skene_foundation::{
  TextDecorationStyle,
  text::{DecorationLine, Laid as _},
};
use skrifa::{
  FontRef,
  GlyphId,
  MetadataProvider as _,
  instance::{LocationRef, NormalizedCoord, Size as FontSize},
  outline::{DrawSettings, OutlinePen},
  raw::types::F2Dot14,
};
use vello_cpu::{
  RenderContext,
  kurbo::{Affine, BezPath, Point, Rect, Shape as _, Stroke},
};

use crate::ParleyLaid;

impl ParleyLaid {
  /// Draws this paragraph's glyphs as vello outlines, its top left at `origin`, each stretch in its
  /// paint's color, then its decorations ([`Self::paint_decorations`]); a glyph with no outline
  /// draws nothing.
  pub fn paint_outlines(&self, context: &mut RenderContext, origin: (f64, f64)) {
    context.set_transform(Affine::translate(origin));

    for run in self.glyph_runs() {
      let Ok(font) = FontRef::from_index(run.font.data.data(), run.font.index)
      else {
        continue;
      };
      let outlines = font.outline_glyphs();
      let coords: Vec<NormalizedCoord> = run
        .coords
        .iter()
        .map(|bits| F2Dot14::from_bits(*bits))
        .collect();
      let mut start = 0;

      while start < run.glyphs.len() {
        let paint = self.paint_at(run.glyphs[start].at);
        let end = run.glyphs[start..]
          .iter()
          .position(|glyph| self.paint_at(glyph.at) != paint)
          .map_or(run.glyphs.len(), |offset| start + offset);
        let mut pen = Pen::default();

        for glyph in &run.glyphs[start..end] {
          (pen.x, pen.y) = (glyph.x, glyph.y);

          if let Some(outline) = outlines.get(GlyphId::new(glyph.id)) {
            let _ = outline.draw(
              DrawSettings::unhinted(FontSize::new(run.size), LocationRef::new(&coords)),
              &mut pen,
            );
          }
        }

        if !pen.path.elements().is_empty() {
          context.set_paint(paint.color.to_alpha_color());
          context.fill_path(&pen.path);
        }

        start = end;
      }
    }

    self.paint_decorations(context);
  }

  /// Draws this paragraph's decorations (underline, line-through, overline, in their styles and
  /// colors) through the shared painter's geometry, under the current transform.
  pub fn paint_decorations(&self, context: &mut RenderContext) {
    for extent in self.run_extents() {
      for line in extent.decoration_lines() {
        context.set_paint(line.color.to_alpha_color());

        match DecorationShape::of(&line) {
          DecorationShape::Fill(path) => context.fill_path(&path),
          DecorationShape::Stroke(path, stroke) => {
            context.set_stroke(stroke);
            context.stroke_path(&path);
          }
        }
      }
    }
  }
}

/// The outlines of a stretch of glyphs, each drawn at its pen position.
#[derive(Default)]
struct Pen {
  path: BezPath,
  x: f32,
  y: f32,
}

impl Pen {
  /// The layout's point for the font's (`x`, `y`), which grows upward.
  fn point(&self, x: f32, y: f32) -> Point {
    Point::new(f64::from(self.x + x), f64::from(self.y - y))
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

/// What a decoration line draws, in the layout's units: a filled path (solid, dotted,
/// dashed), or a stroked one (wavy).
#[derive(Clone, Debug)]
#[must_use]
pub enum DecorationShape {
  /// The path filled.
  Fill(BezPath),
  /// The path stroked.
  Stroke(BezPath, Stroke),
}

impl DecorationShape {
  /// The shape `line` draws in its style.
  pub fn of(line: &DecorationLine) -> Self {
    let (x, y, width, thickness) = (
      f64::from(line.x),
      f64::from(line.y),
      f64::from(line.width),
      f64::from(line.thickness),
    );
    let mut path = BezPath::new();

    match line.style {
      TextDecorationStyle::Solid | TextDecorationStyle::Double => {
        path.extend(Rect::new(x, y, x + width, y + thickness).path_elements(0.1));

        Self::Fill(path)
      }
      TextDecorationStyle::Dotted | TextDecorationStyle::Dashed => {
        let (on, off) = if line.style == TextDecorationStyle::Dotted {
          (thickness, thickness)
        }
        else {
          (thickness * 3., thickness * 2.)
        };
        let mut at = x;

        while at < x + width {
          path.extend(Rect::new(at, y, (at + on).min(x + width), y + thickness).path_elements(0.1));
          at += on + off;
        }

        Self::Fill(path)
      }
      TextDecorationStyle::Wavy => {
        let step = thickness * 2.;
        let mut at = x;
        let mut up = true;

        path.move_to((at, y + thickness));

        while at < x + width {
          let next = (at + step).min(x + width);

          path.line_to((next, if up { y } else { y + thickness * 2. }));
          at = next;
          up = !up;
        }

        Self::Stroke(path, Stroke::new(thickness))
      }
    }
  }
}
