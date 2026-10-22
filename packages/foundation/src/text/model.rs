//! The text model: a value, its runs and its inline boxes with every property resolved, and
//! the paragraph's own properties. An engine receives these concrete values; resolution
//! (tokens, conditions, derived values, motion) happens before.

use std::{ops::Range, rc::Rc};

use crate::{Color, Length, Plain, RunStyle, TextDecoration, TextDecorationStyle, TypeMetrics};

/// The typeface a run is set in.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub enum FontFamily {
  /// The platform's interface typeface.
  #[default]
  System,
  /// The platform's fixed-width typeface.
  Monospace,
  /// A typeface by its family name, the engine's first where it has none by that name.
  Named(Rc<str>),
}

/// A font's weight, from 1 to 1000.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct FontWeight(pub f32);

impl FontWeight {
  /// 400.
  pub const REGULAR: Self = Self(400.);
  /// 500.
  pub const MEDIUM: Self = Self(500.);
  /// 600.
  pub const SEMIBOLD: Self = Self(600.);
  /// 700.
  pub const BOLD: Self = Self(700.);
}

impl Default for FontWeight {
  #[inline(always)]
  fn default() -> Self {
    Self::REGULAR
  }
}

/// Whether a run is upright or slanted.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub enum FontSlant {
  /// Upright.
  #[default]
  Upright,
  /// Slanted, the typeface's italic where it has one.
  Italic,
}

/// What a run is painted with: the properties that never move a glyph, so changing them
/// repaints and never lays out ([`super::Paragraph::repaint`]).
#[derive(Clone, Copy, PartialEq, Hash, Debug)]
#[must_use]
pub struct PaintStyle {
  /// The glyphs' color.
  pub color: Color,
  /// The line the run is decorated with.
  pub decoration: TextDecoration,
  /// How the line is drawn.
  pub decoration_style: TextDecorationStyle,
  /// The line's color, the glyphs' where none.
  pub decoration_color: Option<Color>,
}

impl PaintStyle {
  /// Plain glyphs in `color`, undecorated.
  #[inline(always)]
  pub const fn new(color: Color) -> Self {
    Self {
      color,
      decoration: TextDecoration::None,
      decoration_style: TextDecorationStyle::Solid,
      decoration_color: None,
    }
  }

  /// This paint decorated with `decoration`, drawn as `style`.
  #[inline(always)]
  pub const fn with_decoration(
    self,
    decoration: TextDecoration,
    style: TextDecorationStyle,
  ) -> Self {
    Self {
      decoration,
      decoration_style: style,
      ..self
    }
  }

  /// The color the decoration line is drawn in.
  #[inline(always)]
  pub const fn line_color(&self) -> Color {
    match self.decoration_color {
      Some(color) => color,
      None => self.color,
    }
  }
}

impl Default for PaintStyle {
  #[inline(always)]
  fn default() -> Self {
    Self::new(Color::oklch(0., 0., 0.))
  }
}

/// A run's properties, resolved: what shapes it (the typeface, its size, weight and slant,
/// the spacing past each cluster), how tall its line box is, and what paints it.
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct TextStyle {
  /// The typeface's family.
  pub family: FontFamily,
  /// The font size, in the engine's units (pixels on a pixel platform, cells in the
  /// terminal).
  pub size: f32,
  /// The weight.
  pub weight: FontWeight,
  /// Upright or slanted.
  pub slant: FontSlant,
  /// Extra spacing past each cluster, in the engine's units.
  pub letter_spacing: f32,
  /// How tall the run's line box is, its glyphs centered in it; the paragraph's line height
  /// where none.
  pub line_height: Option<LineHeight>,
  /// What paints the run.
  pub paint: PaintStyle,
}

impl TextStyle {
  /// The platform's interface typeface at `size`, regular and upright, in black.
  #[inline(always)]
  pub const fn new(size: f32) -> Self {
    Self {
      family: FontFamily::System,
      size,
      weight: FontWeight::REGULAR,
      slant: FontSlant::Upright,
      letter_spacing: 0.,
      line_height: None,
      paint: PaintStyle::new(Color::oklch(0., 0., 0.)),
    }
  }

