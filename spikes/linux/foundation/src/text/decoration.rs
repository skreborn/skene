//! The decoration painter's geometry, shared by every engine: the lines a run's decoration
//! draws, from the metrics its engine reports.

use crate::{Color, text::RunExtent, value::TextDecorationStyle};

/// One decoration line: a horizontal band and how it is drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DecorationLine {
  /// Its left edge.
  pub x: f32,
  /// Its top.
  pub y: f32,
  /// Its length.
  pub width: f32,
  /// Its thickness.
  pub thickness: f32,
  /// How it is drawn: a double line comes as two solid ones.
  pub style: TextDecorationStyle,
  /// Its color.
  pub color: Color,
}

impl RunExtent {
  /// The lines this stretch's decoration draws: under, through and over the text, in the
  /// decoration's color and style, each at least a pixel thick.
  #[must_use]
  pub fn decoration_lines(&self) -> Vec<DecorationLine> {
    let paint = &self.paint;
    let decoration = paint.decoration;
    let color = paint.line_color();
    let mut lines = Vec::new();
    let mut add = |top: f32, thickness: f32| {
      let thickness = thickness.max(1.);
      let line = |y: f32, style| DecorationLine {
        x: self.x,
        y,
        width: self.width,
        thickness,
        style,
        color,
      };

      match paint.decoration_style {
        TextDecorationStyle::Double => {
          lines.push(line(top, TextDecorationStyle::Solid));
          lines.push(line(top + thickness * 2., TextDecorationStyle::Solid));
        }
        style => lines.push(line(top, style)),
      }
    };

    if decoration.underline {
      add(
        self.baseline - self.underline_offset,
        self.underline_thickness,
      );
    }

    if decoration.line_through {
      add(
        self.baseline - self.strikethrough_offset,
        self.strikethrough_thickness,
      );
    }

    if decoration.overline {
      add(self.baseline - self.ascent, self.underline_thickness);
    }

    lines
  }
}
