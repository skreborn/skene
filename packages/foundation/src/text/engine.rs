//! The contract every realization's text engine implements: a paragraph of attributed text,
//! its intrinsic widths, a layout at a width with the geometry everything acting on text
//! needs, paint-only restyling, and the boundaries movement follows.
//!
//! A layout's geometry never changes, and its paint is its paragraph's: a repaint shows in
//! every layout of the paragraph, those already handed out included, and moves nothing.
//!
//! Every engine keeps the guaranteed core: a text's blanks and newlines are kept as they are
//! written, a word too wide for a line of its own breaks where the line fills, and an empty
//! text lays out one line, as a trailing newline opens one. Beyond it, a text reads as its
//! engine reads it.
//!
//! Positions and ranges are bytes of the paragraph's text, on character boundaries; lengths
//! are in the engine's units, from the layout's top left.

use std::ops::Range;

use crate::{
  Point,
  Rect,
  Size,
  text::{AttributedText, PaintStyle, ParagraphStyle},
};

/// A realization's text engine: Parley where Skene draws the text itself (over a cell typeface
/// in the terminal), the platform's own elsewhere.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a text engine",
  label = "text is laid out through an engine here",
  note = "a realization implements `TextEngine` once: a paragraph of attributed text, laid out at a width"
)]
pub trait TextEngine {
  /// The engine's paragraph.
  type Paragraph: Paragraph;

  /// `text` as a paragraph styled by `style`: shaped once, laid out at any width after.
  fn paragraph(&self, text: &AttributedText, style: &ParagraphStyle) -> Self::Paragraph;

  /// Each text `texts` yields as a paragraph styled by its style, in order, beside the text
  /// and the style it was made of: what [`TextEngine::paragraph`] makes of each, an engine
  /// free to make them together, and to start on the first while the next are still being
  /// produced (a walk of the scene yielding them as it goes).
  fn paragraphs(
    &self,
    texts: impl IntoIterator<Item = (AttributedText, ParagraphStyle)>,
  ) -> Vec<Made<Self::Paragraph>> {
    texts
      .into_iter()
      .map(|(text, style)| {
        let paragraph = self.paragraph(&text, &style);

        Made {
          text,
          style,
          paragraph,
        }
      })
      .collect()
  }
}

/// A paragraph an engine made of a batch's text ([`TextEngine::paragraphs`]), beside the text
/// and the style it was made of.
#[derive(Debug)]
#[must_use]
pub struct Made<P> {
  /// The text.
  pub text: AttributedText,
  /// The paragraph's style.
  pub style: ParagraphStyle,
  /// The paragraph.
  pub paragraph: P,
}

/// A paragraph: text shaped once, measured and laid out at widths.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a paragraph of a text engine",
  label = "a paragraph of a text engine is needed here",
  note = "an engine's paragraph is its text shaped once, measured and laid out at any width"
)]
pub trait Paragraph {
  /// The paragraph laid out at one width.
  type Laid: Laid;

  /// The text the paragraph lays out.
  fn text(&self) -> &str;

  /// The paragraph's narrowest width (its widest unbreakable piece) and its widest (each
  /// line unwrapped).
  fn intrinsic(&self) -> Intrinsic;

  /// The paragraph laid out within `width`, unbounded where `None`: geometry no later
  /// layout or repaint changes, painted as the paragraph is ([`Paragraph::repaint`]).
  fn layout(&self, width: Option<f32>) -> Self::Laid;

  /// Whether every line starts at the layout's left edge at any width (start-aligned
  /// left-to-right text, left-aligned text), so a layout within a width at or past the
  /// widest line is the unbounded one; `false` where the width places a line (centered, at
  /// the end, right-aligned, start-aligned right-to-left text).
  fn is_left_anchored(&self) -> bool;

  /// Repaints `range` (bytes of the text) with `paint`: the paint is the paragraph's, so it
  /// shows in every layout of the paragraph, those already handed out included, and changes
  /// no size or position. `false` where the engine needs the paragraph rebuilt instead.
  fn repaint(&mut self, range: Range<usize>, paint: &PaintStyle) -> bool;

  /// The boundary of `unit` next to byte `index` in `direction`, as the engine reads the
  /// text: where a caret lands moving logically, and the reach of a deletion.
  ///
  /// A word boundary skips blanks first, then one segment: a word or a punctuation mark.
  fn boundary(&self, index: usize, unit: TextBoundary, direction: Direction) -> usize;
}

/// A paragraph laid out at one width, cheap to clone: its geometry never changes, and it
/// paints as its paragraph paints now ([`Paragraph::repaint`]).
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a laid-out paragraph",
  label = "a paragraph laid out at a width is needed here",
  note = "a layout is a paragraph laid out at one width, cheap to clone, its geometry never changing"
)]
pub trait Laid: Clone {
  /// The size the lines take: the widest line's advance (no more than the width laid out
  /// within, but for one cluster wider than it) and the lines' total height.
  fn size(&self) -> TextSize;

  /// The first line's baseline, from the top.
  fn first_baseline(&self) -> f32;

  /// The last line's baseline, from the top.
  fn last_baseline(&self) -> f32;

  /// The lines laid out.
  fn line_count(&self) -> usize;

  /// Whether the text was cut at the paragraph's most lines.
  fn truncated(&self) -> bool;

  /// The lines, in order.
  fn lines(&self) -> Vec<LineInfo>;

  /// The caret position nearest `x`, `y` (relative to the layout's top left).
  fn hit_test(&self, x: f32, y: f32) -> Hit;

  /// The caret at `position`, and the second one where the position is a direction boundary
  /// of bidirectional text.
  fn caret(&self, position: Position) -> Carets;