  /// The style `style` resolves to, its font size `size` where it sets none: its type
  /// metrics as its medium's [`TypeMetrics`] read them (regular, with no spacing past a
  /// cluster and the paragraph's line height where it sets none), and its color and
  /// decoration (black and undecorated where it sets none), in the platform's interface
  /// typeface, upright.
  pub fn resolved<X: TypeMetrics>(style: &RunStyle<Plain, X>, size: f32) -> Self {
    let paint = PaintStyle {
      color: style.color.unwrap_or(Color::oklch(0., 0., 0.)),
      decoration: style.decoration.unwrap_or(TextDecoration::None),
      decoration_style: style.decoration_style.unwrap_or(TextDecorationStyle::Solid),
      decoration_color: style.decoration_color,
    };

    Self {
      size: X::font_size::<Plain>(&style.ext).map_or(size, |size: Length| size.0),
      weight: X::font_weight::<Plain>(&style.ext).map_or(FontWeight::REGULAR, FontWeight),
      letter_spacing: X::letter_spacing::<Plain>(&style.ext).map_or(0., |spacing| spacing.0),
      line_height: X::line_height::<Plain>(&style.ext).map(LineHeight::Factor),
      paint,
      ..Self::new(size)
    }
  }

  /// This style in `family`.
  #[inline(always)]
  pub fn with_family(self, family: FontFamily) -> Self {
    Self { family, ..self }
  }

  /// This style at `weight`.
  #[inline(always)]
  pub fn with_weight(self, weight: FontWeight) -> Self {
    Self { weight, ..self }
  }

  /// This style in `slant`.
  #[inline(always)]
  pub fn with_slant(self, slant: FontSlant) -> Self {
    Self { slant, ..self }
  }

  /// This style with `spacing` past each cluster.
  #[inline(always)]
  pub fn with_letter_spacing(self, spacing: f32) -> Self {
    Self {
      letter_spacing: spacing,
      ..self
    }
  }

  /// This style's line box as tall as `height` says.
  #[inline(always)]
  pub fn with_line_height(self, height: LineHeight) -> Self {
    Self {
      line_height: Some(height),
      ..self
    }
  }

  /// This style painted with `paint`.
  #[inline(always)]
  pub fn with_paint(self, paint: PaintStyle) -> Self {
    Self { paint, ..self }
  }

  /// Whether `other` shapes and lays out its text exactly as this style does: only the
  /// paint differs.
  #[must_use]
  pub fn shapes_like(&self, other: &Self) -> bool {
    self.family == other.family
      && self.size.to_bits() == other.size.to_bits()
      && self.weight.0.to_bits() == other.weight.0.to_bits()
      && self.slant == other.slant
      && self.letter_spacing.to_bits() == other.letter_spacing.to_bits()
      && self.line_height == other.line_height
  }
}

/// One run of an attributed text: a byte range of its value, on character boundaries, and
/// the run's style.
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct TextRun {
  /// The bytes of the value the run covers.
  pub range: Range<usize>,
  /// The run's properties.
  pub style: TextStyle,
}

/// A box laid out among a text's characters, as wide and as tall as its content, standing on
/// its line's baseline: an embedded view.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct InlineBox {
  /// The byte of the value the box stands before, on a character boundary; the value's
  /// length for a box at its end.
  pub index: usize,
  /// Its width, in the engine's units.
  pub width: f32,
  /// Its height, in the engine's units.
  pub height: f32,
}

impl InlineBox {
  /// A box of `width` by `height` before byte `index`.
  #[inline(always)]
  pub const fn new(index: usize, width: f32, height: f32) -> Self {
    Self {
      index,
      width,
      height,
    }
  }
}

