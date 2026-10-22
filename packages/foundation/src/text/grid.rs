//! A canned engine for the foundation's own tests: every grapheme cluster ten units wide,
//! every line twenty tall with its baseline at fifteen, left to right, wrapping after blanks
//! and breaking a word too wide for a line where the line fills. An inline box stands on its
//! line's baseline, raising the line where it is taller.

use std::{cell::RefCell, ops::Range, rc::Rc};

use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
  Point,
  Rect,
  Size,
  text::{
    Affinity,
    AttributedText,
    Caret,
    Carets,
    Direction,
    Hit,
    InlineBox,
    Intrinsic,
    Laid,
    LineInfo,
    PaintStyle,
    Paragraph,
    ParagraphStyle,
    Position,
    RunExtent,
    Segments,
    TextAlignment,
    TextBoundary,
    TextEngine,
    TextIndex,
    TextSize,
    TextWrap,
    VisualMove,
  },
};

/// A cluster's advance.
pub(super) const ADVANCE: f32 = 10.;
/// A line's height.
pub(super) const LINE: f32 = 20.;
/// A line's baseline, from its top.
pub(super) const BASELINE: f32 = 15.;

/// The canned engine.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Grid;

/// The paint of each stretch of a paragraph's text, shared by its layouts.
type Paints = Rc<RefCell<Vec<(Range<usize>, PaintStyle)>>>;

/// A paragraph of [`Grid`].
#[derive(Debug)]
pub(super) struct GridParagraph {
  text: Rc<str>,
  index: Rc<TextIndex>,
  boxes: Rc<[InlineBox]>,
  style: ParagraphStyle,
  paints: Paints,
}

/// A layout of [`GridParagraph`].
#[derive(Clone, Debug)]
pub(super) struct GridLaid {
  text: Rc<str>,
  index: Rc<TextIndex>,
  boxes: Rc<[InlineBox]>,
  lines: Rc<[LineInfo]>,
  truncated: bool,
  paints: Paints,
}

impl TextEngine for Grid {
  type Paragraph = GridParagraph;

  fn paragraph(&self, text: &AttributedText, style: &ParagraphStyle) -> GridParagraph {
    GridParagraph {
      text: text.shared_value(),
      index: Rc::new(TextIndex::new(text.value())),
      boxes: text.boxes().into(),
      style: *style,
      paints: Rc::new(RefCell::new(
        text
          .spans()
          .into_iter()
          .map(|(range, style)| (range, style.paint))
          .collect(),
      )),
    }
  }
}

/// Whether a box at `index` stands within `range` of a text `len` bytes long: before one of
/// its characters, or at the text's end where the range ends there.
const fn holds(range: &Range<usize>, index: usize, len: usize) -> bool {
  range.start <= index && (index < range.end || index == len && range.end == len)
}

/// The advance of the clusters of `range` of `text`.
#[expect(clippy::cast_precision_loss, reason = "a test's short texts")]
fn clusters(text: &str, range: Range<usize>) -> f32 {
  text[range].graphemes(true).count() as f32 * ADVANCE
}

/// The advance of `range` of `text`, the boxes in it included.
fn advance(text: &str, boxes: &[InlineBox], range: Range<usize>) -> f32 {
  let boxes = boxes
    .iter()
    .filter(|inline| holds(&range, inline.index, text.len()))
    .map(|inline| inline.width)
    .sum::<f32>();

  clusters(text, range) + boxes
}

impl GridParagraph {
  fn width(&self, range: Range<usize>) -> f32 {
    advance(&self.text, &self.boxes, range)
  }

  /// Each word with the blanks after it, by byte range.
  fn tokens(&self, paragraph: Range<usize>) -> Vec<Range<usize>> {
    let mut tokens: Vec<Range<usize>> = Vec::new();

    for (start, segment) in self.text[paragraph.clone()].split_word_bound_indices() {
      let range = paragraph.start + start..paragraph.start + start + segment.len();
      let blank = segment.chars().all(char::is_whitespace);

      match tokens.last_mut() {
        Some(last) if blank => last.end = range.end,
        _ => tokens.push(range),
      }
    }

    tokens
  }

  fn trimmed(&self, range: Range<usize>) -> Range<usize> {
    range.start..range.start + self.text[range.clone()].trim_end().len()
  }

