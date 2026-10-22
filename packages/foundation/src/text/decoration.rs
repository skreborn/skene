//! The decoration painter's geometry, shared by every engine: the lines a run's decoration
//! draws, from the metrics its engine reports.

use crate::{Color, TextDecoration, TextDecorationStyle, text::RunExtent};

/// One decoration line: a horizontal band and how it is drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct DecorationLine {
  /// Its left edge.
  pub x: f32,
  /// Its top.
  pub y: f32,
  /// Its length.
  pub width: f32,
  /// Its thickness.
  pub thickness: f32,
  /// How it is drawn: never [`TextDecorationStyle::Double`], which comes as two solid lines.
  pub style: TextDecorationStyle,
  /// Its color.
  pub color: Color,
}

impl RunExtent {
  /// The lines this stretch's decoration draws under, through or over the text, in the
  /// decoration's color and style, each at least a unit thick: none undecorated, two for a
  /// double line (the second below the first, above it for an overline), one otherwise.
  #[must_use]
  pub fn decoration_lines(&self) -> Vec<DecorationLine> {
    let paint = &self.paint;
    let (top, thickness) = match paint.decoration {
      TextDecoration::None => return Vec::new(),
      TextDecoration::Underline => (
        self.baseline - self.underline_offset,
        self.underline_thickness,
      ),
      TextDecoration::LineThrough => (
        self.baseline - self.strikethrough_offset,
        self.strikethrough_thickness,
      ),
      TextDecoration::Overline => (self.baseline - self.ascent, self.underline_thickness),
    };
    let thickness = thickness.max(1.);
    let line = |y: f32, style| DecorationLine {
      x: self.x,
      y,
      width: self.width,
      thickness,
      style,
      color: paint.line_color(),
    };

    let second = match paint.decoration {
      TextDecoration::Overline => top - thickness * 2.,
      _ => top + thickness * 2.,
    };

    match paint.decoration_style {
      TextDecorationStyle::Double => vec![
        line(top, TextDecorationStyle::Solid),
        line(second, TextDecorationStyle::Solid),
      ],
      style => vec![line(top, style)],
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    Color,
    TextDecoration,
    TextDecorationStyle,
    text::{PaintStyle, RunExtent},
  };

  fn extent(decoration: TextDecoration, style: TextDecorationStyle) -> RunExtent {
    RunExtent {
      range: 0..5,
      line: 0,
      x: 4.,
      width: 50.,
      baseline: 15.,
      ascent: 12.,
      descent: 4.,
      underline_offset: -2.,
      underline_thickness: 0.5,
      strikethrough_offset: 5.,
      strikethrough_thickness: 1.5,
      paint: PaintStyle::new(Color::oklch(0., 0., 0.)).with_decoration(decoration, style),
    }
  }

  fn bands(lines: &[super::DecorationLine]) -> Vec<(f32, f32, TextDecorationStyle)> {
    lines
      .iter()
      .map(|line| (line.y, line.thickness, line.style))
      .collect()
  }

  #[test]
  fn each_decoration_draws_its_line_where_the_metrics_put_it() {
    use TextDecorationStyle::{Dashed, Double, Solid, Wavy};

    let none = extent(TextDecoration::None, Solid);
    let under = extent(TextDecoration::Underline, Wavy);
    let through = extent(TextDecoration::LineThrough, Dashed);
    let over = extent(TextDecoration::Overline, Double);

    assert_eq!(none.decoration_lines(), []);
    // Beneath the baseline, thickened to a unit.
    assert_eq!(bands(&under.decoration_lines()), [(17., 1., Wavy)]);
    assert_eq!(bands(&through.decoration_lines()), [(10., 1.5, Dashed)]);
    // At the ascent, a double line as two solid ones, the second above the first.
    assert_eq!(
      bands(&over.decoration_lines()),
      [(3., 1., Solid), (1., 1., Solid)]
    );

    let double_under = extent(TextDecoration::Underline, Double);

    assert_eq!(
      bands(&double_under.decoration_lines()),
      [(17., 1., Solid), (19., 1., Solid)],
      "the second line of a double underline is below the first"
    );

    let line = under.decoration_lines()[0];

    assert_eq!((line.x, line.width), (4., 50.));
    assert_eq!(
      line.color, under.paint.color,
      "the glyphs' color where none"
    );
  }
}
