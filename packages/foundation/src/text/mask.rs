//! Substituted text: the same engine laying out, in place of some of a text's grapheme
//! clusters, a character of its own, every position it reports and takes still a byte of the
//! real text. A secure entry's mask is one ([`Masked`]): each cluster laid out as a mark but
//! for the one a reader just typed, shown for a moment.

use std::{cell::Cell, ops::Range, rc::Rc};

use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
  Rect,
  text::{
    AttributedText,
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
    TextBoundary,
    TextEngine,
    TextIndex,
    TextRun,
    TextSize,
    VisualMove,
  },
};

/// What a [`Substituted`] engine lays out in place of a text's grapheme clusters.
#[diagnostic::on_unimplemented(
  message = "`{Self}` says nothing of what a substituted text lays out",
  label = "a substitution is needed here",
  note = "a substitution names the character laid out in place of each grapheme cluster it replaces"
)]
pub trait Substitute {
  /// Whether anything is substituted now: where not, the text is laid out as it is.
  fn is_active(&self) -> bool {
    true
  }

  /// The character laid out in place of `cluster`, which starts at byte `start` of the real
  /// text: `None` lays the cluster out as it is.
  fn substitute(&self, cluster: &str, start: usize) -> Option<char>;

  /// Whether the text laid out hides the real text's words, so a word or a paragraph
  /// boundary is the whole text's and a visual word move goes to the line's end.
  fn hides_words(&self) -> bool {
    false
  }
}

#[diagnostic::do_not_recommend]
impl<S: Substitute + ?Sized> Substitute for Rc<S> {
  #[inline(always)]
  fn is_active(&self) -> bool {
    (**self).is_active()
  }

  #[inline(always)]
  fn substitute(&self, cluster: &str, start: usize) -> Option<char> {
    (**self).substitute(cluster, start)
  }

  #[inline(always)]
  fn hides_words(&self) -> bool {
    (**self).hides_words()
  }
}

/// An engine laying text out with clusters substituted where its [`Substitute`] says.
///
/// It lays out as its inner engine does otherwise: every position it reports and takes is a
/// byte of the real text, and the text's runs and inline boxes are carried over to the text
/// laid out.
#[derive(Clone, Debug)]
#[must_use]
pub struct Substituted<E, S> {
  inner: E,
  substitute: S,
}

impl<E, S> Substituted<E, S> {
  /// `inner`, substituting as `substitute` says.
  #[inline(always)]
  pub const fn new(inner: E, substitute: S) -> Self {
    Self { inner, substitute }
  }

  /// The inner engine.
  #[inline(always)]
  pub const fn inner(&self) -> &E {
    &self.inner
  }

  /// The substitution.
  #[inline(always)]
  pub const fn substitute(&self) -> &S {
    &self.substitute
  }
}

/// The bytes where each cluster starts in the real text and in the shown one, the ends
/// included.
#[derive(Debug)]
struct Map {
  real: Vec<usize>,
  shown: Vec<usize>,
  index: TextIndex,
  hides_words: bool,
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

/// A paragraph of [`Substituted`]: the inner engine's, over the text shown where anything is
/// substituted.
#[derive(Debug)]
#[must_use]
pub struct SubstitutedParagraph<P> {
  inner: P,
  real: Rc<str>,
  map: Option<Rc<Map>>,
}

/// A layout of [`SubstitutedParagraph`].
#[derive(Clone, Debug)]
#[must_use]
pub struct SubstitutedLaid<L> {
  inner: L,
  map: Option<Rc<Map>>,
}

impl<L> SubstitutedLaid<L> {
  /// The inner engine's layout, of the text as shown.
  #[inline(always)]
  pub const fn inner(&self) -> &L {
    &self.inner
  }

  /// Whether anything was substituted when the text was laid out.
  #[inline(always)]
  #[must_use]
  pub const fn is_substituted(&self) -> bool {
    self.map.is_some()
  }

  /// The byte of the real text where the cluster shown at byte `shown` of the inner layout's
  /// text starts: `shown` itself where nothing is substituted.
  #[must_use]
  pub fn real_index(&self, shown: usize) -> usize {
    self.map.as_ref().map_or(shown, |map| map.to_real(shown))
  }