  fn break_lines(&self, width: Option<f32>) -> Vec<Range<usize>> {
    let limit = match self.style.wrap {
      TextWrap::Words => width.unwrap_or(f32::INFINITY),
      TextWrap::None => f32::INFINITY,
    };
    let mut lines = Vec::new();
    let mut start = 0;

    loop {
      let end = self.text[start..]
        .find('\n')
        .map_or(self.text.len(), |at| start + at);
      let mut line = start..start;

      for token in self.tokens(start..end) {
        let fits = self.width(self.trimmed(line.start..token.end)) <= limit;

        if fits || line.is_empty() {
          line.end = token.end;
        }
        else {
          lines.push(line.clone());
          line = token.start..token.end;
        }

        while self.width(self.trimmed(line.clone())) > limit
          && self.text[line.clone()].graphemes(true).count() > 1
        {
          let mut cut = line.start;

          for (at, cluster) in self.text[line.clone()].grapheme_indices(true) {
            if cut > line.start && self.width(line.start..line.start + at + cluster.len()) > limit {
              break;
            }

            cut = line.start + at + cluster.len();
          }

          lines.push(line.start..cut);
          line = cut..line.end;
        }
      }

      if end < self.text.len() {
        line.end = end + 1;
        lines.push(line);
        start = end + 1;
      }
      else {
        lines.push(line);

        break;
      }
    }

    lines
  }
}

impl Paragraph for GridParagraph {
  type Laid = GridLaid;

  fn text(&self) -> &str {
    &self.text
  }

  fn intrinsic(&self) -> Intrinsic {
    let mut intrinsic = Intrinsic {
      min: self
        .boxes
        .iter()
        .map(|inline| inline.width)
        .fold(0., f32::max),
      max: 0.,
    };
    let mut start = 0;

    loop {
      let end = self.text[start..]
        .find('\n')
        .map_or(self.text.len(), |at| start + at);

      intrinsic.max = intrinsic.max.max(self.width(self.trimmed(start..end)));

      for token in self.tokens(start..end) {
        intrinsic.min = intrinsic.min.max(self.width(self.trimmed(token)));
      }

      if end == self.text.len() {
        break intrinsic;
      }

      start = end + 1;
    }
  }

  fn is_left_anchored(&self) -> bool {
    matches!(
      self.style.alignment,
      TextAlignment::Start | TextAlignment::Left
    )
  }

  fn layout(&self, width: Option<f32>) -> GridLaid {
    let mut ranges = self.break_lines(width);
    let max = self.style.max_lines.map(|lines| lines as usize);
    let truncated = max.is_some_and(|max| ranges.len() > max);

    if let Some(max) = max {
      ranges.truncate(max.max(1));
    }

    let widest = ranges
      .iter()
      .map(|range| self.width(self.trimmed(range.clone())))
      .fold(0., f32::max);
    let within = width.unwrap_or(widest);
    let mut top = 0.;
    let lines = ranges
      .into_iter()
      .map(|range| {
        let tallest = self
          .boxes
          .iter()
          .filter(|inline| holds(&range, inline.index, self.text.len()))
          .map(|inline| inline.height)
          .fold(0., f32::max);
        let baseline = BASELINE.max(tallest);
        let height = LINE.max(baseline + LINE - BASELINE);
        let advance = self.width(self.trimmed(range.clone()));
        let left = match self.style.alignment {
          TextAlignment::End | TextAlignment::Right => within - advance,
          TextAlignment::Center => (within - advance) / 2.,
          TextAlignment::Start | TextAlignment::Left => 0.,
        };
        let line = LineInfo {
          hard_break: self.text[range.clone()].ends_with('\n'),
          range,
          top,
          height,
          baseline: top + baseline,
          left: left.max(0.),
          width: advance,
        };

        top += height;

        line
      })
      .collect();

    GridLaid {
      text: Rc::clone(&self.text),
      index: Rc::clone(&self.index),
      boxes: Rc::clone(&self.boxes),
      lines,
      truncated,
      paints: Rc::clone(&self.paints),
    }
  }

