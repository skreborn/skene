//! The text model: a value and its runs with every property resolved, and the paragraph's
//! own properties. An engine receives these concrete values; resolution (tokens, conditions,
//! derived values, motion) happens before.

use std::{ops::Range, rc::Rc};

use crate::{
  Color,
  value::{FontFamily, FontStyle, FontWeight, TextAlignment, TextDecoration, TextDecorationStyle},
};

/// What a run is painted with: the properties that never move a glyph, so changing them
/// repaints and never lays out ([`super::Paragraph::repaint`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub struct PaintStyle {
  /// The glyphs' color.
  pub color: Color,
  /// The lines the run is decorated with.
  pub decoration: TextDecoration,
  /// How the lines are drawn.
  pub decoration_style: TextDecorationStyle,
  /// The lines' color, the glyphs' where none.
  pub decoration_color: Option<Color>,
}

impl PaintStyle {
  /// Plain glyphs in `color`, undecorated.
  #[inline(always)]
  pub const fn new(color: Color) -> Self {
    Self {
      color,
      decoration: TextDecoration::NONE,
      decoration_style: TextDecorationStyle::Solid,
      decoration_color: None,
    }
  }

  /// The color the decoration lines are drawn in.
  #[inline(always)]
  pub const fn line_color(&self) -> Color {
    match self.decoration_color {
      Some(color) => color,
      None => self.color,
    }
  }
}

impl Default for PaintStyle {
  fn default() -> Self {
    Self::new(Color::BLACK)
  }
}

/// A run's properties, resolved: what shapes it (the typeface, its size, weight and slant,
/// the spacing past each cluster) and what paints it.
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct RunStyle {
  /// The typeface's family.
  pub family: FontFamily,
  /// The font size, in the engine's units (pixels on a pixel platform, cells in the
  /// terminal).
  pub size: f32,
  /// The weight.
  pub weight: FontWeight,
  /// Upright or slanted.
  pub slant: FontStyle,
  /// Extra spacing past each cluster, in the engine's units.
  pub letter_spacing: f32,
  /// What paints the run.
  pub paint: PaintStyle,
}

impl RunStyle {
  /// The platform's interface typeface at `size`, regular and upright, in black.
  #[inline(always)]
  pub const fn new(size: f32) -> Self {
    Self {
      family: FontFamily::System,
      size,
      weight: FontWeight::REGULAR,
      slant: FontStyle::Normal,
      letter_spacing: 0.,
      paint: PaintStyle::new(Color::BLACK),
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

  /// This style painted with `paint`.
  #[inline(always)]
  pub fn with_paint(self, paint: PaintStyle) -> Self {
    Self { paint, ..self }
  }

  /// Whether `other` shapes its text exactly as this style does: only the paint differs.
  #[inline(always)]
  #[must_use]
  pub fn shapes_like(&self, other: &Self) -> bool {
    self.family == other.family
      && self.size.to_bits() == other.size.to_bits()
      && self.weight == other.weight
      && self.slant == other.slant
      && self.letter_spacing.to_bits() == other.letter_spacing.to_bits()
  }
}

/// One run of an attributed text: a byte range of its value, on character boundaries, and
/// the run's style.
#[derive(Clone, PartialEq, Debug)]
pub struct TextRun {
  /// The bytes of the value the run covers.
  pub range: Range<usize>,
  /// The run's properties.
  pub style: RunStyle,
}

/// A UTF-8 value and its runs, every property resolved: what an engine lays out.
///
/// The runs cover the value in order, without gaps or overlaps; a value with no runs
/// is laid out in the paragraph's base style ([`AttributedText::base`]).
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct AttributedText {
  value: Rc<str>,
  base: RunStyle,
  runs: Vec<TextRun>,
}

impl AttributedText {
  /// `value` in `style` throughout.
  pub fn plain(value: impl Into<Rc<str>>, style: RunStyle) -> Self {
    Self {
      value: value.into(),
      base: style,
      runs: Vec::new(),
    }
  }

  /// `value` in `base`, with `runs` restyling parts of it.
  ///
  /// # Panics
  ///
  /// In a debug build, where a run's range is out of order, overlaps another, or falls
  /// inside a character.
  pub fn new(value: impl Into<Rc<str>>, base: RunStyle, runs: Vec<TextRun>) -> Self {
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

    Self { value, base, runs }
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
  pub const fn base(&self) -> &RunStyle {
    &self.base
  }

  /// The runs restyling parts of the value.
  #[inline(always)]
  #[must_use]
  pub fn runs(&self) -> &[TextRun] {
    &self.runs
  }

  /// Every byte range of the value with its style, gaps between runs in the base style:
  /// the whole value, in order, with no empty range but for an empty value's one.
  #[must_use]
  pub fn spans(&self) -> Vec<(Range<usize>, &RunStyle)> {
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
pub enum Wrap {
  /// Wraps where the Unicode line breaking algorithm allows, a word too wide for a line
  /// of its own broken where the line fills.
  #[default]
  Words,
  /// Never wraps: only newlines end lines.
  None,
}

/// How tall a paragraph's lines are.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum LineHeight {
  /// The typeface's own line height.
  #[default]
  Normal,
  /// A multiple of the font size.
  Factor(f32),
  /// A height in the engine's units.
  Absolute(f32),
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
  pub wrap: Wrap,
  /// How tall the lines are.
  pub line_height: LineHeight,
}

impl ParagraphStyle {
  /// This style wrapping as `wrap` says.
  #[inline(always)]
  pub const fn with_wrap(self, wrap: Wrap) -> Self {
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
}