  /// The byte of the inner layout's text where the cluster at byte `real` of the real text is
  /// shown: `real` itself where nothing is substituted.
  #[must_use]
  pub fn shown_index(&self, real: usize) -> usize {
    self.map.as_ref().map_or(real, |map| map.to_shown(real))
  }
}

impl<E: TextEngine, S: Substitute> TextEngine for Substituted<E, S> {
  type Paragraph = SubstitutedParagraph<E::Paragraph>;

  fn paragraph(&self, text: &AttributedText, style: &ParagraphStyle) -> Self::Paragraph {
    let real = text.shared_value();
    let unchanged = || SubstitutedParagraph {
      inner: self.inner.paragraph(text, style),
      real: Rc::clone(&real),
      map: None,
    };

    if !self.substitute.is_active() {
      return unchanged();
    }

    let mut shown = String::with_capacity(real.len());
    let mut map = Map {
      real: Vec::new(),
      shown: Vec::new(),
      index: TextIndex::default(),
      hides_words: self.substitute.hides_words(),
    };
    let mut substituted = false;

    for (start, cluster) in real.grapheme_indices(true) {
      map.real.push(start);
      map.shown.push(shown.len());

      match self.substitute.substitute(cluster, start) {
        Some(character) => {
          substituted = true;
          shown.push(character);
        }
        None => shown.push_str(cluster),
      }
    }

    if !substituted {
      return unchanged();
    }

    map.real.push(real.len());
    map.shown.push(shown.len());
    map.index = TextIndex::new(&real);

    let runs = text
      .runs()
      .iter()
      .map(|run| TextRun {
        range: map.range_to_shown(run.range.clone()),
        style: run.style.clone(),
      })
      .collect();
    let boxes = text
      .boxes()
      .iter()
      .map(|inline| InlineBox {
        index: map.to_shown(inline.index),
        ..*inline
      })
      .collect();
    let shown = AttributedText::new(shown, text.base().clone(), runs).with_boxes(boxes);

    SubstitutedParagraph {
      inner: self.inner.paragraph(&shown, style),
      real,
      map: Some(Rc::new(map)),
    }
  }
}

impl<P: Paragraph> Paragraph for SubstitutedParagraph<P> {
  type Laid = SubstitutedLaid<P::Laid>;

  fn text(&self) -> &str {
    &self.real
  }

  fn intrinsic(&self) -> Intrinsic {
    self.inner.intrinsic()
  }

  fn is_left_anchored(&self) -> bool {
    self.inner.is_left_anchored()
  }

  fn layout(&self, width: Option<f32>) -> Self::Laid {
    SubstitutedLaid {
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
      Some(map)
        if map.hides_words && matches!(unit, TextBoundary::Word | TextBoundary::Paragraph) =>
      {
        match direction {
          Direction::Forward => self.real.len(),
          Direction::Backward => 0,
        }
      }
      Some(map) => Segments::new(&self.real).boundary(&map.index, index, unit, direction),
    }
  }
}

impl<L: Laid> SubstitutedLaid<L> {
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

impl<L: Laid> Laid for SubstitutedLaid<L> {
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

  fn range_rects(&self, range: Range<usize>) -> Vec<Rect<f32>> {
    let range = match &self.map {
      Some(map) => map.range_to_shown(range),
      None => range,
    };

    self.inner.range_rects(range)
  }

