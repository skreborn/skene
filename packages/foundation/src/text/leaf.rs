//! A text leaf of the layout engine: a paragraph measured as taffy asks, its intrinsic widths
//! cached for the paragraph's life and its layouts by width.

use std::{
  cell::{Cell, RefCell},
  ops::Range,
};

use crate::text::{Intrinsic, Laid, PaintStyle, Paragraph};

/// The space a leaf is measured in along one axis.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub enum Space {
  /// As narrow as it can be: its min-content size.
  MinContent,
  /// As wide as it would be: its max-content size.
  MaxContent,
  /// Within this much.
  Definite(f32),
}

/// What a leaf measures at: its size, and its baselines from its top.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct LeafMeasure {
  /// The width.
  pub width: f32,
  /// The height.
  pub height: f32,
  /// The first line's baseline.
  pub first_baseline: Option<f32>,
  /// The last line's baseline.
  pub last_baseline: Option<f32>,
}

/// A leaf the layout engine measures itself rather than through a toolkit: text, through its
/// realization's [`crate::text::TextEngine`].
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a text the layout engine measures",
  label = "a text leaf is measured here",
  note = "a text leaf measures through its realization's text engine: `TextLeaf` is one"
)]
pub trait MeasureText {
  /// The leaf's size given what is known of it and the space it has.
  fn measure(&self, known: (Option<f32>, Option<f32>), available: (Space, Space)) -> LeafMeasure;

  /// The leaf's width alone in the space `available` along its width: the width
  /// [`MeasureText::measure`] gives there, its height not asked for.
  fn measure_width(&self, available: Space) -> f32 {
    self
      .measure((None, None), (available, Space::MaxContent))
      .width
  }
}

/// The most layouts a leaf keeps, by width.
const KEPT: usize = 3;

/// A paragraph as a leaf of the layout engine.
///
/// Min-content is the paragraph's narrowest width, max-content its widest; a definite width
/// lays it out within that width and reports the width it uses, so a width under min-content
/// breaks words where the line fills; a known width lays it out at that width. Its intrinsic
/// widths are measured once a paragraph, and its last three layouts are kept by width. A
/// width at or past its widest shares the unbounded layout wherever only sizes and baselines
/// are asked for (measuring), and for a layout where the paragraph's lines start at the left
/// edge whatever the width ([`Paragraph::is_left_anchored`]).
#[must_use]
pub struct TextLeaf<P: Paragraph> {
  paragraph: P,
  epoch: u64,
  intrinsic: Cell<Option<Intrinsic>>,
  laid: RefCell<Vec<(Option<u32>, P::Laid)>>,
  layouts: Cell<u32>,
}

impl<P: Paragraph> std::fmt::Debug for TextLeaf<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("TextLeaf")
      .field("text", &self.paragraph.text())
      .field("epoch", &self.epoch)
      .field("layouts", &self.layouts.get())
      .finish_non_exhaustive()
  }
}

impl<P: Paragraph> TextLeaf<P> {
  /// The leaf of `paragraph`.
  pub fn new(paragraph: P) -> Self {
    Self {
      paragraph,
      epoch: 0,
      intrinsic: Cell::new(None),
      laid: RefCell::new(Vec::with_capacity(KEPT)),
      layouts: Cell::new(0),
    }
  }

  /// The paragraph.
  #[inline(always)]
  pub const fn paragraph(&self) -> &P {
    &self.paragraph
  }

  /// Replaces the paragraph after its content or a metric-affecting style changed: a new
  /// epoch, every cached measurement dropped.
  pub fn replace(&mut self, paragraph: P) {
    self.paragraph = paragraph;
    self.epoch += 1;
    self.intrinsic.set(None);
    self.laid.borrow_mut().clear();
  }

  /// Repaints `range` with `paint`, keeping every measurement and every layout, which show
  /// it: `false` where the engine needs the paragraph rebuilt ([`TextLeaf::replace`]).
  pub fn repaint(&mut self, range: Range<usize>, paint: &PaintStyle) -> bool {
    self.paragraph.repaint(range, paint)
  }

  /// The paragraph's epoch: how many times it was replaced.
  #[inline(always)]
  #[must_use]
  pub const fn epoch(&self) -> u64 {
    self.epoch
  }

  /// How many layouts the engine made for this leaf.
  #[inline(always)]
  #[must_use]
  pub const fn layouts(&self) -> u32 {
    self.layouts.get()
  }

  /// The paragraph's intrinsic widths, measured once an epoch.
  pub fn intrinsic(&self) -> Intrinsic {
    if let Some(intrinsic) = self.intrinsic.get() {
      return intrinsic;
    }

    let intrinsic = self.paragraph.intrinsic();

    self.intrinsic.set(Some(intrinsic));

    intrinsic
  }

  /// The paragraph laid out within `width` (unbounded where `None`), from the cache where a
  /// layout at that width is kept.
  pub fn laid(&self, width: Option<f32>) -> P::Laid {
    self.kept(width, self.paragraph.is_left_anchored())
  }

  /// The layout within `width`, the unbounded one standing for any width at or past the
  /// widest where `shared`.
  fn kept(&self, width: Option<f32>, shared: bool) -> P::Laid {
    let key = width
      .filter(|width| !shared || *width < self.intrinsic().max)
      .map(f32::to_bits);
    let mut kept = self.laid.borrow_mut();

    if let Some(at) = kept.iter().position(|(width, _)| *width == key) {
      let entry = kept.remove(at);
      let laid = entry.1.clone();

      kept.insert(0, entry);

      return laid;
    }

    let laid = self.paragraph.layout(key.map(f32::from_bits));

    self.layouts.set(self.layouts.get() + 1);
    kept.truncate(KEPT - 1);
    kept.insert(0, (key, laid.clone()));

    laid
  }
}