  fn repaint(&mut self, range: Range<usize>, paint: &PaintStyle) -> bool {
    let mut paints = self.paints.borrow_mut();
    let mut next = Vec::new();

    for (span, style) in paints.drain(..) {
      if span.start < range.start {
        next.push((span.start..span.end.min(range.start), style));
      }

      if span.end > range.end {
        next.push((span.start.max(range.end)..span.end, style));
      }
    }

    next.push((range, *paint));
    next.sort_by_key(|(span, _)| span.start);
    *paints = next;

    true
  }

  fn boundary(&self, index: usize, unit: TextBoundary, direction: Direction) -> usize {
    Segments::new(&self.text).boundary(&self.index, index, unit, direction)
  }
}

impl GridLaid {
  /// Where the caret before byte `index` stands on `line`: after the clusters and the boxes
  /// before it.
  fn x_of(&self, line: &LineInfo, index: usize) -> f32 {
    let end = index.clamp(line.range.start, line.range.end);
    let boxes = self
      .boxes
      .iter()
      .filter(|inline| line.range.start <= inline.index && inline.index < end)
      .map(|inline| inline.width)
      .sum::<f32>();

    line.left + clusters(&self.text, line.range.start..end) + boxes
  }

  /// The line a position stands on: a position at a soft break belongs upstream to the line
  /// it ends, downstream to the line it starts.
  fn line_of(&self, position: Position) -> usize {
    let last = self.lines.len() - 1;

    for (at, line) in self.lines.iter().enumerate() {
      let end = self.shown_end(at);

      if position.index < line.range.end
        || (position.index == end && (position.affinity == Affinity::Upstream || at == last))
      {
        return at;
      }
    }

    last
  }

  /// Where a line's caret positions end: before its newline.
  fn shown_end(&self, line: usize) -> usize {
    let range = &self.lines[line].range;

    if self.text[range.clone()].ends_with('\n') {
      range.end - 1
    }
    else {
      range.end
    }
  }

  fn position_on(&self, line: usize, x: f32) -> Position {
    let info = &self.lines[line];
    let mut at = info.range.start;
    let end = self.shown_end(line);

    for (start, cluster) in self.text[info.range.start..end].grapheme_indices(true) {
      let left = self.x_of(info, info.range.start + start);

      if x < left + ADVANCE / 2. {
        break;
      }

      at = info.range.start + start + cluster.len();
    }

    let soft_end = at == end && !info.hard_break && line + 1 < self.lines.len();

    Position::with(
      at,
      if soft_end {
        Affinity::Upstream
      }
      else {
        Affinity::Downstream
      },
    )
  }

  /// The line at `y`, the nearest where none.
  fn line_at(&self, y: f32) -> usize {
    self
      .lines
      .iter()
      .position(|line| y < line.top + line.height)
      .unwrap_or(self.lines.len() - 1)
  }
}

impl Laid for GridLaid {
  fn size(&self) -> TextSize {
    TextSize {
      width: self.lines.iter().map(|line| line.width).fold(0., f32::max),
      height: self
        .lines
        .last()
        .map_or(LINE, |line| line.top + line.height),
    }
  }

  fn first_baseline(&self) -> f32 {
    self.lines.first().map_or(BASELINE, |line| line.baseline)
  }

  fn last_baseline(&self) -> f32 {
    self.lines.last().map_or(BASELINE, |line| line.baseline)
  }

  fn line_count(&self) -> usize {
    self.lines.len()
  }

  fn truncated(&self) -> bool {
    self.truncated
  }

  fn lines(&self) -> Vec<LineInfo> {
    self.lines.to_vec()
  }

  fn hit_test(&self, x: f32, y: f32) -> Hit {
    let line = self.line_at(y);
    let info = &self.lines[line];

    Hit {
      position: self.position_on(line, x),
      inside: y >= 0. && y < self.size().height && x >= info.left && x < info.left + info.width,
    }
  }

  fn caret(&self, position: Position) -> Carets {
    let line = &self.lines[self.line_of(position)];

    Carets {
      primary: Caret {
        x: self.x_of(line, position.index),
        y: line.top,
        height: line.height,
        right_to_left: false,
      },
      secondary: None,
    }
  }

