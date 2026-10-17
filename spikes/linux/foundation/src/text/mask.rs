//! A secure entry's text: the same engine and the same entry as any other, each grapheme
//! cluster laid out as a bullet but for the one a reader just typed, shown for a moment.

use std::{cell::Cell, ops::Range, rc::Rc};

use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
  layout::Rect,
  text::{
    AttributedText, Carets, Direction, Hit, Intrinsic, Laid, LineInfo, PaintStyle, Paragraph,
    ParagraphStyle, Position, RunExtent, TextBoundary, TextEngine, TextRun, TextSize, VisualMove,
  },
};

/// Whether an entry's text is masked, and which cluster shows as typed: shared between the
/// entry and its engine, so a change takes effect at the next layout.
#[derive(Debug)]
pub struct MaskState {
  masked: Cell<bool>,
  revealed: Cell<Option<usize>>,
  mark: Cell<char>,
}

impl MaskState {
  /// A state masking nothing, each masked cluster drawn as `mark`.
  #[must_use]
  pub const fn new(mark: char) -> Self {
    Self {
      masked: Cell::new(false),
      revealed: Cell::new(None),
      mark: Cell::new(mark),
    }
  }

  /// What each masked cluster is drawn as.
  #[inline(always)]
  #[must_use]
  pub const fn mark(&self) -> char {
    self.mark.get()
  }

  /// Draws each masked cluster as `mark` from the next layout on.
  pub fn set_mark(&self, mark: char) {
    self.mark.set(mark);
  }

  /// What `text` shows as under the state: a mark per grapheme cluster where it masks, the
  /// text itself otherwise.
  #[must_use]
  pub fn shown(&self, text: &str) -> String {
    if self.is_masked() {
      std::iter::repeat_n(self.mark(), text.graphemes(true).count()).collect()
    } else {
      text.to_owned()
    }
  }

  /// Whether the text is masked.
  #[inline(always)]
  #[must_use]
  pub const fn is_masked(&self) -> bool {
    self.masked.get()
  }

  /// Masks the text where `masked`, revealing nothing.
  pub fn set_masked(&self, masked: bool) {
    self.masked.set(masked);
    self.revealed.set(None);
  }

  /// The byte index of the cluster shown as typed.
  #[inline(always)]
  #[must_use]
  pub const fn revealed(&self) -> Option<usize> {
    self.revealed.get()
  }

  /// Shows the cluster starting at byte `index` as typed, or none.
  pub fn reveal(&self, index: Option<usize>) {
    self.revealed.set(index);
  }
}

/// An engine laying text out masked where its [`MaskState`] says, and as its inner engine
/// does otherwise: every position it reports and takes is a byte of the real text.
#[derive(Clone, Debug)]
pub struct Masked<E> {
  inner: E,
  state: Rc<MaskState>,
}

impl<E> Masked<E> {
  /// `inner`, masked as `state` says.
  pub const fn new(inner: E, state: Rc<MaskState>) -> Self {
    Self { inner, state }
  }

  /// The state.
  #[must_use]
  pub const fn state(&self) -> &Rc<MaskState> {
    &self.state
  }
}

/// The bytes where each cluster starts in the real text and in the shown one, the ends
/// included.
#[derive(Debug)]
struct Map {
  real: Vec<usize>,
  shown: Vec<usize>,
}

impl Map {
  fn convert(from: &[usize], to: &[usize], index: usize) -> usize {
    let cluster = from
      .partition_point(|start| *start <= index)
      .saturating_sub(1);

    to[cluster.min(to.len() - 1)]
  }

  fn to_shown(&self, index: usize) -> usize {
    Self::convert(&self.real, &self.shown, index)
  }

  fn to_real(&self, index: usize) -> usize {
    Self::convert(&self.shown, &self.real, index)
  }

  fn range_to_shown(&self, range: Range<usize>) -> Range<usize> {
    self.to_shown(range.start)..self.to_shown(range.end)
  }

  fn range_to_real(&self, range: Range<usize>) -> Range<usize> {
    self.to_real(range.start)..self.to_real(range.end)
  }
}

/// A paragraph of [`Masked`]: the inner engine's, over the shown text where it is masked.
pub struct MaskedParagraph<P> {
  inner: P,
  real: Rc<str>,
  map: Option<Rc<Map>>,
}

/// A layout of [`MaskedParagraph`].
#[derive(Clone)]
pub struct MaskedLaid<L> {
  inner: L,
  map: Option<Rc<Map>>,
}

impl<L> MaskedLaid<L> {
  /// The inner engine's layout, of the text as shown.
  pub const fn inner(&self) -> &L {
    &self.inner
  }

  /// Whether the text was masked when it was laid out.
  #[must_use]
  pub const fn is_masked(&self) -> bool {
    self.map.is_some()
  }

  /// The byte of the real text where the cluster shown at byte `shown` of the inner layout's
  /// text starts: `shown` itself where the text is not masked.
  #[must_use]
  pub fn real_index(&self, shown: usize) -> usize {
    self.map.as_ref().map_or(shown, |map| map.to_real(shown))
  }
}

impl<E: TextEngine> TextEngine for Masked<E> {
  type Paragraph = MaskedParagraph<E::Paragraph>;