/// A UTF-8 value, its runs and its inline boxes, every property resolved: what an engine lays
/// out.
///
/// The runs restyle parts of the value in order, without overlaps; the bytes no run covers
/// are laid out in the base style ([`AttributedText::base`]). The boxes stand in order among
/// the characters.
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct AttributedText {
  value: Rc<str>,
  base: TextStyle,
  runs: Vec<TextRun>,
  boxes: Vec<InlineBox>,
}

impl AttributedText {
  /// `value` in `style` throughout.
  pub fn plain(value: impl Into<Rc<str>>, style: TextStyle) -> Self {
    Self {
      value: value.into(),
      base: style,
      runs: Vec::new(),
      boxes: Vec::new(),
    }
  }

  /// `value` in `base`, with `runs` restyling parts of it.
  ///
  /// # Panics
  ///
  /// In a debug build, where a run's range is out of order, overlaps another, or falls
  /// inside a character.
  pub fn new(value: impl Into<Rc<str>>, base: TextStyle, runs: Vec<TextRun>) -> Self {
    let value: Rc<str> = value.into();

    debug_assert!(
      runs
        .windows(2)
        .all(|pair| pair[0].range.end <= pair[1].range.start),
      "runs are ordered and disjoint"
    );
    debug_assert!(
      runs.iter().all(|run| run.range.start <= run.range.end
        && value.is_char_boundary(run.range.start)
        && value.is_char_boundary(run.range.end)),
      "runs lie on character boundaries"
    );

    Self {
      value,
      base,
      runs,
      boxes: Vec::new(),
    }
  }

  /// This text with `boxes` standing among its characters.
  ///
  /// # Panics
  ///
  /// In a debug build, where the boxes are out of order or one stands inside a character.
  pub fn with_boxes(self, boxes: Vec<InlineBox>) -> Self {
    debug_assert!(
      boxes.windows(2).all(|pair| pair[0].index <= pair[1].index),
      "boxes are ordered"
    );
    debug_assert!(
      boxes
        .iter()
        .all(|inline| self.value.is_char_boundary(inline.index)),
      "boxes stand on character boundaries"
    );

    Self { boxes, ..self }
  }

  /// The value.
  #[inline(always)]
  #[must_use]
  pub fn value(&self) -> &str {
    &self.value
  }

  /// The value, shared.
  #[inline(always)]
  #[must_use]
  pub fn shared_value(&self) -> Rc<str> {
    Rc::clone(&self.value)
  }

  /// The style of every byte no run covers.
  #[inline(always)]
  #[must_use]
  pub const fn base(&self) -> &TextStyle {
    &self.base
  }

  /// The runs restyling parts of the value.
  #[inline(always)]
  #[must_use]
  pub fn runs(&self) -> &[TextRun] {
    &self.runs
  }

  /// The boxes standing among the characters, in order.
  #[inline(always)]
  #[must_use]
  pub fn boxes(&self) -> &[InlineBox] {
    &self.boxes
  }

  /// Every byte range of the value with its style, gaps between runs in the base style:
  /// the whole value, in order, with no empty range but for an empty value's one.
  #[must_use]
  pub fn spans(&self) -> Vec<(Range<usize>, &TextStyle)> {
    let mut spans = Vec::new();
    let mut at = 0;

    for run in &self.runs {
      if run.range.start > at {
        spans.push((at..run.range.start, &self.base));
      }

      if !run.range.is_empty() {
        spans.push((run.range.clone(), &run.style));
      }

      at = run.range.end;
    }

    if at < self.value.len() || spans.is_empty() {
      spans.push((at..self.value.len(), &self.base));
    }

    spans
  }
}

/// The direction a paragraph's lines start from where its text says nothing.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub enum BaseDirection {
  /// From the first strong character, left to right where none.
  #[default]
  Auto,
  /// Left to right.
  LeftToRight,
  /// Right to left.
  RightToLeft,
}

/// Whether a paragraph wraps its lines.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub enum TextWrap {
  /// Wraps where the Unicode line breaking algorithm allows, a word too wide for a line
  /// of its own broken where the line fills.
  #[default]
  Words,
  /// Never wraps: only newlines end lines.
  None,
}