impl<P: Paragraph> MeasureText for TextLeaf<P> {
  fn measure_width(&self, available: Space) -> f32 {
    match available {
      Space::MinContent => self.intrinsic().min,
      Space::MaxContent => self.intrinsic().max,
      Space::Definite(width) => self.kept(Some(width), true).size().width,
    }
  }

  fn measure(&self, known: (Option<f32>, Option<f32>), available: (Space, Space)) -> LeafMeasure {
    let intrinsic = self.intrinsic();
    let within = match (known.0, available.0) {
      (Some(width), _) | (None, Space::Definite(width)) => Some(width),
      (None, Space::MinContent) => Some(intrinsic.min),
      (None, Space::MaxContent) => None,
    };
    let laid = self.kept(within, true);
    let size = laid.size();
    let width = match (known.0, available.0) {
      (Some(width), _) => width,
      (None, Space::MinContent) => intrinsic.min,
      (None, Space::MaxContent) => intrinsic.max,
      (None, Space::Definite(_)) => size.width,
    };

    LeafMeasure {
      width,
      height: known.1.unwrap_or(size.height),
      first_baseline: Some(laid.first_baseline()),
      last_baseline: Some(laid.last_baseline()),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{LeafMeasure, MeasureText, Space, TextLeaf};
  use crate::{
    Color,
    text::{
      AttributedText,
      Laid,
      PaintStyle,
      ParagraphStyle,
      TextAlignment,
      TextEngine,
      TextStyle,
      grid::{Grid, GridParagraph},
    },
  };

  fn leaf(text: &str) -> TextLeaf<GridParagraph> {
    TextLeaf::new(Grid.paragraph(
      &AttributedText::plain(text, TextStyle::new(13.)),
      &ParagraphStyle::default(),
    ))
  }

  fn measured(leaf: &TextLeaf<GridParagraph>, width: Space) -> (f32, f32) {
    let LeafMeasure { width, height, .. } = leaf.measure((None, None), (width, Space::MaxContent));

    (width, height)
  }

  #[test]
  fn min_and_max_content_are_the_intrinsic_widths() {
    let leaf = leaf("alpha beta gamma");

    assert_eq!(measured(&leaf, Space::MinContent), (50., 60.));
    assert_eq!(measured(&leaf, Space::MaxContent), (160., 20.));
    assert_eq!(leaf.measure_width(Space::MinContent), 50.);
  }

  #[test]
  fn a_definite_width_reports_the_width_used() {
    let leaf = leaf("alpha beta gamma");

    assert_eq!(measured(&leaf, Space::Definite(125.)), (100., 40.));
    // Under min-content a word breaks where the line fills, as overflow-wrap: break-word
    // does: the leaf is as wide as the space, not clamped to its min-content.
    assert_eq!(measured(&leaf, Space::Definite(30.)), (30., 120.));
    // Past max-content it is as wide as its widest line.
    assert_eq!(measured(&leaf, Space::Definite(500.)), (160., 20.));
  }

  #[test]
  fn a_known_width_lays_out_at_it_with_both_baselines() {
    let leaf = leaf("alpha beta gamma");
    let measure = leaf.measure((Some(60.), None), (Space::MaxContent, Space::MaxContent));

    assert_eq!(measure.width, 60.);
    assert_eq!(measure.height, 60.);
    assert_eq!(measure.first_baseline, Some(15.));
    assert_eq!(measure.last_baseline, Some(55.));
  }

  #[test]
  fn intrinsics_are_measured_once_and_layouts_kept_by_width() {
    let mut leaf = leaf("alpha beta gamma");

    for _ in 0..3 {
      let _ = measured(&leaf, Space::Definite(125.));
      let _ = measured(&leaf, Space::MaxContent);
      let _ = measured(&leaf, Space::Definite(900.));
    }

    // Two layouts: at 125, and unbounded (shared by max-content and every width past it).
    assert_eq!(leaf.layouts(), 2);

    for width in [70., 80., 90., 125.] {
      let _ = measured(&leaf, Space::Definite(width));
    }

    // Three kept: 125 was pushed out by 70, 80 and 90.
    assert_eq!(leaf.layouts(), 6);
    assert!(
      leaf.repaint(0..5, &PaintStyle::new(Color::TRANSPARENT)),
      "paint never relayouts"
    );

    let _ = measured(&leaf, Space::Definite(125.));

    assert_eq!(leaf.layouts(), 6);

    leaf.replace(Grid.paragraph(
      &AttributedText::plain("other", TextStyle::new(13.)),
      &ParagraphStyle::default(),
    ));
    assert_eq!(leaf.epoch(), 1);
    assert_eq!(measured(&leaf, Space::MaxContent), (50., 20.));
  }

  #[test]
  fn a_layout_past_the_widest_line_is_shared_only_where_the_width_moves_no_line() {
    let start = leaf("alpha");

    let _ = measured(&start, Space::MaxContent);
    assert_eq!(start.laid(Some(500.)).lines()[0].left, 0.);
    assert_eq!(
      start.layouts(),
      1,
      "start-aligned lines share the unbounded layout"
    );

    let centered = TextLeaf::new(Grid.paragraph(
      &AttributedText::plain("alpha", TextStyle::new(13.)),
      &ParagraphStyle::default().with_alignment(TextAlignment::Center),
    ));

    assert_eq!(measured(&centered, Space::Definite(500.)), (50., 20.));
    assert_eq!(
      centered.layouts(),
      1,
      "measuring shares the unbounded layout"
    );
    assert_eq!(
      centered.laid(Some(500.)).lines()[0].left,
      225.,
      "centered within the width"
    );
    assert_eq!(centered.layouts(), 2);
  }
}
