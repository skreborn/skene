//! A canned engine for the foundation's own tests: every grapheme cluster ten units wide,
//! every line twenty tall with its baseline at fifteen, left to right, wrapping after blanks
//! and breaking a word too wide for a line where the line fills.

use std::{cell::RefCell, ops::Range, rc::Rc};

use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
  layout::Rect,
  text::{
    Affinity, AttributedText, Caret, Carets, Direction, Hit, Intrinsic, Laid, LineInfo, PaintStyle,
    Paragraph, ParagraphStyle, Position, RunExtent, Segments, TextBoundary, TextEngine, TextIndex,
    TextSize, VisualMove, Wrap,
  },
};

pub const ADVANCE: f32 = 10.;
pub const LINE: f32 = 20.;
pub const BASELINE: f32 = 15.;

#[derive(Clone, Copy, Default)]
pub struct Grid;

type Paints = Rc<RefCell<Vec<(Range<usize>, PaintStyle)>>>;

pub struct GridParagraph {
  text: Rc<str>,
  index: Rc<TextIndex>,
  style: ParagraphStyle,
  paints: Paints,
}

#[derive(Clone)]
pub struct GridLaid {
  text: Rc<str>,
  index: Rc<TextIndex>,
  lines: Rc<Vec<LineInfo>>,
  truncated: bool,
  paints: Paints,
}

impl TextEngine for Grid {
  type Paragraph = GridParagraph;

  fn paragraph(&self, text: &AttributedText, style: &ParagraphStyle) -> GridParagraph {
    GridParagraph {
      text: text.shared_value(),
      index: Rc::new(TextIndex::new(text.value())),
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

impl GridParagraph {
  fn width(&self, range: Range<usize>) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "a test's short texts")]
    let clusters = self.text[range].graphemes(true).count() as f32;

    clusters * ADVANCE
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
      Wrap::Words => width.unwrap_or(f32::INFINITY),
      Wrap::None => f32::INFINITY,
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
        } else {
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
      } else {
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
    let mut intrinsic = Intrinsic::default();
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

  fn layout(&self, width: Option<f32>) -> GridLaid {
    let mut ranges = self.break_lines(width);
    let max = self.style.max_lines.map(|lines| lines as usize);
    let truncated = max.is_some_and(|max| ranges.len() > max);

    if let Some(max) = max {
      ranges.truncate(max.max(1));
    }

    let count = ranges.len();
    let lines = ranges
      .into_iter()
      .enumerate()
      .map(|(at, range)| {
        #[expect(clippy::cast_precision_loss, reason = "a test's few lines")]
        let top = at as f32 * LINE;
        let hard_break = self.text[range.clone()].ends_with('\n');

        LineInfo {
          width: self.width(self.trimmed(range.clone())),
          range,
          top,
          height: LINE,
          baseline: top + BASELINE,
          left: 0.,
          hard_break: hard_break && at < count,
        }
      })
      .collect();

    GridLaid {
      text: Rc::clone(&self.text),
      index: Rc::clone(&self.index),
      lines: Rc::new(lines),
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
  fn x_of(&self, line: &LineInfo, index: usize) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "a test's short texts")]
    let clusters = self.text[line.range.start..index.clamp(line.range.start, line.range.end)]
      .graphemes(true)
      .count() as f32;

    line.left + clusters * ADVANCE
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
    } else {
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
      } else {
        Affinity::Downstream
      },
    )
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
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "clamped to the lines"
    )]
    let line = ((y / LINE).floor().max(0.) as usize).min(self.lines.len() - 1);
    let info = &self.lines[line];

    Hit {
      position: self.position_on(line, x),
      inside: y >= 0. && y < self.size().height && x >= 0. && x < info.width,
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

  fn range_rects(&self, range: Range<usize>) -> Vec<Rect> {
    self
      .lines
      .iter()
      .filter(|line| line.range.start < range.end && range.start < line.range.end)
      .map(|line| {
        let left = self.x_of(line, range.start);
        let right = self.x_of(line, range.end);

        Rect {
          x: left,
          y: line.top,
          width: right - left,
          height: line.height,
        }
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

        Position::with(
          end,
          if soft {
            Affinity::Upstream
          } else {
            Affinity::Downstream
          },
        )
      }
    }
  }

  fn move_vertical(&self, position: Position, lines: isize, goal: Option<f32>) -> (Position, f32) {
    let line = self.line_of(position);
    let goal = goal.unwrap_or_else(|| self.caret(position).primary.x);
    let target = line.checked_add_signed(lines);

    match target {
      Some(target) if target < self.lines.len() => (self.position_on(target, goal), goal),
      _ if lines < 0 => (Position::new(0), goal),
      _ => (Position::new(self.text.len()), goal),
    }
  }
}