  fn paragraph(&self, text: &AttributedText, style: &ParagraphStyle) -> Self::Paragraph {
    let real = text.shared_value();

    if !self.state.is_masked() {
      return MaskedParagraph {
        inner: self.inner.paragraph(text, style),
        real,
        map: None,
      };
    }

    let revealed = self.state.revealed();
    let mut shown = String::with_capacity(real.len());
    let mut map = Map {
      real: Vec::new(),
      shown: Vec::new(),
    };

    for (start, cluster) in real.grapheme_indices(true) {
      map.real.push(start);
      map.shown.push(shown.len());

      if revealed == Some(start) {
        shown.push_str(cluster);
      } else {
        shown.push(self.state.mark());
      }
    }

    map.real.push(real.len());
    map.shown.push(shown.len());

    let runs = text
      .runs()
      .iter()
      .map(|run| TextRun {
        range: map.range_to_shown(run.range.clone()),
        style: run.style.clone(),
      })
      .collect();
    let shown = AttributedText::new(shown, text.base().clone(), runs);

    MaskedParagraph {
      inner: self.inner.paragraph(&shown, style),
      real,
      map: Some(Rc::new(map)),
    }
  }
}

impl<P: Paragraph> Paragraph for MaskedParagraph<P> {
  type Laid = MaskedLaid<P::Laid>;

  fn text(&self) -> &str {
    &self.real
  }

  fn intrinsic(&self) -> Intrinsic {
    self.inner.intrinsic()
  }

  fn layout(&self, width: Option<f32>) -> Self::Laid {
    MaskedLaid {
      inner: self.inner.layout(width),
      map: self.map.clone(),
    }
  }

  fn repaint(&mut self, range: Range<usize>, paint: &PaintStyle) -> bool {
    let range = match &self.map {
      Some(map) => map.range_to_shown(range),
      None => range,
    };

    self.inner.repaint(range, paint)
  }

  fn boundary(&self, index: usize, unit: TextBoundary, direction: Direction) -> usize {
    match &self.map {
      None => self.inner.boundary(index, unit, direction),
      Some(_) if matches!(unit, TextBoundary::Word | TextBoundary::Paragraph) => match direction {
        Direction::Forward => self.real.len(),
        Direction::Backward => 0,
      },
      Some(map) => map.to_real(self.inner.boundary(map.to_shown(index), unit, direction)),
    }
  }
}

impl<L: Laid> MaskedLaid<L> {
  fn to_shown(&self, position: Position) -> Position {
    match &self.map {
      Some(map) => Position::with(map.to_shown(position.index), position.affinity),
      None => position,
    }
  }

  fn to_real(&self, position: Position) -> Position {
    match &self.map {
      Some(map) => Position::with(map.to_real(position.index), position.affinity),
      None => position,
    }
  }

  fn range_to_real(&self, range: Range<usize>) -> Range<usize> {
    match &self.map {
      Some(map) => map.range_to_real(range),
      None => range,
    }
  }
}

impl<L: Laid> Laid for MaskedLaid<L> {
  fn size(&self) -> TextSize {
    self.inner.size()
  }

  fn first_baseline(&self) -> f32 {
    self.inner.first_baseline()
  }

  fn last_baseline(&self) -> f32 {
    self.inner.last_baseline()
  }

  fn line_count(&self) -> usize {
    self.inner.line_count()
  }

  fn truncated(&self) -> bool {
    self.inner.truncated()
  }

  fn lines(&self) -> Vec<LineInfo> {
    self
      .inner
      .lines()
      .into_iter()
      .map(|line| LineInfo {
        range: self.range_to_real(line.range.clone()),
        ..line
      })
      .collect()
  }

  fn hit_test(&self, x: f32, y: f32) -> Hit {
    let hit = self.inner.hit_test(x, y);

    Hit {
      position: self.to_real(hit.position),
      ..hit
    }
  }

  fn caret(&self, position: Position) -> Carets {
    self.inner.caret(self.to_shown(position))
  }

  fn range_rects(&self, range: Range<usize>) -> Vec<Rect> {
    let range = match &self.map {
      Some(map) => map.range_to_shown(range),
      None => range,
    };

    self.inner.range_rects(range)
  }

  fn run_extents(&self) -> Vec<RunExtent> {
    self
      .inner
      .run_extents()
      .into_iter()
      .map(|extent| RunExtent {
        range: self.range_to_real(extent.range.clone()),
        ..extent
      })
      .collect()
  }

  fn move_visual(&self, position: Position, movement: VisualMove) -> Position {
    let movement = match (movement, &self.map) {
      (VisualMove::WordLeft, Some(_)) => VisualMove::LineStart,
      (VisualMove::WordRight, Some(_)) => VisualMove::LineEnd,
      (movement, _) => movement,
    };

    self.to_real(self.inner.move_visual(self.to_shown(position), movement))
  }

  fn move_vertical(&self, position: Position, lines: isize, goal: Option<f32>) -> (Position, f32) {
    let (moved, goal) = self
      .inner
      .move_vertical(self.to_shown(position), lines, goal);

    (self.to_real(moved), goal)
  }
}