  /// The rectangles `range` (bytes) covers, one or more a line, in visual order.
  fn range_rects(&self, range: Range<usize>) -> Vec<Rect<f32>>;

  /// The frame of each of the text's inline boxes, in the text's order
  /// ([`AttributedText::boxes`]); a box on a line cut by the paragraph's most lines has a
  /// frame of no size at the origin.
  fn inline_boxes(&self) -> Vec<Rect<f32>>;

  /// Each stretch of a line painted alike as the paragraph paints it now, with what the
  /// shared painter needs to draw its decorations.
  fn run_extents(&self) -> Vec<RunExtent>;

  /// The position a visual move from `position` lands on.
  fn move_visual(&self, position: Position, movement: VisualMove) -> Position;

  /// The position `lines` lines below `position` (above where negative), nearest the
  /// horizontal `goal` where given, else the position's own, a goal on a cluster's middle
  /// going past the cluster; with the goal the move keeps for the next. Past the first
  /// line it is the text's start, past the last its end.
  fn move_vertical(&self, position: Position, lines: isize, goal: Option<f32>) -> (Position, f32);
}

/// A paragraph's intrinsic widths, in its engine's units.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct Intrinsic {
  /// The narrowest the paragraph lays out at without overflowing a piece it cannot break.
  pub min: f32,
  /// The widest the paragraph is, each line unwrapped.
  pub max: f32,
}

/// A size in an engine's units.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct TextSize {
  /// The width.
  pub width: f32,
  /// The height.
  pub height: f32,
}

/// Which side of a position a caret belongs to where both sides are not adjacent on screen
/// (a line's end, a direction change).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub enum Affinity {
  /// With the character after the position.
  #[default]
  Downstream,
  /// With the character before the position.
  Upstream,
}

/// A caret position: a byte index of the text and the side it belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub struct Position {
  /// The byte index, on a character boundary.
  pub index: usize,
  /// The side.
  pub affinity: Affinity,
}

impl Position {
  /// The position at `index`, with the character after it.
  #[inline(always)]
  pub const fn new(index: usize) -> Self {
    Self {
      index,
      affinity: Affinity::Downstream,
    }
  }

  /// The position at `index`, on `affinity`'s side.
  #[inline(always)]
  pub const fn with(index: usize, affinity: Affinity) -> Self {
    Self { index, affinity }
  }
}

/// What a point hits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub struct Hit {
  /// The caret position nearest the point.
  pub position: Position,
  /// Whether the point lies on the text rather than beside it.
  pub inside: bool,
}

/// A caret: a vertical bar.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct Caret {
  /// Its horizontal position.
  pub x: f32,
  /// Its top.
  pub y: f32,
  /// Its height.
  pub height: f32,
  /// Whether the text it stands in runs right to left.
  pub right_to_left: bool,
}

impl Caret {
  /// The caret as a rectangle of no width.
  #[inline(always)]
  pub const fn rect(&self) -> Rect<f32> {
    Rect::new(Point::new(self.x, self.y), Size::new(0., self.height))
  }
}

/// A position's carets: where typing goes, and where text of the other direction would go
/// at a direction boundary.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct Carets {
  /// The caret of the position's own side.
  pub primary: Caret,
  /// The caret of the other side, where it stands elsewhere.
  pub secondary: Option<Caret>,
}

/// One line laid out.
#[derive(Clone, PartialEq, Debug, Default)]
#[must_use]
pub struct LineInfo {
  /// The bytes of the text on the line, the newline ending it included.
  pub range: Range<usize>,
  /// Its top.
  pub top: f32,
  /// Its height.
  pub height: f32,
  /// Its baseline, from the layout's top.
  pub baseline: f32,
  /// Where its content starts horizontally, after alignment.
  pub left: f32,
  /// The advance of its content, short of the blanks hanging past its end.
  pub width: f32,
  /// Whether a newline ends it.
  pub hard_break: bool,
}

/// One stretch of a line painted alike: where it sits, and the typeface's decoration
/// metrics there.
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct RunExtent {
  /// The bytes it covers.
  pub range: Range<usize>,
  /// The line it is on.
  pub line: usize,
  /// Its left edge.
  pub x: f32,
  /// Its advance.
  pub width: f32,
  /// Its baseline, from the layout's top.
  pub baseline: f32,
  /// The typeface's ascent above the baseline.
  pub ascent: f32,
  /// The typeface's descent below the baseline.
  pub descent: f32,
  /// The underline's top, above the baseline (below where negative).
  pub underline_offset: f32,
  /// The underline's thickness.
  pub underline_thickness: f32,
  /// The strikethrough's top, above the baseline.
  pub strikethrough_offset: f32,
  /// The strikethrough's thickness.
  pub strikethrough_thickness: f32,
  /// What it is painted with, as its paragraph paints it now.
  pub paint: PaintStyle,
}

/// A unit of logical movement.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub enum TextBoundary {
  /// An extended grapheme cluster.
  Grapheme,
  /// A word: blanks skipped, then a word or a punctuation mark.
  Word,
  /// A hard line: up to the newline before, or the next one.
  Paragraph,
  /// The whole text.
  Text,
}

/// A logical direction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub enum Direction {
  /// Toward the text's end.
  Forward,
  /// Toward the text's start.
  Backward,
}

/// A move on screen.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub enum VisualMove {
  /// One cluster to the left.
  Left,
  /// One cluster to the right.
  Right,
  /// One word to the left.
  WordLeft,
  /// One word to the right.
  WordRight,
  /// To the start of the visual line (its left edge in left-to-right text).
  LineStart,
  /// To the end of the visual line: short of the blanks hanging past it where a soft break
  /// ends it.
  LineEnd,
}