  fn inline_boxes(&self) -> Vec<Rect<f32>> {
    self.inner.inline_boxes()
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
    let hides_words = self.map.as_ref().is_some_and(|map| map.hides_words);
    let movement = match movement {
      VisualMove::WordLeft if hides_words => VisualMove::LineStart,
      VisualMove::WordRight if hides_words => VisualMove::LineEnd,
      movement => movement,
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

/// Whether an entry's text is masked, and which cluster shows as typed: shared between the
/// entry and its engine ([`Masked`]), so a change takes effect at the next layout.
#[derive(Debug)]
#[must_use]
pub struct MaskState {
  masked: Cell<bool>,
  revealed: Cell<Option<usize>>,
  mark: Cell<char>,
}

impl MaskState {
  /// A state masking nothing, each masked cluster drawn as `mark`.
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
    }
    else {
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

impl Substitute for MaskState {
  #[inline(always)]
  fn is_active(&self) -> bool {
    self.is_masked()
  }

  fn substitute(&self, _: &str, start: usize) -> Option<char> {
    (self.revealed() != Some(start)).then(|| self.mark())
  }

  #[inline(always)]
  fn hides_words(&self) -> bool {
    true
  }
}

/// An engine laying a secure entry's text out masked where its [`MaskState`] says, and as its
/// inner engine does otherwise.
pub type Masked<E> = Substituted<E, Rc<MaskState>>;

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::{MaskState, Masked, Substitute, Substituted};
  use crate::text::{
    AttributedText,
    Direction,
    InlineBox,
    Laid,
    Paragraph,
    ParagraphStyle,
    Position,
    TextBoundary,
    TextEngine,
    TextStyle,
    VisualMove,
    grid::{ADVANCE, Grid},
  };

  /// "é" written as an e and a combining acute, then a face: two clusters, five characters.
  const SECRET: &str = "e\u{301}\u{1F600}ab";

  #[test]
  fn a_masked_text_lays_out_its_marks_and_keeps_real_offsets() {
    let state = Rc::new(MaskState::new('\u{2022}'));
    let engine = Masked::new(Grid, Rc::clone(&state));
    let text = AttributedText::plain(SECRET, TextStyle::new(13.))
      .with_boxes(vec![InlineBox::new(7, 20., 10.)]);
    let style = ParagraphStyle::default();

    state.set_masked(true);

    let paragraph = engine.paragraph(&text, &style);
    let laid = paragraph.layout(None);

    assert_eq!(
      paragraph.text(),
      SECRET,
      "the paragraph's text is the real one"
    );
    assert_eq!(
      laid.inner().lines()[0].range,
      0..12,
      "four marks of three bytes"
    );
    assert_eq!(laid.lines()[0].range, 0..SECRET.len());
    // Four clusters and the box after the second, at ten units a cluster.
    assert_eq!(laid.size().width, 4. * ADVANCE + 20.);
    assert_eq!(laid.inline_boxes()[0].origin.x, 2. * ADVANCE);
    // A hit past the second mark lands after the face, a byte of the real text.
    assert_eq!(laid.hit_test(2. * ADVANCE - 1., 5.).position.index, 7);
    assert_eq!(laid.caret(Position::new(3)).primary.x, ADVANCE);
    assert_eq!(laid.real_index(6), 7);
    assert_eq!(laid.shown_index(7), 6);
    // Its words are hidden: a word is the whole text, a word move the line's end.
    assert_eq!(
      paragraph.boundary(3, TextBoundary::Word, Direction::Forward),
      SECRET.len()
    );
    assert_eq!(
      laid
        .move_visual(Position::new(0), VisualMove::WordRight)
        .index,
      SECRET.len()
    );
    assert_eq!(
      paragraph.boundary(0, TextBoundary::Grapheme, Direction::Forward),
      3
    );

    state.reveal(Some(3));

    let revealed = engine.paragraph(&text, &style).layout(None);

    assert_eq!(
      revealed.inner().lines()[0].range,
      0..13,
      "the face shows as typed"
    );

    state.set_masked(false);

    let plain = engine.paragraph(&text, &style).layout(None);

    assert!(!plain.is_substituted());
    assert_eq!(plain.real_index(5), 5);
  }

  /// Stands every wide cluster in for one character.
  struct Wide;

  impl Substitute for Wide {
    fn substitute(&self, cluster: &str, _: usize) -> Option<char> {
      (cluster.len() > 1).then_some('W')
    }
  }

  #[test]
  fn a_substitution_keeps_the_real_texts_words() {
    let engine = Substituted::new(Grid, Wide);
    let text = AttributedText::plain("ab \u{1F600}cd ef", TextStyle::new(13.));
    let paragraph = engine.paragraph(&text, &ParagraphStyle::default());
    let laid = paragraph.layout(None);

    assert!(laid.is_substituted());
    assert_eq!(laid.lines()[0].range, 0..text.value().len());
    assert_eq!(
      paragraph.boundary(3, TextBoundary::Word, Direction::Forward),
      7,
      "the face is a segment of its own in the real text"
    );
    assert_eq!(laid.caret(Position::new(7)).primary.x, 4. * ADVANCE);
    assert!(
      !Substituted::new(Grid, Wide)
        .paragraph(
          &AttributedText::plain("plain", TextStyle::new(13.)),
          &ParagraphStyle::default()
        )
        .layout(None)
        .is_substituted(),
      "nothing to substitute"
    );
  }
}