  fn range_rects(&self, range: Range<usize>) -> Vec<Rect<f32>> {
    self
      .lines
      .iter()
      .filter(|line| line.range.start < range.end && range.start < line.range.end)
      .map(|line| {
        let left = self.x_of(line, range.start);
        let right = self.x_of(line, range.end);

        Rect::new(
          Point::new(left, line.top),
          Size::new(right - left, line.height),
        )
      })
      .collect()
  }

  fn inline_boxes(&self) -> Vec<Rect<f32>> {
    let len = self.text.len();

    self
      .boxes
      .iter()
      .enumerate()
      .map(|(at, inline)| {
        let Some(line) = self
          .lines
          .iter()
          .find(|line| holds(&line.range, inline.index, len))
        else {
          return Rect::ZERO;
        };
        let before = self.boxes[..at]
          .iter()
          .filter(|other| holds(&line.range, other.index, len))
          .map(|other| other.width)
          .sum::<f32>();
        let x = line.left + clusters(&self.text, line.range.start..inline.index) + before;

        Rect::new(
          Point::new(x, line.baseline - inline.height),
          Size::new(inline.width, inline.height),
        )
      })
      .collect()
  }

  fn run_extents(&self) -> Vec<RunExtent> {
    let paints = self.paints.borrow();
    let mut extents = Vec::new();

    for (at, line) in self.lines.iter().enumerate() {
      for (span, paint) in paints.iter() {
        let start = span.start.max(line.range.start);
        let end = span.end.min(line.range.end);

        if start < end {
          extents.push(RunExtent {
            range: start..end,
            line: at,
            x: self.x_of(line, start),
            width: self.x_of(line, end) - self.x_of(line, start),
            baseline: line.baseline,
            ascent: BASELINE,
            descent: LINE - BASELINE,
            underline_offset: -2.,
            underline_thickness: 1.,
            strikethrough_offset: 5.,
            strikethrough_thickness: 1.,
            paint: *paint,
          });
        }
      }
    }

    extents
  }

  fn move_visual(&self, position: Position, movement: VisualMove) -> Position {
    let segments = Segments::new(&self.text);
    let line = self.line_of(position);

    match movement {
      VisualMove::Left => Position::new(self.index.previous_grapheme(position.index)),
      VisualMove::Right => Position::new(self.index.next_grapheme(position.index)),
      VisualMove::WordLeft => Position::new(segments.word_before(position.index)),
      VisualMove::WordRight => Position::new(segments.word_after(position.index)),
      VisualMove::LineStart => Position::new(self.lines[line].range.start),
      VisualMove::LineEnd => {
        let end = self.shown_end(line);
        let soft = !self.lines[line].hard_break && line + 1 < self.lines.len();

        if soft {
          let start = self.lines[line].range.start;
          let shown = self.text[start..end].trim_end_matches(char::is_whitespace);

          Position::with(start + shown.len(), Affinity::Upstream)
        }
        else {
          Position::new(end)
        }
      }
    }
  }