/// How tall a paragraph's lines are.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub enum LineHeight {
  /// The typeface's own line height.
  #[default]
  Normal,
  /// A multiple of the font size.
  Factor(f32),
  /// A height in the engine's units.
  Absolute(f32),
}

/// Where a paragraph's lines sit across its width.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub enum TextAlignment {
  /// At the edge its lines start from: the left in left-to-right text.
  #[default]
  Start,
  /// At the edge its lines end at: the right in left-to-right text.
  End,
  /// At the left, whatever the direction.
  Left,
  /// At the right, whatever the direction.
  Right,
  /// Centered.
  Center,
}

/// A paragraph's own properties, resolved.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct ParagraphStyle {
  /// Where the lines sit across the width.
  pub alignment: TextAlignment,
  /// The direction the lines start from.
  pub direction: BaseDirection,
  /// The most lines shown; the rest is cut, the text then truncated.
  pub max_lines: Option<u32>,
  /// Whether a truncated text ends its last line in an ellipsis.
  pub ellipsis: bool,
  /// Whether lines wrap.
  pub wrap: TextWrap,
  /// How tall the lines are.
  pub line_height: LineHeight,
}

impl ParagraphStyle {
  /// This style placing its lines as `alignment` says.
  #[inline(always)]
  pub const fn with_alignment(self, alignment: TextAlignment) -> Self {
    Self { alignment, ..self }
  }

  /// This style wrapping as `wrap` says.
  #[inline(always)]
  pub const fn with_wrap(self, wrap: TextWrap) -> Self {
    Self { wrap, ..self }
  }

  /// This style showing at most `lines` lines, ending the last in an ellipsis when it
  /// cuts.
  #[inline(always)]
  pub const fn with_max_lines(self, lines: u32) -> Self {
    Self {
      max_lines: Some(lines),
      ellipsis: true,
      ..self
    }
  }

  /// This style starting its lines from `direction`.
  #[inline(always)]
  pub const fn with_direction(self, direction: BaseDirection) -> Self {
    Self { direction, ..self }
  }

  /// This style's lines as tall as `height` says.
  #[inline(always)]
  pub const fn with_line_height(self, height: LineHeight) -> Self {
    Self {
      line_height: height,
      ..self
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{FontWeight, LineHeight, PaintStyle, TextStyle};
  use crate::{
    Color,
    Length,
    Plain,
    RunStyle,
    TextDecoration,
    TextDecorationStyle,
    terminal::TerminalStyle,
    web::{WebRunFields, WebStyle},
  };

  #[test]
  fn a_resolved_run_style_reads_its_mediums_type_metrics() {
    let red = Color::srgb(1., 0., 0.);
    let style = RunStyle::<Plain, WebStyle> {
      color: Some(red),
      decoration: Some(TextDecoration::Underline),
      decoration_style: Some(TextDecorationStyle::Wavy),
      ext: WebRunFields {
        font_size: Some(Length(20.)),
        font_weight: Some(600.),
        line_height: Some(2.),
        letter_spacing: Some(Length(1.)),
      },
      ..RunStyle::default()
    };
    let resolved = TextStyle::resolved(&style, 16.);

    assert_eq!(resolved.size, 20.);
    assert_eq!(resolved.weight, FontWeight::SEMIBOLD);
    assert_eq!(resolved.line_height, Some(LineHeight::Factor(2.)));
    assert_eq!(resolved.letter_spacing, 1.);
    assert_eq!(
      resolved.paint,
      PaintStyle::new(red).with_decoration(TextDecoration::Underline, TextDecorationStyle::Wavy)
    );
    assert_eq!(
      TextStyle::resolved(&RunStyle::<Plain, WebStyle>::default(), 16.),
      TextStyle::new(16.),
      "what a style leaves unset"
    );
    assert_eq!(
      TextStyle::resolved(&RunStyle::<Plain, TerminalStyle>::default(), 1.).line_height,
      None,
      "a medium without a line height"
    );
  }
}