  fn move_vertical(&self, position: Position, lines: isize, goal: Option<f32>) -> (Position, f32) {
    let line = self.line_of(position);
    let goal = goal.unwrap_or_else(|| self.caret(position).primary.x);

    match line.checked_add_signed(lines) {
      Some(target) if target < self.lines.len() => (self.position_on(target, goal), goal),
      _ if lines < 0 => (Position::new(0), goal),
      _ => (Position::new(self.text.len()), goal),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{ADVANCE, BASELINE, Grid, LINE};
  use crate::{
    Color,
    Point,
    Rect,
    Size,
    text::{
      AttributedText,
      InlineBox,
      Laid,
      PaintStyle,
      Paragraph,
      ParagraphStyle,
      Position,
      TextEngine,
      TextStyle,
    },
  };

  fn paragraph(text: &str) -> super::GridParagraph {
    Grid.paragraph(
      &AttributedText::plain(text, TextStyle::new(13.)),
      &ParagraphStyle::default(),
    )
  }

  fn lines(laid: &impl Laid) -> Vec<std::ops::Range<usize>> {
    laid.lines().into_iter().map(|line| line.range).collect()
  }

  #[test]
  fn blanks_and_newlines_are_kept() {
    let laid = paragraph("a  b\n\nc").layout(None);

    assert_eq!(lines(&laid), [0..5, 5..6, 6..7], "each newline ends a line");
    assert_eq!(laid.lines()[0].width, 4. * ADVANCE, "both blanks are kept");
    assert!(laid.lines()[1].hard_break);
    assert_eq!(laid.size().height, 3. * LINE);
  }

  #[test]
  fn a_word_too_wide_for_its_line_breaks_where_the_line_fills() {
    let paragraph = paragraph("ab abcdefgh");
    let laid = paragraph.layout(Some(35.));

    assert_eq!(
      paragraph.intrinsic().min,
      8. * ADVANCE,
      "the word is the widest piece"
    );
    assert_eq!(lines(&laid), [0..3, 3..6, 6..9, 9..11]);
    assert!(laid.size().width <= 35.);
  }

  #[test]
  fn an_empty_text_lays_out_one_line_and_a_trailing_newline_opens_one() {
    let empty = paragraph("").layout(Some(100.));

    assert_eq!(empty.line_count(), 1);
    assert_eq!(empty.size().height, LINE);
    assert_eq!(empty.first_baseline(), BASELINE);

    let ended = paragraph("a\n").layout(Some(100.));

    assert_eq!(lines(&ended), [0..2, 2..2]);
    assert_eq!(ended.size().height, 2. * LINE);
    assert_eq!(ended.caret(Position::new(2)).primary.y, LINE);
  }

  #[test]
  fn a_layout_is_immutable_and_independent_of_later_ones() {
    let paragraph = paragraph("alpha beta gamma");
    let narrow = paragraph.layout(Some(60.));
    let before = (narrow.lines(), narrow.size(), narrow.range_rects(0..16));
    let wide = paragraph.layout(None);

    assert_eq!(wide.line_count(), 1);
    assert_eq!(narrow.line_count(), 3);
    assert_eq!(
      (narrow.lines(), narrow.size(), narrow.range_rects(0..16)),
      before,
      "a later layout at another width changes none handed out"
    );
  }

  #[test]
  fn a_repaint_shows_in_every_layout_handed_out_and_moves_nothing() {
    let mut paragraph = paragraph("alpha beta");
    let laid = paragraph.layout(Some(60.));
    let geometry = (laid.lines(), laid.size(), laid.caret(Position::new(7)));
    let white = PaintStyle::new(Color::oklch(1., 0., 0.));

    assert!(paragraph.repaint(6..10, &white));

    let painted = laid
      .run_extents()
      .into_iter()
      .filter(|extent| extent.paint == white)
      .map(|extent| (extent.range.start, extent.range.end))
      .collect::<Vec<_>>();

    assert_eq!(
      painted,
      [(6, 10)],
      "the layout handed out shows the repaint"
    );
    assert_eq!(
      (laid.lines(), laid.size(), laid.caret(Position::new(7))),
      geometry,
      "a repaint moves nothing"
    );
  }

  #[test]
  fn an_inline_box_flows_among_the_characters_and_raises_its_line() {
    let text = AttributedText::plain("ab cd", TextStyle::new(13.))
      .with_boxes(vec![InlineBox::new(1, 30., 40.), InlineBox::new(5, 5., 5.)]);
    let paragraph = Grid.paragraph(&text, &ParagraphStyle::default());
    let laid = paragraph.layout(None);

    assert_eq!(laid.size().width, 5. * ADVANCE + 35.);
    assert_eq!(laid.first_baseline(), 40., "the box stands on the baseline");
    assert_eq!(
      laid.inline_boxes(),
      [
        Rect::new(Point::new(ADVANCE, 0.), Size::new(30., 40.)),
        Rect::new(Point::new(5. * ADVANCE + 30., 35.), Size::new(5., 5.)),
      ]
    );
    assert_eq!(laid.caret(Position::new(2)).primary.x, 2. * ADVANCE + 30.);

    let cut = Grid
      .paragraph(
        &AttributedText::plain("ab\ncd", TextStyle::new(13.))
          .with_boxes(vec![InlineBox::new(4, 5., 5.)]),
        &ParagraphStyle::default().with_max_lines(1),
      )
      .layout(None);

    assert!(cut.truncated());
    assert_eq!(cut.inline_boxes(), [Rect::ZERO], "a box on a line cut");
  }
}
